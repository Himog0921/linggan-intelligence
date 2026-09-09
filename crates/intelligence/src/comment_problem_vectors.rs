//! Qualified embedding requests and a bounded, versioned cache. Cosine only recalls;
//! Task B still decides equivalence using definitions, evidence and human boundaries.
use super::*;
use crate::comment_intelligence_problems::{
    DefinitionVector, problem_details, promote_unmatched_expression, recall_vector_candidates,
};

struct Entity {
    kind: &'static str,
    id: Uuid,
    text: String,
    fingerprint: String,
    guards: Vec<Value>,
}
fn entity(
    kind: &'static str,
    id: Uuid,
    name: &str,
    meaning: &str,
    revision: i64,
    guards: Vec<Value>,
) -> Entity {
    let text = crate::comment_cleaning::outbound(crate::comment_cleaning::clean(&format!(
        "问题：{name}\n定义：{meaning}"
    )))
    .text;
    let fingerprint =
        comment_source_hash(&json!({"text":text,"revision":revision,"guards":guards}).to_string());
    Entity {
        kind,
        id,
        text,
        fingerprint,
        guards,
    }
}
fn uuid(v: &Value, key: &str) -> Result<Uuid, ModelError> {
    v[key]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(ModelError::Invalid)
}

pub(super) async fn advance_problem_retrieval(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&crate::model_worker_drain::ModelWorkerDrain>,
) -> Result<bool, ModelError> {
    if super::drain_requested(drain) {
        return Ok(false);
    }
    // Qualification withdrawal invalidates derived features even while dispatch is disabled.
    sqlx::query("DELETE FROM linggan_ci_definition_vector v WHERE EXISTS(SELECT 1 FROM jsonb_array_elements(v.source_guards) g WHERE NOT linggan_ci_analysis_context_readable(jsonb_build_object('contextRefs',g))) OR (v.entity_kind='expression' AND NOT EXISTS(SELECT 1 FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) JOIN linggan_comment_analysis_work a ON a.work_ref=c.analysis_ref AND a.result->>'sourceSha256'=s.source_sha256 WHERE c.candidate_ref=v.entity_ref AND c.domain_ref=v.domain_ref AND c.state<>'superseded')) OR (v.entity_kind='problem' AND NOT EXISTS(SELECT 1 FROM linggan_ci_problem p JOIN linggan_ci_problem_member m USING(problem_ref) JOIN linggan_ci_source s USING(canonical_ref) WHERE p.problem_ref=v.entity_ref AND p.domain_ref=v.domain_ref AND p.redirect_ref IS NULL))").execute(db.pool()).await?;
    let Some(config) = crate::embedding_settings::active_config(db).await? else {
        return Ok(false);
    };
    let embedding_model = uuid(&config, "modelRef")?;
    let dimensions = config["dimensions"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 8192)
        .ok_or(ModelError::NotQualified)? as usize;
    let row=sqlx::query(crate::model_settings::with_model_callability("SELECT t.task_ref,t.batch_ref,c.candidate_ref,c.domain_ref,c.name,c.meaning,c.analysis_ref,s.source_ref,s.body,a.result FROM linggan_ci_problem_task t JOIN linggan_ci_problem_candidate c USING(candidate_ref) JOIN linggan_ci_source s USING(canonical_ref,domain_ref) JOIN linggan_comment_analysis_work a ON a.work_ref=c.analysis_ref AND a.result->>'sourceSha256'=s.source_sha256 JOIN linggan_comment_daily_batch b USING(batch_ref) JOIN linggan_model_config cfg ON cfg.config_ref=t.config_ref JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version cv ON cv.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) WHERE conn.enabled AND __MODEL_CALLABLE__ AND EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=c.domain_ref AND d.is_own_domain) AND t.state='blocked_retrieval' AND c.state='unmerged' AND b.enabled AND b.request->>'ruleVersion'='comment-research.v4' AND COALESCE((b.context_policy->>'existingProblems')::boolean,true) AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled)) AND NOT EXISTS(SELECT 1 FROM linggan_comment_daily_item i WHERE i.batch_ref=b.batch_ref AND i.state IN('pending','running')) ORDER BY t.created_at,t.task_ref LIMIT 1"))
        .fetch_optional(db.pool()).await?;
    let Some(row) = row else { return Ok(false) };
    let marker: Uuid = row.get("task_ref");
    let batch: Uuid = row.get("batch_ref");
    let candidate: Uuid = row.get("candidate_ref");
    let domain: Uuid = row.get("domain_ref");
    if crate::comment_cleaning::clean(&row.get::<String, _>("body")).state == "dropped"
        || !crate::comment_daily_read::context_readable(db, &row.get::<Value, _>("result")).await?
    {
        mark(db, marker, "failed", "source_unavailable").await?;
        return Ok(true);
    }
    let mut entities = vec![entity(
        "expression",
        candidate,
        &row.get::<String, _>("name"),
        &row.get::<String, _>("meaning"),
        0,
        vec![row.get::<Value, _>("result")["contextRefs"].clone()],
    )];
    match eligible_problem_entities(db, domain).await {
        Ok(items) => entities.extend(items),
        Err(ModelError::InputLimit) => {
            mark(db, marker, "failed", "problem_recall_scope_limit").await?;
            return Ok(true);
        }
        Err(error) => return Err(error),
    }
    // Qualification plus an actually empty eligible problem set is a defensible bootstrap.
    // It does not assert novelty outside the observed corpus.
    if entities.len() == 1 {
        promote_unmatched_expression(db, domain, candidate)
            .await
            .map_err(|_| ModelError::Source)?;
        mark(db, marker, "succeeded", "first_emerging_problem").await?;
        return Ok(true);
    }
    let (mut vectors, missing) =
        cached_vectors(db, &entities, domain, embedding_model, dimensions).await?;
    if !missing.is_empty() {
        if super::drain_requested(drain) {
            return Ok(false);
        }
        return embed_missing(
            db,
            store,
            adapter,
            drain,
            EmbeddingJob {
                config: &config,
                domain,
                batch,
                marker,
                entities: &missing,
                dimensions,
            },
        )
        .await;
    }
    let query = vectors
        .iter()
        .find(|v| v.problem_ref == candidate)
        .ok_or(ModelError::Source)?
        .clone();
    vectors.retain(|v| v.problem_ref != candidate);
    let matches =
        recall_vector_candidates(&query, &vectors).map_err(|_| ModelError::InvalidOutput)?;
    let targets: Vec<Uuid> = matches.into_iter().map(|(id, _)| id).collect();
    super::queue_problem_task(
        db,
        batch,
        domain,
        candidate,
        &targets,
        &format!("embedding:{embedding_model}:{dimensions}:exact-cosine.v1"),
    )
    .await?;
    Ok(true)
}

async fn cached_vectors<'a>(
    db: &Database,
    entities: &'a [Entity],
    domain: Uuid,
    embedding_model: Uuid,
    dimensions: usize,
) -> Result<(Vec<DefinitionVector>, Vec<&'a Entity>), ModelError> {
    let mut vectors = Vec::new();
    let mut missing = Vec::new();
    for entity in entities {
        let values: Option<Value> = sqlx::query_scalar("SELECT embedding FROM linggan_ci_definition_vector WHERE domain_ref=$1 AND entity_kind=$2 AND entity_ref=$3 AND definition_fingerprint=$4 AND model_ref=$5 AND dimensions=$6")
            .bind(domain).bind(entity.kind).bind(entity.id).bind(&entity.fingerprint).bind(embedding_model).bind(dimensions as i32).fetch_optional(db.pool()).await?;
        if let Some(values) = values {
            vectors.push(DefinitionVector {
                problem_ref: entity.id,
                model_version: format!("{embedding_model}:{dimensions}"),
                dimensions,
                values: serde_json::from_value(values).map_err(|_| ModelError::InvalidOutput)?,
            });
        } else if missing.len() < 2 {
            missing.push(entity);
        }
    }
    Ok((vectors, missing))
}

async fn mark(db: &Database, id: Uuid, state: &str, reason: &str) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_ci_problem_task SET state=$2,failure_code=$3,updated_at=scope_001_now() WHERE task_ref=$1").bind(id).bind(state).bind(reason).execute(db.pool()).await?;
    Ok(())
}
struct EmbeddingJob<'a> {
    config: &'a Value,
    domain: Uuid,
    batch: Uuid,
    marker: Uuid,
    entities: &'a [&'a Entity],
    dimensions: usize,
}
async fn embed_missing(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&crate::model_worker_drain::ModelWorkerDrain>,
    job: EmbeddingJob<'_>,
) -> Result<bool, ModelError> {
    let EmbeddingJob {
        config,
        domain,
        batch,
        marker,
        entities,
        dimensions,
    } = job;
    if super::drain_requested(drain) {
        return Ok(false);
    }
    let texts: Vec<&str> = entities.iter().map(|e| e.text.as_str()).collect();
    let prompt = serde_json::to_string(&texts).map_err(|_| ModelError::Invalid)?;
    if texts.iter().any(|s| s.len() > 8000) || texts.iter().map(|s| s.len()).sum::<usize>() > 16000
    {
        mark(db, marker, "failed", "embedding_input_limit").await?;
        return Ok(true);
    }
    let reserved = (prompt.len() + 512) as i64;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let marker_pending:bool=sqlx::query_scalar("SELECT state='blocked_retrieval' FROM linggan_ci_problem_task WHERE task_ref=$1 FOR UPDATE")
        .bind(marker).fetch_one(&mut *tx).await?;
    if !marker_pending {
        tx.commit().await?;
        return Ok(false);
    }
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_batch b WHERE b.batch_ref=$1 AND b.enabled AND COALESCE((SELECT sum(v.charged_tokens) FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) JOIN linggan_comment_daily_batch family ON family.batch_ref=p.batch_ref WHERE COALESCE(family.request->>'originBatchRef',family.batch_ref::text)=COALESCE(b.request->>'originBatchRef',b.batch_ref::text)),0)+$2<=COALESCE((SELECT (a.request->>'tokenLimit')::bigint FROM linggan_comment_daily_adjustment a WHERE a.batch_ref=COALESCE((b.request->>'originBatchRef')::uuid,b.batch_ref) AND a.kind='continue' ORDER BY a.created_at DESC,a.command_ref DESC LIMIT 1),b.token_limit))")
        .bind(batch).bind(reserved).fetch_one(&mut *tx).await?;
    if !allowed {
        tx.commit().await?;
        mark(db, marker, "blocked_retrieval", "model_budget_exhausted").await?;
        return Ok(false);
    }
    let budget = crate::comment_execution_budget::check_budget_in(
        &mut tx,
        crate::comment_execution_budget::BudgetPurpose::Semantic,
        reserved,
        false,
    )
    .await?;
    if !budget.allowed {
        tx.rollback().await?;
        return Ok(false);
    }
    let invocation = Uuid::new_v4();
    let packet = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,'embed',$4,'running',$5,$5,$6)")
        .bind(invocation).bind(uuid(config,"connectionVersionRef")?).bind(uuid(config,"modelRef")?).bind(comment_source_hash(&prompt)).bind(reserved).bind(&budget.ledger_metadata).execute(&mut *tx).await?;
    let refs: Vec<Uuid> = entities
        .iter()
        .flat_map(|e| e.guards.iter())
        .flat_map(|g| g["researchSourceRefs"].as_array().into_iter().flatten())
        .filter_map(|r| r.as_str().and_then(|s| Uuid::parse_str(s).ok()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    sqlx::query("INSERT INTO linggan_comment_daily_packet(packet_ref,batch_ref,source_refs,context_refs,context_hash,invocation_ref,lease_until,state,purpose) VALUES($1,$2,$3,$3,$4,$5,scope_001_now()+interval '120 seconds','running','problem_embedding')")
        .bind(packet).bind(batch).bind(refs).bind(comment_source_hash(&prompt)).bind(invocation).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_ci_problem_task SET state='running',invocation_ref=$2,packet_ref=$3,lease_until=scope_001_now()+interval '120 seconds' WHERE task_ref=$1").bind(marker).bind(invocation).bind(packet).execute(&mut *tx).await?;
    if super::drain_requested(drain) {
        tx.rollback().await?;
        return Ok(false);
    }
    tx.commit().await?;
    let dispatch = dispatch_embedding(db, store, adapter, drain, config, entities, prompt).await;
    finish_embedding(
        db,
        EmbeddingFinish {
            domain,
            marker,
            invocation,
            packet,
            config,
            entities,
            dimensions,
            reserved,
            called: dispatch.called,
            drain_interrupted: dispatch.drain_interrupted,
            result: dispatch.result,
        },
    )
    .await
}

async fn dispatch_embedding(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&crate::model_worker_drain::ModelWorkerDrain>,
    config: &Value,
    entities: &[&Entity],
    prompt: String,
) -> EmbeddingDispatch {
    if super::drain_requested(drain) {
        return EmbeddingDispatch::interrupted();
    }
    let mut called = false;
    let mut drain_interrupted = false;
    let result = async {
        let active = crate::embedding_settings::active_config(db)
            .await?
            .ok_or(ModelError::Disabled)?;
        if active["configRef"] != config["configRef"] {
            return Err(ModelError::Disabled);
        }
        for entity in entities {
            for guard in &entity.guards {
                if !crate::comment_daily_read::context_readable(db, &json!({"contextRefs":guard}))
                    .await?
                {
                    return Err(ModelError::Source);
                }
            }
        }
        let mut request =
            connection_request(db, store, uuid(config, "connectionVersionRef")?).await?;
        request.operation = "embed".into();
        request.model_id = config["modelId"]
            .as_str()
            .ok_or(ModelError::Invalid)?
            .into();
        request.prompt = prompt;
        request.timeout_ms = 30000;
        request.max_output_tokens = 16;
        if super::drain_requested(drain) {
            drain_interrupted = true;
            return Err(ModelError::Source);
        }
        called = true;
        adapter.call(&request).await
    }
    .await;
    EmbeddingDispatch {
        called,
        drain_interrupted,
        result,
    }
}

struct EmbeddingDispatch {
    called: bool,
    drain_interrupted: bool,
    result: Result<crate::pi_adapter::PiResponse, ModelError>,
}

impl EmbeddingDispatch {
    fn interrupted() -> Self {
        Self {
            called: false,
            drain_interrupted: true,
            result: Err(ModelError::Source),
        }
    }
}

struct EmbeddingFinish<'a> {
    domain: Uuid,
    marker: Uuid,
    invocation: Uuid,
    packet: Uuid,
    config: &'a Value,
    entities: &'a [&'a Entity],
    dimensions: usize,
    reserved: i64,
    called: bool,
    drain_interrupted: bool,
    result: Result<crate::pi_adapter::PiResponse, ModelError>,
}
async fn finish_embedding(db: &Database, run: EmbeddingFinish<'_>) -> Result<bool, ModelError> {
    let EmbeddingFinish {
        domain,
        marker,
        invocation,
        packet,
        config,
        entities,
        dimensions,
        reserved,
        called,
        drain_interrupted,
        result,
    } = run;
    let response = result.as_ref().ok();
    if called {
        checkpoint_invocation_usage(db, invocation, response).await?
    }
    let mut failure = if drain_interrupted {
        Some("worker_interrupted".to_owned())
    } else {
        result.as_ref().err().map(|e| e.code().to_owned())
    };
    let mut accepted = None;
    if let Some(r) = response {
        if !r.ok {
            failure = Some(
                r.failure_code
                    .clone()
                    .unwrap_or_else(|| "embedding_failed".into()),
            )
        }
        if r.usage.input_tokens.is_some_and(|n| n > reserved) {
            failure = Some("embedding_budget_overrun".into())
        }
        if failure.is_none() {
            accepted = r
                .text
                .as_deref()
                .and_then(|s| validate_vectors(s, entities.len(), dimensions).ok());
            if accepted.is_none() {
                failure = Some("invalid_embedding_output".into())
            }
        }
    }
    // Recheck source eligibility after the request before caching derived vectors.
    if failure.is_none() {
        for e in entities {
            for guard in &e.guards {
                if !crate::comment_daily_read::context_readable(db, &json!({"contextRefs":guard}))
                    .await?
                {
                    failure = Some("source_unavailable".into())
                }
            }
        }
    }
    if failure.is_none() {
        let vectors = accepted.ok_or(ModelError::InvalidOutput)?;
        let mut tx = db.pool().begin().await?;
        for (e, vector) in entities.iter().zip(vectors) {
            sqlx::query("INSERT INTO linggan_ci_definition_vector(vector_ref,domain_ref,entity_kind,entity_ref,definition_fingerprint,model_ref,dimensions,embedding,source_guards,invocation_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(domain_ref,entity_kind,entity_ref,definition_fingerprint,model_ref) DO NOTHING")
                .bind(Uuid::new_v4()).bind(domain).bind(e.kind).bind(e.id).bind(&e.fingerprint).bind(uuid(config,"modelRef")?).bind(dimensions as i32).bind(json!(vector)).bind(json!(e.guards)).bind(invocation).execute(&mut *tx).await?;
        }
        tx.commit().await?;
    }
    finish_invocation(db,invocation,response,failure.is_none(),failure.as_deref(),&json!({"task":"problem_embedding","callStarted":called,"entities":entities.len(),"dimensions":dimensions,"usageUnknown":called&&response.is_none()})).await?;
    if !called {
        sqlx::query("UPDATE linggan_model_invocation SET charged_tokens=0 WHERE invocation_ref=$1")
            .bind(invocation)
            .execute(db.pool())
            .await?;
    }
    sqlx::query("UPDATE linggan_comment_daily_packet SET state=$2,finished_at=scope_001_now() WHERE packet_ref=$1 AND purpose='problem_embedding'").bind(packet).bind(if failure.is_none(){"succeeded"}else{"failed"}).execute(db.pool()).await?;
    mark(
        db,
        marker,
        if failure.is_none() {
            "blocked_retrieval"
        } else {
            "failed"
        },
        failure.as_deref().unwrap_or("embedding_cached"),
    )
    .await?;
    Ok(true)
}
fn validate_vectors(
    text: &str,
    count: usize,
    dimensions: usize,
) -> Result<Vec<Vec<f64>>, ModelError> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Output {
        vectors: Vec<Vec<f64>>,
        dimensions: usize,
        // The shared adapter reports rejected positions even for a complete response.
        // Legacy Task B remains all-or-nothing; partial caching belongs to the atom path.
        #[serde(default)]
        rejected: Vec<Value>,
    }
    let out: Output = serde_json::from_str(text).map_err(|_| ModelError::InvalidOutput)?;
    if !out.rejected.is_empty()
        || out.dimensions != dimensions
        || out.vectors.len() != count
        || out.vectors.iter().any(|v| {
            v.len() != dimensions
                || v.iter().any(|n| !n.is_finite())
                || !v.iter().map(|n| n * n).sum::<f64>().is_finite()
                || v.iter().map(|n| n * n).sum::<f64>() <= 0.0
        })
    {
        return Err(ModelError::InvalidOutput);
    }
    Ok(out.vectors)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedding_shape_space_and_norm_are_not_guessed() {
        assert!(validate_vectors(r#"{"vectors":[[1,0]],"dimensions":2}"#, 1, 2).is_ok());
        assert!(validate_vectors(r#"{"vectors":[[0,0]],"dimensions":2}"#, 1, 2).is_err());
        assert!(validate_vectors(r#"{"vectors":[[1,0]],"dimensions":3}"#, 1, 2).is_err());
        assert!(validate_vectors(r#"{"vectors":[[1,0]],"dimensions":2}"#, 2, 2).is_err());
        assert!(
            validate_vectors(r#"{"vectors":[[1,0]],"dimensions":2,"rejected":[]}"#, 1, 2).is_ok()
        );
        assert!(validate_vectors(r#"{"vectors":[[1,0]],"dimensions":2,"rejected":[{"index":0,"code":"invalid_vector"}]}"#, 1, 2).is_err());
        assert!(
            validate_vectors(r#"{"vectors":[null],"dimensions":2,"rejected":[]}"#, 1, 2).is_err()
        );
        assert!(
            validate_vectors(
                r#"{"vectors":[[1,0]],"dimensions":2,"rejected":[],"invented":true}"#,
                1,
                2
            )
            .is_err()
        );
    }
}

async fn eligible_problem_entities(db: &Database, domain: Uuid) -> Result<Vec<Entity>, ModelError> {
    let mut entities = Vec::new();
    let problems:Vec<Uuid>=sqlx::query_scalar("SELECT problem_ref FROM linggan_ci_problem WHERE domain_ref=$1 AND redirect_ref IS NULL ORDER BY updated_at DESC,problem_ref LIMIT 501").bind(domain).fetch_all(db.pool()).await?;
    if problems.len() > 500 {
        return Err(ModelError::InputLimit);
    }
    for id in problems {
        let details =
            match problem_details(db, domain, id).await {
                Ok(details) => details,
                Err(linggan_storage_postgres::StorageError::Statement(sqlx::Error::Protocol(
                    code,
                ))) if code == "CI_PROBLEM_UNAVAILABLE" => continue,
                Err(_) => return Err(ModelError::Source),
            };
        let p = &details["problem"];
        let sample_refs: Vec<Uuid> = details["members"]
            .as_array()
            .into_iter()
            .flatten()
            .take(3)
            .filter_map(|m| {
                m["sourceRef"]
                    .as_str()
                    .and_then(|r| Uuid::parse_str(r).ok())
            })
            .collect();
        let guards:Vec<Value>=sqlx::query_scalar("SELECT COALESCE(a.result->'contextRefs',jsonb_build_object('researchSourceRefs',jsonb_build_array(s.source_ref))) FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) LEFT JOIN linggan_comment_analysis_work a ON a.work_ref=m.analysis_ref WHERE m.problem_ref=$1 AND s.source_ref=ANY($2)")
            .bind(id).bind(sample_refs).fetch_all(db.pool()).await?;
        entities.push(entity(
            "problem",
            id,
            p["name"].as_str().ok_or(ModelError::Source)?,
            p["meaning"].as_str().ok_or(ModelError::Source)?,
            p["definitionRevision"].as_i64().ok_or(ModelError::Source)?,
            guards,
        ));
    }
    Ok(entities)
}
