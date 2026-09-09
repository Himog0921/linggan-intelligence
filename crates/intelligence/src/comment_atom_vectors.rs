//! Bounded, cached embedding work for current semantic atoms. No generated text is used as an
//! execution instruction. Spaces exclude credential versions and include provider/model/shape.
use crate::{
    comment_execution_budget::{BudgetPurpose, check_budget_in},
    comment_research::comment_source_hash,
    model_invocation::{checkpoint_invocation_usage, connection_request, finish_invocation_in},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::{PiAdapter, PiResponse},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

fn uuid(v: &Value, key: &str) -> Result<Uuid, ModelError> {
    v[key]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(ModelError::Invalid)
}
fn draining(drain: Option<&ModelWorkerDrain>) -> bool {
    drain.is_some_and(ModelWorkerDrain::is_requested)
}

pub(crate) async fn run_once(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&ModelWorkerDrain>,
) -> Result<bool, ModelError> {
    if draining(drain) {
        return Ok(false);
    }
    let Some(config) = crate::embedding_settings::active_config(db).await? else {
        return Ok(false);
    };
    let dimensions = config["dimensions"]
        .as_i64()
        .filter(|n| (2..=8192).contains(n))
        .ok_or(ModelError::Invalid)? as i32;
    let space_hash=comment_source_hash(&json!({"provider":config["api"],"endpoint":config["baseUrl"],"model":config["modelId"],"dimensions":dimensions,"normalization":"l2"}).to_string());
    let space:Uuid=sqlx::query_scalar("INSERT INTO linggan_ci_semantic_space(space_ref,model_ref,model_id,model_version,dimensions,normalization,space_hash,provider_api,endpoint) VALUES($1,$2,$3,$3,$4,'l2',$5,$6,$7) ON CONFLICT(space_hash) DO NOTHING RETURNING space_ref")
        .bind(Uuid::new_v4()).bind(uuid(&config,"modelRef")?).bind(config["modelId"].as_str().ok_or(ModelError::Invalid)?).bind(dimensions).bind(&space_hash).bind(config["api"].as_str().ok_or(ModelError::Invalid)?).bind(config["baseUrl"].as_str().ok_or(ModelError::Invalid)?).fetch_optional(db.pool()).await?
        .unwrap_or_else(Uuid::nil);
    let space = if space.is_nil() {
        sqlx::query_scalar("SELECT space_ref FROM linggan_ci_semantic_space WHERE space_hash=$1")
            .bind(&space_hash)
            .fetch_one(db.pool())
            .await?
    } else {
        space
    };
    recover(db).await?;
    // Repeated equivalent atoms reuse a space/definition cache entry; different people still
    // remain separate atoms and source counts, even when their semantic vectors are shared.
    sqlx::query("INSERT INTO linggan_ci_atom_embedding_work(space_ref,definition_hash,atom_ref,state) SELECT $1,a.definition_hash,min(a.atom_ref::text)::uuid,'pending' FROM linggan_ci_semantic_atom_current a WHERE NOT EXISTS(SELECT 1 FROM linggan_ci_atom_vector v WHERE v.space_ref=$1 AND v.definition_hash=a.definition_hash) GROUP BY a.definition_hash ORDER BY a.definition_hash LIMIT 500 ON CONFLICT DO NOTHING")
        .bind(space).execute(db.pool()).await?;
    let Some(job) = reserve(db, space, dimensions, &config, drain).await? else {
        return Ok(false);
    };
    let mut called = false;
    let outcome=async {
        let mut request=connection_request(db,store,uuid(&config,"connectionVersionRef")?).await?;
        request.operation="embed".into();request.model_id=config["modelId"].as_str().ok_or(ModelError::Invalid)?.into();
        request.prompt=job.prompt.clone();request.timeout_ms=30000;request.max_output_tokens=16;
        if draining(drain){return Err(ModelError::Disabled);}
        let active=crate::embedding_settings::active_config(db).await?.ok_or(ModelError::Disabled)?;
        if active["configRef"]!=config["configRef"] {return Err(ModelError::Disabled);}
        let readable:i64=sqlx::query_scalar("SELECT count(DISTINCT definition_hash) FROM linggan_ci_semantic_atom_current WHERE definition_hash=ANY($1)")
            .bind(&job.hashes).fetch_one(db.pool()).await?;
        if readable!=job.hashes.len() as i64{return Err(ModelError::Source);}
        called=true;adapter.call(&request).await
    }.await;
    let response = outcome.as_ref().ok();
    if called {
        checkpoint_invocation_usage(db, job.invocation, response).await?;
    }
    finish(
        db,
        &job,
        response,
        outcome.as_ref().err().map(ModelError::code),
        called,
    )
    .await?;
    Ok(true)
}
struct VectorJob {
    space: Uuid,
    dimensions: i32,
    invocation: Uuid,
    hashes: Vec<String>,
    prompt: String,
    reserved: i64,
}
async fn reserve(
    db: &Database,
    space: Uuid,
    dimensions: i32,
    config: &Value,
    drain: Option<&ModelWorkerDrain>,
) -> Result<Option<VectorJob>, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let rows=sqlx::query(r#"SELECT w.definition_hash,a.meaning,a.context_text,a.target,a.position,(w.state='failed' AND (v.input_tokens IS NULL OR v.output_tokens IS NULL)) AS unknown_retry FROM linggan_ci_atom_embedding_work w
      JOIN LATERAL(SELECT meaning,context_text,target,position FROM linggan_ci_semantic_atom_current a WHERE a.definition_hash=w.definition_hash ORDER BY a.atom_ref LIMIT 1) a ON true
      LEFT JOIN linggan_model_invocation v ON v.invocation_ref=w.invocation_ref
      WHERE w.space_ref=$1 AND (w.state='pending' OR w.state='failed' AND w.attempts<3 AND w.updated_at<=scope_001_now()-interval '60 seconds'
        AND w.failure_code IN('provider_timeout','provider_network_error','provider_rate_limited','provider_unavailable','worker_interrupted','partial_embedding_output','invalid_embedding_output')
        AND ((v.input_tokens IS NOT NULL AND v.output_tokens IS NOT NULL) OR w.unknown_retry_attempts<COALESCE((SELECT (auto_policy->>'unknownRetryMaxAttempts')::integer FROM linggan_comment_daily_schedule WHERE singleton),0)))
      ORDER BY w.updated_at,w.definition_hash LIMIT 20 FOR UPDATE OF w"#).bind(space).fetch_all(&mut *tx).await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut hashes = Vec::new();
    let mut texts = Vec::new();
    let mut bytes = 512;
    let mut unknown_hashes = Vec::new();
    for row in rows {
        let meaning: String = row.get("meaning");
        let context: Option<String> = row.get("context_text");
        let target: Option<String> = row.get("target");
        let position: Option<String> = row.get("position");
        let meaning = match (target, position) {
            (Some(t), Some(p)) => format!("{t}\n{p}\n{meaning}"),
            _ => meaning,
        };
        let text = if let Some(context) = context {
            format!("{meaning}\n{context}")
        } else {
            meaning
        };
        if bytes + text.len() > 16000 {
            break;
        }
        bytes += text.len();
        let hash: String = row.get("definition_hash");
        if row.get::<bool, _>("unknown_retry") {
            unknown_hashes.push(hash.clone());
        }
        hashes.push(hash);
        texts.push(text);
    }
    if hashes.is_empty() {
        return Ok(None);
    }
    let prompt = json!(texts).to_string();
    let reserved = (prompt.len() + 512) as i64;
    let budget = check_budget_in(
        &mut tx,
        BudgetPurpose::Semantic,
        reserved,
        !unknown_hashes.is_empty(),
    )
    .await?;
    if !budget.allowed {
        sqlx::query("UPDATE linggan_ci_atom_embedding_work SET failure_code=$3 WHERE space_ref=$1 AND definition_hash=ANY($2) AND state='pending'")
            .bind(space).bind(&hashes).bind(budget.reason_code).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(None);
    }
    if draining(drain) {
        return Ok(None);
    }
    let invocation = Uuid::new_v4();
    let mut metadata = budget.ledger_metadata;
    metadata["task"] = json!("atom_embedding");
    metadata["spaceRef"] = json!(space);
    metadata["definitionHashes"] = json!(hashes);
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,'embed',$4,'running',$5,$5,$6)")
        .bind(invocation).bind(uuid(config,"connectionVersionRef")?).bind(uuid(config,"modelRef")?).bind(comment_source_hash(&prompt)).bind(reserved).bind(metadata).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_ci_atom_embedding_work SET state='running',attempts=attempts+1,unknown_retry_attempts=unknown_retry_attempts+CASE WHEN definition_hash=ANY($4) THEN 1 ELSE 0 END,invocation_ref=$3,lease_until=scope_001_now()+interval '60 seconds',failure_code=NULL,updated_at=scope_001_now() WHERE space_ref=$1 AND definition_hash=ANY($2)")
        .bind(space).bind(&hashes).bind(invocation).bind(&unknown_hashes).execute(&mut *tx).await?;
    if draining(drain) {
        return Ok(None);
    }
    tx.commit().await?;
    Ok(Some(VectorJob {
        space,
        dimensions,
        invocation,
        hashes,
        prompt,
        reserved,
    }))
}
async fn recover(db: &Database) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    let invocations:Vec<Uuid>=sqlx::query_scalar("UPDATE linggan_ci_atom_embedding_work SET state='failed',failure_code='worker_interrupted',updated_at=scope_001_now() WHERE state='running' AND lease_until<=scope_001_now() RETURNING invocation_ref").fetch_all(&mut *tx).await?;
    sqlx::query("UPDATE linggan_model_invocation SET state='failed',failure_code='worker_interrupted',finished_at=scope_001_now(),result=COALESCE(result,'{}'::jsonb)||jsonb_build_object('usageUnknown',input_tokens IS NULL OR output_tokens IS NULL,'callStarted',true) WHERE invocation_ref=ANY($1) AND state='running'").bind(invocations).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
fn normalized_vector(value: &Value, dimension: usize) -> Option<Vec<u8>> {
    let values = value.as_array().filter(|v| v.len() == dimension)?;
    let values: Vec<f64> = values
        .iter()
        .map(|n| n.as_f64().filter(|n| n.is_finite()))
        .collect::<Option<_>>()?;
    let norm = values.iter().map(|n| n * n).sum::<f64>().sqrt();
    if !norm.is_finite() || norm <= 0.0 {
        return None;
    }
    Some(
        values
            .iter()
            .flat_map(|n| ((*n / norm) as f32).to_le_bytes())
            .collect(),
    )
}
fn normalized_vectors(
    response: &PiResponse,
    count: usize,
    dimension: usize,
) -> Result<Vec<Option<Vec<u8>>>, ModelError> {
    let output: Value =
        serde_json::from_str(response.text.as_deref().ok_or(ModelError::InvalidOutput)?)
            .map_err(|_| ModelError::InvalidOutput)?;
    if !response.ok {
        return Err(ModelError::InvalidOutput);
    }
    // Transport has already validated unique bounded indexes and filled missing slots with null.
    let vectors = output["vectors"]
        .as_array()
        .filter(|v| v.len() == count)
        .ok_or(ModelError::InvalidOutput)?;
    Ok(vectors
        .iter()
        .map(|v| normalized_vector(v, dimension))
        .collect())
}
async fn finish(
    db: &Database,
    job: &VectorJob,
    response: Option<&PiResponse>,
    error: Option<&str>,
    called: bool,
) -> Result<(), ModelError> {
    let vectors = if error.is_none() {
        response
            .ok_or(ModelError::InvalidOutput)
            .and_then(|p| normalized_vectors(p, job.hashes.len(), job.dimensions as usize))
    } else {
        Err(ModelError::InvalidOutput)
    };
    let failure = error
        .or_else(|| {
            response
                .filter(|p| !p.ok)
                .and_then(|p| p.failure_code.as_deref())
        })
        .or(if vectors.is_err() {
            Some("invalid_embedding_output")
        } else {
            None
        });
    let failure = if response
        .and_then(|p| p.usage.input_tokens)
        .is_some_and(|n| n > job.reserved)
    {
        Some("embedding_budget_overrun")
    } else {
        failure
    };
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ci_atom_embedding_work WHERE space_ref=$1 AND invocation_ref=$2 AND state='running' AND lease_until>scope_001_now())").bind(job.space).bind(job.invocation).fetch_one(&mut *tx).await?;
    if !valid {
        return Err(ModelError::Conflict);
    }
    let mut accepted = 0;
    for (index, hash) in job.hashes.iter().enumerate() {
        let readable:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ci_semantic_atom_current WHERE definition_hash=$1)").bind(hash).fetch_one(&mut *tx).await?;
        let bytes = vectors.as_ref().ok().and_then(|v| v[index].as_ref());
        let member_failure = failure.or(if !readable {
            Some("source_unavailable")
        } else if bytes.is_none() {
            Some("partial_embedding_output")
        } else {
            None
        });
        if member_failure.is_none() {
            sqlx::query("INSERT INTO linggan_ci_atom_vector(space_ref,definition_hash,vector_bytes,dimensions,invocation_ref) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING")
                .bind(job.space).bind(hash).bind(bytes).bind(job.dimensions).bind(job.invocation).execute(&mut *tx).await?;
            accepted += 1;
        }
        sqlx::query("UPDATE linggan_ci_atom_embedding_work SET state=$4,failure_code=$5,updated_at=scope_001_now() WHERE space_ref=$1 AND definition_hash=$2 AND invocation_ref=$3 AND state='running'")
            .bind(job.space).bind(hash).bind(job.invocation).bind(if member_failure.is_none(){"succeeded"}else{"failed"}).bind(member_failure).execute(&mut *tx).await?;
    }
    let failure = failure.or(if accepted < job.hashes.len() {
        Some("partial_embedding_output")
    } else {
        None
    });
    finish_invocation_in(&mut tx,job.invocation,response,failure.is_none(),failure,&json!({"callStarted":called,"cachedVectors":accepted,"rejectedVectors":job.hashes.len()-accepted,"usageUnknown":called&&response.and_then(|p|p.usage.input_tokens).is_none()})).await?;
    if !called {
        sqlx::query("UPDATE linggan_model_invocation SET charged_tokens=0,input_tokens=0,output_tokens=0 WHERE invocation_ref=$1").bind(job.invocation).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_bad_member_cannot_shift_or_invalidate_other_vectors() {
        let values = [
            json!([3.0, 4.0]),
            Value::Null,
            json!([1.0]),
            json!([0.0, 0.0]),
            json!([4.0, 3.0]),
        ];
        let normalized: Vec<_> = values.iter().map(|v| normalized_vector(v, 2)).collect();
        assert!(normalized[0].is_some() && normalized[4].is_some());
        assert!(normalized[1..4].iter().all(Option::is_none));
        let bytes = normalized[0].as_ref().unwrap();
        assert!((f32::from_le_bytes(bytes[..4].try_into().unwrap()) - 0.6).abs() < 1e-6);
    }
}
