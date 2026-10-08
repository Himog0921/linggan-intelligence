//! Bounded local scan and leased semantic execution, using the existing model adapter/ledger.
use crate::{
    creator_discovery_analysis::{FOCUS_SYSTEM, SYSTEM, validate_focus, validate_work},
    model_invocation::{connection_request, finish_invocation},
    model_secrets::ModelSecretStore,
    model_settings::{
        ModelError, research_model_semantically_ready_in_transaction,
        reserve_research_model_semantic_dispatch_permit,
    },
    pi_adapter::PiAdapter,
};
use linggan_evidence::creator_discovery::{self as discovery, DiscoveryWork, Fragment};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

// UTF-8 bytes conservatively bound the adapter's input tokens, including CJK text.
const INPUT_ENVELOPE_BYTES: usize = 128;

fn read_error(e: discovery::DiscoveryError) -> ModelError {
    match e {
        discovery::DiscoveryError::Database(e) => ModelError::Database(e),
        _ => ModelError::Source,
    }
}
fn scope(domain: Uuid) -> linggan_contracts::creator_discovery::CreatorScope {
    serde_json::from_value(json!({"domain":domain})).expect("fixed valid scope")
}

async fn recover_expired(db: &Database, table: &str, kind: &str) -> Result<(), ModelError> {
    // The table name is selected from two fixed private identifiers by this module.
    let sql = format!(
        r#"WITH expired AS (SELECT domain_ref,lease_token FROM {table} WHERE job_state='running' AND lease_expires_at<scope_001_now() FOR UPDATE), recovered AS (UPDATE {table} t SET job_state=CASE WHEN attempt_count>=3 THEN 'failed' ELSE 'queued' END,lease_token=NULL,lease_expires_at=NULL,last_error_code='lease_expired',next_attempt_at=scope_001_now(),updated_at=scope_001_now() FROM expired e WHERE t.domain_ref=e.domain_ref AND t.lease_token=e.lease_token RETURNING e.domain_ref,e.lease_token) UPDATE linggan_model_invocation i SET state='failed',failure_code='lease_expired',finished_at=scope_001_now(),result=COALESCE(i.result,'{{}}'::jsonb)||'{{"recovery":"lease_expired; provider charge unknown"}}'::jsonb FROM recovered e WHERE i.state='running' AND i.result->>'purpose'='creator_discovery' AND i.result->>'jobKind'=$1 AND i.result->>'domainRef'=e.domain_ref::text AND i.result->>'jobLease'=e.lease_token::text"#
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(kind)
        .execute(db.pool())
        .await?;
    // A changed source can replace the lease before the old process exits. If that
    // process never returns, close its reservation after the lease window too.
    let orphan_sql = format!(
        "UPDATE linggan_model_invocation i SET state='failed',failure_code='lease_superseded',finished_at=scope_001_now(),result=COALESCE(i.result,'{{}}'::jsonb)||'{{\"recovery\":\"source changed; provider charge unknown\"}}'::jsonb WHERE i.state='running' AND i.created_at<scope_001_now()-interval '5 minutes' AND i.result->>'purpose'='creator_discovery' AND i.result->>'jobKind'=$1 AND NOT EXISTS (SELECT 1 FROM {table} a WHERE a.domain_ref::text=i.result->>'domainRef' AND a.lease_token::text=i.result->>'jobLease' AND a.job_state='running')"
    );
    sqlx::query(sqlx::AssertSqlSafe(orphan_sql))
        .bind(kind)
        .execute(db.pool())
        .await?;
    Ok(())
}

async fn finish_without_dispatch(
    db: &Database,
    invocation: Uuid,
    code: &str,
) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_model_invocation SET state='failed',failure_code=$2,charged_tokens=0,finished_at=scope_001_now(),result=COALESCE(result,'{}'::jsonb)||'{\"dispatchStarted\":false}'::jsonb WHERE invocation_ref=$1 AND state='running'")
        .bind(invocation).bind(code).execute(db.pool()).await?;
    Ok(())
}

pub async fn run_once(
    db: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    let ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_creator_discovery_policy') IS NOT NULL")
            .fetch_one(db.pool())
            .await?;
    if !ready {
        return Ok(false);
    }
    // Recovery keeps any charged reservation; a timeout does not imply no provider cost.
    recover_expired(db, "linggan_creator_discovery_work_analysis", "work").await?;
    recover_expired(db, "linggan_creator_discovery_author_analysis", "focus").await?;
    for table in [
        "linggan_creator_discovery_work_analysis",
        "linggan_creator_discovery_author_analysis",
    ] {
        // Private static identifiers, never input text.
        let sql = format!(
            "UPDATE {table} SET job_state='queued',next_attempt_at=scope_001_now(),last_error_code=NULL WHERE job_state='paused' AND last_error_code='model_budget_exhausted' AND updated_at < date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(db.pool())
            .await?;
    }
    let policies=sqlx::query("SELECT p.domain_ref,p.scan_after FROM linggan_creator_discovery_policy p JOIN observation_domain d USING(domain_ref) WHERE p.analysis_enabled AND d.status='active' ORDER BY p.updated_at,p.domain_ref").fetch_all(db.pool()).await?;
    for p in policies.iter().take(1) {
        let domain: Uuid = p.get("domain_ref");
        let after: Option<Uuid> = p.get("scan_after");
        let mut q = scope(domain);
        let mut data = discovery::load(db, &q).await.map_err(read_error)?;
        q.usage_role = "reference".into();
        let reference = discovery::load(db, &q).await.map_err(read_error)?;
        data.works.extend(reference.works);
        data.works.sort_by_key(|w| w.work_ref);
        data.works.dedup_by_key(|w| w.work_ref);
        let selected: Vec<_> = data
            .works
            .iter()
            .filter(|w| after.is_none_or(|a| w.work_ref > a))
            .take(50)
            .collect();
        let mut tx = db.pool().begin().await?;
        for w in &selected {
            sqlx::query("INSERT INTO linggan_creator_discovery_work_analysis(domain_ref,work_public_ref,requested_fingerprint) VALUES($1,$2,$3) ON CONFLICT(domain_ref,work_public_ref) DO UPDATE SET requested_fingerprint=EXCLUDED.requested_fingerprint,job_state='queued',attempt_count=0,lease_token=NULL,lease_expires_at=NULL,last_error_code=NULL,next_attempt_at=scope_001_now(),updated_at=scope_001_now() WHERE linggan_creator_discovery_work_analysis.requested_fingerprint<>EXCLUDED.requested_fingerprint").bind(domain).bind(w.work_ref).bind(&w.fingerprint).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE linggan_creator_discovery_policy SET scan_after=$2,updated_at=scope_001_now() WHERE domain_ref=$1 AND platform='xhs'").bind(domain).bind(if selected.len()==50{selected.last().map(|w|w.work_ref)}else{None}).execute(&mut *tx).await?;
        tx.commit().await?;
        let authors: std::collections::HashSet<_> = data
            .works
            .iter()
            .filter_map(|w| w.author_external_id.clone())
            .collect();
        for author in authors {
            let works: Vec<_> = data
                .works
                .iter()
                .filter(|w| w.author_external_id.as_deref() == Some(&author))
                .collect();
            if !works.is_empty() {
                queue_focus(
                    db,
                    domain,
                    works[0],
                    &works,
                    data.policy["configRef"]
                        .as_str()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        .ok_or(ModelError::Invalid)?,
                )
                .await?;
            }
        }
    }
    let lease = Uuid::new_v4();
    let mut tx = db.pool().begin().await?;
    let row=sqlx::query("SELECT a.domain_ref,a.work_public_ref,a.requested_fingerprint,p.config_ref FROM linggan_creator_discovery_work_analysis a JOIN linggan_creator_discovery_policy p ON p.domain_ref=a.domain_ref AND p.platform='xhs' JOIN observation_domain d ON d.domain_ref=a.domain_ref WHERE a.job_state='queued' AND a.attempt_count<3 AND a.next_attempt_at<=scope_001_now() AND p.analysis_enabled AND d.status='active' ORDER BY a.next_attempt_at,a.domain_ref,a.work_public_ref FOR UPDATE OF a SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.commit().await?;
        return run_focus(db, secrets, adapter).await;
    };
    let domain: Uuid = row.get("domain_ref");
    let work: Uuid = row.get("work_public_ref");
    let config: Uuid = row.get("config_ref");
    let fingerprint: String = row.get("requested_fingerprint");
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state='running',attempt_count=attempt_count+1,lease_token=$3,lease_expires_at=scope_001_now()+interval '5 minutes' WHERE domain_ref=$1 AND work_public_ref=$2").bind(domain).bind(work).bind(lease).execute(&mut *tx).await?;
    tx.commit().await?;
    let result = execute(
        db,
        secrets,
        adapter,
        domain,
        work,
        config,
        &fingerprint,
        lease,
    )
    .await;
    let mut tx = db.pool().begin().await?;
    match result {
        Ok(Some((output, invocation))) => {
            sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET result_json=$5,result_fingerprint=$4,result_rule_version=$6,result_model_config_ref=$7,result_invocation_ref=$8,result_at=scope_001_now(),job_state='idle',lease_token=NULL,lease_expires_at=NULL,last_error_code=NULL,updated_at=scope_001_now() WHERE domain_ref=$1 AND work_public_ref=$2 AND lease_token=$3 AND requested_fingerprint=$4").bind(domain).bind(work).bind(lease).bind(&fingerprint).bind(output).bind(discovery::RULE_VERSION).bind(config).bind(invocation).execute(&mut *tx).await?;
        }
        Ok(None) => {
            sqlx::query("DELETE FROM linggan_creator_discovery_work_analysis WHERE domain_ref=$1 AND work_public_ref=$2 AND lease_token=$3 AND result_at IS NULL AND manual_overrides='{}'::jsonb").bind(domain).bind(work).bind(lease).execute(&mut *tx).await?;
        }
        Err(error) => {
            let paused = is_paused(&error);
            sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state=CASE WHEN $4 THEN 'paused' WHEN attempt_count>=3 THEN 'failed' ELSE 'queued' END,attempt_count=CASE WHEN $4 THEN greatest(attempt_count-1,0) ELSE attempt_count END,lease_token=NULL,lease_expires_at=NULL,last_error_code=$5,next_attempt_at=scope_001_now()+interval '1 minute',updated_at=scope_001_now() WHERE domain_ref=$1 AND work_public_ref=$2 AND lease_token=$3").bind(domain).bind(work).bind(lease).bind(paused).bind(error.code()).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(true)
}
async fn execute(
    db: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    domain: Uuid,
    work: Uuid,
    config: Uuid,
    fingerprint: &str,
    lease: Uuid,
) -> Result<Option<(Value, Option<Uuid>)>, ModelError> {
    let mut q = scope(domain);
    let mut data = discovery::load(db, &q).await.map_err(read_error)?;
    q.usage_role = "reference".into();
    data.works
        .extend(discovery::load(db, &q).await.map_err(read_error)?.works);
    data.works.sort_by_key(|w| w.work_ref);
    data.works.dedup_by_key(|w| w.work_ref);
    let Some(w) = data
        .works
        .iter()
        .find(|w| w.work_ref == work && w.fingerprint == fingerprint)
    else {
        return Err(ModelError::Source);
    };
    if w.fragments.is_empty() {
        return Ok(Some((validate_work(&json!({}), &[]), None)));
    }
    let input_budget: i32 = sqlx::query_scalar(
        "SELECT input_token_limit FROM linggan_model_config WHERE config_ref=$1",
    )
    .bind(config)
    .fetch_one(db.pool())
    .await?;
    let mut fragments = w.fragments.clone();
    let input = loop {
        let input = json!({"domain":data.domain,"workRef":work,"sourceFingerprint":fingerprint,"fragments":fragments,"acquisitionContext":{"kind":w.acquisition_kind,"selectionBiased":true}});
        if input.to_string().len() + SYSTEM.len() + INPUT_ENVELOPE_BYTES <= input_budget as usize {
            break input;
        }
        let Some(longest) = fragments.iter_mut().max_by_key(|f| f.text.chars().count()) else {
            return Err(ModelError::InputLimit);
        };
        let len = longest.text.chars().count();
        if len <= 1 {
            return Err(ModelError::InputLimit);
        }
        longest.text = longest.text.chars().take((len / 2).max(1)).collect();
        longest.end = longest.start + longest.text.chars().count();
    };
    let (raw, invocation) = call(
        db,
        secrets,
        adapter,
        domain,
        config,
        SYSTEM,
        &input,
        "work",
        work.to_string(),
        lease,
        fingerprint,
    )
    .await?;
    let output = validate_work(&raw, &fragments);
    // Author focus uses already available summaries; limited samples remain explicitly limited.
    if let Some(author) = &w.author_external_id {
        let related: Vec<_> = data
            .works
            .iter()
            .filter(|x| x.author_external_id.as_ref() == Some(author))
            .collect();
        if related.len() >= 2 && w.profile["biography"].as_str().is_some() {
            queue_focus(db, domain, w, &related, config).await?;
        }
    }
    Ok(Some((output, Some(invocation))))
}
async fn queue_focus(
    db: &Database,
    domain: Uuid,
    w: &DiscoveryWork,
    works: &[&DiscoveryWork],
    config: Uuid,
) -> Result<(), ModelError> {
    let fp = discovery::focus_fingerprint(&w.profile, works, &json!(config));
    sqlx::query("INSERT INTO linggan_creator_discovery_author_analysis(domain_ref,platform,author_external_id,requested_fingerprint) VALUES($1,$2,$3,$4) ON CONFLICT(domain_ref,platform,author_external_id) DO UPDATE SET requested_fingerprint=$4,job_state='queued',attempt_count=0,lease_token=NULL,lease_expires_at=NULL,last_error_code=NULL WHERE linggan_creator_discovery_author_analysis.requested_fingerprint<>$4").bind(domain).bind(&w.platform).bind(&w.author_external_id).bind(fp).execute(db.pool()).await?;
    Ok(())
}
async fn call(
    db: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    domain: Uuid,
    config: Uuid,
    system: &str,
    input: &Value,
    job_kind: &str,
    job_key: String,
    lease: Uuid,
    fingerprint: &str,
) -> Result<(Value, Uuid), ModelError> {
    let mut permit = reserve_research_model_semantic_dispatch_permit(db, config).await?;
    let row=sqlx::query("SELECT c.*,m.connection_version_ref,m.model_id FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$1").bind(config).fetch_one(db.pool()).await?;
    let mut request = connection_request(db, secrets, row.get("connection_version_ref")).await?;
    request.operation = "analyze".into();
    request.model_id = row.get("model_id");
    request.system = system.into();
    request.prompt = input.to_string();
    request.timeout_ms = row.get::<i32, _>("timeout_seconds") as u64 * 1000;
    request.max_output_tokens = row.get::<i32, _>("output_token_limit");
    let input_budget = row.get::<i32, _>("input_token_limit") as usize;
    if request.prompt.len() + request.system.len() + INPUT_ENVELOPE_BYTES > input_budget {
        return Err(ModelError::InputLimit);
    }
    let reserved = (input_budget + request.max_output_tokens as usize) as i64;
    let invocation = Uuid::new_v4();
    let mut tx = db.pool().begin().await?;
    let enabled:bool=sqlx::query_scalar("SELECT p.analysis_enabled AND p.config_ref=$2 AND d.status='active' FROM linggan_creator_discovery_policy p JOIN observation_domain d USING(domain_ref) WHERE p.domain_ref=$1 AND p.platform='xhs' FOR UPDATE OF p FOR SHARE OF d").bind(domain).bind(config).fetch_one(&mut *tx).await?;
    if !enabled {
        return Err(ModelError::Disabled);
    }
    // Serialize this purpose budget with its policy; charge through the shared invocation ledger.
    sqlx::query("SELECT config_ref FROM linggan_model_config WHERE config_ref=$1 FOR UPDATE")
        .bind(config)
        .fetch_one(&mut *tx)
        .await?;
    let charged:i64=sqlx::query_scalar("SELECT COALESCE(sum(charged_tokens),0)::bigint FROM linggan_model_invocation WHERE result->>'purpose'='creator_discovery' AND result->>'domainRef'=$1::text AND created_at>=date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'").bind(domain.to_string()).fetch_one(&mut *tx).await?;
    let daily_limit:i64=sqlx::query_scalar("SELECT daily_token_limit FROM linggan_creator_discovery_policy WHERE domain_ref=$1 AND platform='xhs'").bind(domain).fetch_one(&mut *tx).await?;
    if charged + reserved > daily_limit {
        return Err(ModelError::Budget);
    }
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,jsonb_build_object('purpose','creator_discovery','domainRef',$7::text,'jobKind',$8::text,'jobKey',$9::text,'jobLease',$10::text,'inputFingerprint',$11::text))").bind(invocation).bind(row.get::<Uuid,_>("connection_version_ref")).bind(row.get::<Uuid,_>("model_ref")).bind(config).bind(discovery::hash(&input.to_string())).bind(reserved).bind(domain.to_string()).bind(job_kind).bind(&job_key).bind(lease.to_string()).bind(fingerprint).execute(&mut *tx).await?;
    tx.commit().await?;
    let mut dispatch = permit.begin().await?;
    let ready = research_model_semantically_ready_in_transaction(&mut dispatch, config).await;
    let admission=match ready {
        Ok(true) => sqlx::query_scalar::<_,bool>("SELECT p.analysis_enabled AND p.config_ref=$2 AND d.status='active' FROM linggan_creator_discovery_policy p JOIN observation_domain d USING(domain_ref) WHERE p.domain_ref=$1 AND p.platform='xhs' FOR SHARE OF p,d").bind(domain).bind(config).fetch_one(&mut *dispatch).await.map_err(ModelError::from).and_then(|enabled|if enabled {Ok(())}else{Err(ModelError::Disabled)}),
        Ok(false) => Err(ModelError::NotQualified),
        Err(error) => Err(error),
    };
    if let Err(error) = admission {
        drop(dispatch);
        let _ = permit.release().await;
        finish_without_dispatch(db, invocation, error.code()).await?;
        return Err(error);
    }
    let current: bool = if job_kind == "work" {
        sqlx::query("SELECT 1 FROM linggan_creator_discovery_work_analysis WHERE domain_ref=$1 AND work_public_ref=$2::uuid AND lease_token=$3 AND requested_fingerprint=$4 AND job_state='running' AND lease_expires_at>scope_001_now() FOR SHARE").bind(domain).bind(&job_key).bind(lease).bind(fingerprint).fetch_optional(&mut *dispatch).await?.is_some()
    } else {
        sqlx::query("SELECT 1 FROM linggan_creator_discovery_author_analysis WHERE domain_ref=$1 AND platform='xhs' AND author_external_id=$2 AND lease_token=$3 AND requested_fingerprint=$4 AND job_state='running' AND lease_expires_at>scope_001_now() FOR SHARE").bind(domain).bind(&job_key).bind(lease).bind(fingerprint).fetch_optional(&mut *dispatch).await?.is_some()
    };
    if !current {
        drop(dispatch);
        let _ = permit.release().await;
        finish_without_dispatch(db, invocation, "creator_source_changed").await?;
        return Err(ModelError::Source);
    }
    // This committed marker is the dispatch admission point. The provider call
    // follows outside the transaction; a crash in between remains cost-unknown.
    let admitted=sqlx::query("UPDATE linggan_model_invocation SET result=result||jsonb_build_object('dispatchAdmittedAt',scope_001_now()::text) WHERE invocation_ref=$1 AND state='running'").bind(invocation).execute(&mut *dispatch).await?;
    if admitted.rows_affected() != 1 {
        return Err(ModelError::Source);
    }
    dispatch.commit().await?;
    let response = adapter.call(&request).await;
    let raw = response
        .as_ref()
        .ok()
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object);
    let ok = raw.is_some();
    let code = if ok {
        None
    } else {
        Some("creator_analysis_invalid_output")
    };
    finish_invocation(db,invocation,response.as_ref().ok(),ok,code,&json!({"ok":ok,"purpose":"creator_discovery","domainRef":domain,"ruleVersion":discovery::RULE_VERSION,"failureCode":code})).await?;
    permit.release().await?;
    Ok((raw.ok_or(ModelError::InvalidOutput)?, invocation))
}

async fn run_focus(
    db: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    let mut tx = db.pool().begin().await?;
    let row=sqlx::query("SELECT a.domain_ref,a.platform,a.author_external_id,a.requested_fingerprint,p.config_ref FROM linggan_creator_discovery_author_analysis a JOIN linggan_creator_discovery_policy p USING(domain_ref,platform) JOIN observation_domain d USING(domain_ref) WHERE a.job_state='queued' AND a.attempt_count<3 AND a.next_attempt_at<=scope_001_now() AND p.analysis_enabled AND d.status='active' ORDER BY a.next_attempt_at,a.domain_ref FOR UPDATE OF a SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(false);
    };
    let domain: Uuid = row.get("domain_ref");
    let author: String = row.get("author_external_id");
    let fingerprint: String = row.get("requested_fingerprint");
    let config: Uuid = row.get("config_ref");
    let lease = Uuid::new_v4();
    sqlx::query("UPDATE linggan_creator_discovery_author_analysis SET job_state='running',attempt_count=attempt_count+1,lease_token=$3,lease_expires_at=scope_001_now()+interval '5 minutes' WHERE domain_ref=$1 AND platform='xhs' AND author_external_id=$2").bind(domain).bind(&author).bind(lease).execute(&mut *tx).await?;
    tx.commit().await?;
    let result=async{
  let mut q=scope(domain);let mut data=discovery::load(db,&q).await.map_err(read_error)?;q.usage_role="reference".into();data.works.extend(discovery::load(db,&q).await.map_err(read_error)?.works);data.works.sort_by_key(|w|w.work_ref);data.works.dedup_by_key(|w|w.work_ref);
  let mut works:Vec<_>=data.works.iter().filter(|w|w.author_external_id.as_deref()==Some(&author)).collect();works.sort_by(|a,b|a.published_date.cmp(&b.published_date).then_with(||a.work_ref.cmp(&b.work_ref)));
  if works.is_empty() || discovery::focus_fingerprint(&works[0].profile,&works,&json!(config))!=fingerprint {return Err(ModelError::Source);}
  let input_budget:i32=sqlx::query_scalar("SELECT input_token_limit FROM linggan_model_config WHERE config_ref=$1").bind(config).fetch_one(db.pool()).await?;
  let mut fragments:Vec<Fragment>=Vec::new();let mut summaries=Vec::new();let step=works.len().div_ceil(20).max(1);
  let biography=works.first().and_then(|w|discovery::fragment(w.work_ref,"biography",w.profile["biography"].as_str(),w.profile["sourceRef"].as_str().and_then(|s|Uuid::parse_str(s).ok()),None));
  if let Some(mut bio)=biography {
    bio.text=bio.text.chars().take(600).collect();bio.end=bio.text.chars().count();
    loop {
      let trial=json!({"domain":data.domain,"works":[],"fragments":[bio],"sampleBasis":"limited_domain_sample"});
      if trial.to_string().len()+FOCUS_SYSTEM.len()+INPUT_ENVELOPE_BYTES<=input_budget as usize {fragments.push(bio);break;}
      let len=bio.text.chars().count();if len<=1 {break;}
      bio.text=bio.text.chars().take(len/2).collect();bio.end=bio.start+bio.text.chars().count();
    }
  }
  let mut related=std::collections::HashMap::new();let mut broad=std::collections::HashSet::new();
  for w in works.iter().step_by(step).take(20){
    let mut chosen:Vec<_>=w.fragments.iter().take(2).cloned().collect();
    for f in &mut chosen {f.text=f.text.chars().take(240).collect();f.end=f.text.chars().count();}
    let summary=loop {
      if chosen.is_empty() {break None;}
      let summary=json!({"workRef":w.work_ref,"relevance":w.relevance,"topicHints":w.topic_hints,"sourceKind":w.acquisition_kind,"fragmentIds":chosen.iter().map(|f|&f.fragment_id).collect::<Vec<_>>()});
      let mut trial_fragments=fragments.clone();trial_fragments.extend(chosen.clone());let mut trial_summaries=summaries.clone();trial_summaries.push(summary.clone());
      let trial=json!({"domain":data.domain,"works":trial_summaries,"fragments":trial_fragments,"sampleBasis":"profile_discovery_sample"});
      if trial.to_string().len()+FOCUS_SYSTEM.len()+INPUT_ENVELOPE_BYTES<=input_budget as usize {break Some(summary);}
      let Some(longest)=chosen.iter_mut().max_by_key(|f|f.text.chars().count()) else {break None};
      let len=longest.text.chars().count();
      if len<=1 {chosen.pop();continue;}
      longest.text=longest.text.chars().take(len/2).collect();longest.end=longest.start+longest.text.chars().count();
    };
    let Some(summary)=summary else {continue;};
    if w.relevance=="related" {for f in &chosen{related.insert(f.fragment_id.clone(),w.work_ref);}}
    if w.acquisition_kind=="profile_discovery" {broad.insert(w.work_ref);}
    fragments.extend(chosen);summaries.push(summary);
  }
  let input=json!({"domain":data.domain,"works":summaries,"fragments":fragments,"sampleBasis":if broad.len()>=2{"profile_discovery_sample"}else{"limited_domain_sample"}});
  if fragments.is_empty() {return Ok::<_,ModelError>((validate_focus(&json!({}),&fragments,&related,&broad),None));}
  let (raw,invocation)=call(db,secrets,adapter,domain,config,FOCUS_SYSTEM,&input,"focus",author.clone(),lease,&fingerprint).await?;
  Ok::<_,ModelError>((validate_focus(&raw,&fragments,&related,&broad),Some(invocation)))
 }.await;
    match result {
        Ok((result, invocation)) => {
            sqlx::query("UPDATE linggan_creator_discovery_author_analysis SET result_json=$5,result_fingerprint=$4,result_rule_version=$6,result_model_config_ref=$7,result_invocation_ref=$8,result_at=scope_001_now(),job_state='idle',lease_token=NULL,lease_expires_at=NULL,last_error_code=NULL WHERE domain_ref=$1 AND platform='xhs' AND author_external_id=$2 AND lease_token=$3 AND requested_fingerprint=$4").bind(domain).bind(&author).bind(lease).bind(&fingerprint).bind(result).bind(discovery::RULE_VERSION).bind(config).bind(invocation).execute(db.pool()).await?;
        }
        Err(e) => {
            sqlx::query("UPDATE linggan_creator_discovery_author_analysis SET job_state=CASE WHEN $5 THEN 'paused' WHEN attempt_count>=3 THEN 'failed' ELSE 'queued' END,attempt_count=CASE WHEN $5 THEN greatest(attempt_count-1,0) ELSE attempt_count END,lease_token=NULL,lease_expires_at=NULL,last_error_code=$4,next_attempt_at=scope_001_now()+interval '1 minute',updated_at=scope_001_now() WHERE domain_ref=$1 AND platform='xhs' AND author_external_id=$2 AND lease_token=$3").bind(domain).bind(author).bind(lease).bind(e.code()).bind(is_paused(&e)).execute(db.pool()).await?;
        }
    }
    Ok(true)
}

fn is_paused(e: &ModelError) -> bool {
    matches!(
        e,
        ModelError::Disabled
            | ModelError::Budget
            | ModelError::SecretUnavailable
            | ModelError::NotQualified
    )
}
