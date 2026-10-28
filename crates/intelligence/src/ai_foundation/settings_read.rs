//! Safe local configuration facts. No secrets, prompts, provider requests or fabricated health.
use super::store::FoundationError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};

pub async fn read_settings(db: &Database, synthetic: bool) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let connections:Value=sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(row ORDER BY row->>'connectionRef'),'[]') FROM (
          SELECT jsonb_build_object('connectionRef',c.connection_ref,'enabled',c.enabled,'revision',c.revision,
           'versionRef',v.version_ref,'name',v.name,'api',v.api,'baseUrl',v.base_url,'localEndpoint',v.local_endpoint,
           'endpointRef',p.endpoint_ref) AS row
          FROM linggan_model_connection c JOIN LATERAL(
           SELECT * FROM linggan_model_connection_version WHERE connection_ref=c.connection_ref ORDER BY revision DESC LIMIT 1
          )v ON true JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref
         )rows"
    ).fetch_one(&mut *tx).await?;
    let models:Value=sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(row ORDER BY row->>'identityRef'),'[]') FROM (
          SELECT jsonb_build_object('identityRef',i.identity_ref,'connectionRef',i.connection_ref,'modelId',i.model_id,
           'endpointRef',p.endpoint_ref,'modelRef',m.model_ref,'selectionModelRef',selection.model_ref,'connectionVersionRef',v.version_ref,
           'probe',CASE WHEN probe.invocation_ref IS NULL THEN NULL ELSE jsonb_build_object(
            'invocationRef',probe.invocation_ref,'state',probe.state,'modelCallable',probe.result->'modelCallable',
            'finishedAt',probe.finished_at,'failureCode',probe.failure_code) END,
           'capability',CASE WHEN cap.capability_ref IS NULL THEN NULL ELSE jsonb_build_object(
            'capabilityRef',cap.capability_ref,'source',cap.source_kind,'sourceVersion',cap.source_version,
            'probeInvocationRef',cap.probe_invocation_ref,'contextTokens',cap.context_tokens,
            'maxOutputTokens',cap.max_output_tokens,'maxTimeoutSeconds',cap.max_timeout_seconds,
            'thinkingLevels',cap.thinking_levels,'compat',cap.compat,'thinkingLevelMap',cap.thinking_level_map,
            'outputMode',cap.output_mode,'strictSchema',cap.strict_schema) END,
           'price',CASE WHEN price.price_ref IS NULL THEN NULL ELSE jsonb_build_object(
            'priceRef',price.price_ref,'currency',price.currency,'source',price.source_kind,'sourceVersion',price.source_version,
            'input',price.input_micro_usd_per_million::text,'output',price.output_micro_usd_per_million::text,
            'cachedRead',price.cached_read_micro_usd_per_million::text,'cachedWrite',price.cached_write_micro_usd_per_million::text) END) AS row
          FROM linggan_ai_model_identity i JOIN LATERAL(
           SELECT * FROM linggan_model_connection_version WHERE connection_ref=i.connection_ref ORDER BY revision DESC LIMIT 1
          )v ON true JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref
          LEFT JOIN linggan_model_entry m ON m.connection_version_ref=v.version_ref AND m.model_id=i.model_id
          LEFT JOIN LATERAL(SELECT selected.model_ref FROM linggan_model_entry selected
            JOIN linggan_model_connection_version sv ON sv.version_ref=selected.connection_version_ref
            WHERE sv.connection_ref=i.connection_ref AND selected.model_id=i.model_id
            ORDER BY sv.revision DESC LIMIT 1)selection ON true
          LEFT JOIN LATERAL(SELECT * FROM linggan_model_capability WHERE identity_ref=i.identity_ref AND endpoint_ref=p.endpoint_ref ORDER BY revision_sequence DESC LIMIT 1)cap ON true
          LEFT JOIN LATERAL(SELECT * FROM linggan_model_price WHERE identity_ref=i.identity_ref AND endpoint_ref=p.endpoint_ref ORDER BY revision_sequence DESC LIMIT 1)price ON true
          LEFT JOIN LATERAL(SELECT * FROM linggan_model_invocation WHERE connection_version_ref=v.version_ref AND model_ref=m.model_ref AND operation='probe' ORDER BY created_at DESC,invocation_ref DESC LIMIT 1)probe ON true
         )rows"
    ).fetch_one(&mut *tx).await?;
    let controls:Value=sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(jsonb_build_object('scope',c.scope_key,'taskKey',c.task_key,
          'enabled',c.enabled,'revision',c.revision,'actor',c.actor_kind,'reason',c.reason,'commandRef',c.command_ref,
          'updatedAt',c.updated_at,'label',t.label,'lifecycle',t.lifecycle,'automaticBrake',t.automatic_brake)
         ORDER BY c.scope_key),'[]') FROM linggan_ai_control c LEFT JOIN linggan_ai_task t USING(task_key)"
    ).fetch_one(&mut *tx).await?;
    let egress:Value=sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(jsonb_build_object('eventRef',e.event_ref,'endpointRef',e.endpoint_ref,
          'taskKey',e.task_key,'materialClass',e.material_class,'allowed',e.allowed,'actor',e.actor_kind,
          'reason',e.reason,'createdAt',e.created_at) ORDER BY e.endpoint_ref,e.task_key,e.material_class),'[]')
         FROM linggan_ai_current_egress e"
    ).fetch_one(&mut *tx).await?;
    let skills:Value=sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(row ORDER BY row->>'name'),'[]') FROM (
          SELECT jsonb_build_object('skillRef',s.skill_ref,'name',s.name,'versions',(
           SELECT jsonb_agg(jsonb_build_object('versionRef',v.version_ref,'versionNo',v.version_no,
            'description',v.description,'packageSha256',v.package_sha256,'sourceSha256',v.source_sha256,
            'disableModelInvocation',v.disable_model_invocation,'createdAt',v.created_at,
            'fileCount',(SELECT count(*) FROM linggan_skill_file WHERE version_ref=v.version_ref)) ORDER BY v.version_no DESC)
           FROM linggan_skill_version v WHERE v.skill_ref=s.skill_ref)) AS row FROM linggan_skill s
         )rows"
    ).fetch_one(&mut *tx).await?;
    let tasks:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(jsonb_build_object('taskKey',t.task_key,'label',t.label,
        'lifecycle',t.lifecycle,'requiresSkill',t.requires_skill,'skillAllowed',t.skill_allowed,'handlerContract',t.handler_contract,
        'revision',s.revision,'configuration',p.configuration,'bindingRevision',b.revision,'activeSkillVersionRef',b.current_version_ref,
        'approvalSource',b.approval_source,'approvalReason',b.reason) ORDER BY t.task_key),'[]')
        FROM linggan_ai_task t JOIN linggan_ai_task_setting s USING(task_key)
        JOIN linggan_ai_config_projection p ON p.task_config_ref=s.current_config_ref
        JOIN linggan_skill_binding b ON b.task_key=t.task_key")
        .fetch_one(&mut *tx).await?;
    let configurations:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(configuration ORDER BY config_sequence DESC),'[]') FROM (
        SELECT p.*,row_number() OVER(PARTITION BY p.task_key ORDER BY config_sequence DESC) AS rank,
        p.task_config_ref=s.current_config_ref AS current FROM linggan_ai_config_projection p JOIN linggan_ai_task_setting s USING(task_key)
        )configs WHERE rank<=10 OR current")
        .fetch_one(&mut *tx).await?;
    let trials:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(row ORDER BY created_at DESC),'[]') FROM (
        SELECT comparison.created_at,jsonb_build_object('comparisonRef',comparison.comparison_ref,'taskKey',comparison.task_key,
         'baselineBatchRef',comparison.baseline_batch_ref,'candidateBatchRef',comparison.candidate_batch_ref,
         'baselineState',baseline.state,'candidateState',candidate.state,'candidateConfigRef',b.task_config_ref,
         'itemCount',b.item_count,'inputsSha256',b.inputs_sha256,'createdAt',comparison.created_at) AS row
        FROM linggan_ai_trial_comparison comparison JOIN linggan_ai_batch b ON b.batch_ref=comparison.candidate_batch_ref
        JOIN linggan_ai_batch_control candidate ON candidate.batch_ref=b.batch_ref
        LEFT JOIN linggan_ai_batch_control baseline ON baseline.batch_ref=comparison.baseline_batch_ref
        ORDER BY comparison.created_at DESC,comparison.comparison_ref DESC LIMIT 50)trials")
        .fetch_one(&mut *tx).await?;
    let execution = super::executor::read::snapshot_in(&mut tx).await?;
    let batches = super::batch_control::recent_in(&mut tx).await?;
    tx.commit().await?;
    Ok(json!({"contract":"ai-settings.v1","synthetic":synthetic,
        "connections":connections,"models":models,"controls":controls,"egress":egress,"skills":skills,
        "tasks":tasks,"configurations":configurations,"trials":trials,"execution":execution,"batches":batches,
        "resources":&*linggan_contracts::ai_limits::AI_LIMITS,"protocol":&*linggan_contracts::ai_protocol::PROTOCOL}))
}

pub async fn read_configuration(
    db: &Database,
    config: uuid::Uuid,
) -> Result<Value, FoundationError> {
    sqlx::query_scalar(
        "SELECT configuration FROM linggan_ai_config_projection WHERE task_config_ref=$1",
    )
    .bind(config)
    .fetch_optional(db.pool())
    .await?
    .ok_or(FoundationError::NotFound)
}

pub async fn read_comparison(
    db: &Database,
    comparison: uuid::Uuid,
    position: u32,
) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let mut comparison:Value=sqlx::query_scalar("SELECT jsonb_build_object('comparisonRef',comparison.comparison_ref,'taskKey',comparison.task_key,
        'position',c.position,'itemCount',candidate.item_count,'inputRef',c.input_ref,'inputsSha256',candidate.inputs_sha256,
        'baseline',CASE WHEN baseline.batch_ref IS NULL THEN NULL ELSE jsonb_build_object('batchRef',baseline.batch_ref,
          'taskConfigRef',baseline.task_config_ref,'state',bs.state,'itemState',bc.state,'attemptCount',bc.attempt_count,
          'output',bc.output,'failureCode',bc.failure_code,'executionContract',bc.execution_contract,'skillVersionRef',bc.executed_skill_version_ref,'invocationRef',bc.invocation_ref) END,
        'candidate',jsonb_build_object('batchRef',candidate.batch_ref,'taskConfigRef',candidate.task_config_ref,
          'state',cs.state,'itemState',cc.state,'attemptCount',cc.attempt_count,'output',cc.output,'failureCode',cc.failure_code,
          'executionContract',cc.execution_contract,'skillVersionRef',cc.executed_skill_version_ref,'invocationRef',cc.invocation_ref),
        'qualifiesForActivation',linggan_ai_trial_qualifies(comparison.comparison_ref,comparison.task_key,config.skill_version_ref,config.task_config_ref))
        FROM linggan_ai_trial_comparison comparison JOIN linggan_ai_batch candidate ON candidate.batch_ref=comparison.candidate_batch_ref
        JOIN linggan_ai_task_config config ON config.task_config_ref=candidate.task_config_ref
        JOIN linggan_ai_batch_control cs ON cs.batch_ref=candidate.batch_ref
        JOIN linggan_ai_batch_item c ON c.batch_ref=candidate.batch_ref AND c.position=$2
        JOIN linggan_ai_batch_item_control cc ON cc.item_ref=c.item_ref
        LEFT JOIN linggan_ai_batch baseline ON baseline.batch_ref=comparison.baseline_batch_ref
        LEFT JOIN linggan_ai_batch_control bs ON bs.batch_ref=baseline.batch_ref
        LEFT JOIN linggan_ai_batch_item b ON b.batch_ref=baseline.batch_ref AND b.position=c.position
        LEFT JOIN linggan_ai_batch_item_control bc ON bc.item_ref=b.item_ref WHERE comparison.comparison_ref=$1")
        .bind(comparison).bind(position as i64).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let input =
        serde_json::from_value::<super::batch_store::InputRef>(comparison["inputRef"].clone())
            .map_err(|_| FoundationError::Invalid)?;
    let (readable, qualification) =
        if let super::batch_store::InputRef::WritingPreview { preview_ref } = input {
            let connected: bool =
                sqlx::query_scalar("SELECT to_regclass('linggan_creation_ai_preview') IS NOT NULL")
                    .fetch_one(&mut *tx)
                    .await?;
            if !connected {
                return Err(FoundationError::NotConnected);
            }
            match crate::content_workbench_ai::execution::readable_preview_in(&mut tx, preview_ref)
                .await
            {
                Ok(readable) => (readable, if readable { "readable" } else { "unavailable" }),
                Err(crate::content_workbench_ai::AiError::Database(error)) => {
                    return Err(FoundationError::Database(error));
                }
                Err(_) => (false, "unavailable"),
            }
        } else {
            (false, "not_connected")
        };
    comparison["sourceQualification"] = serde_json::json!(qualification);
    if !readable {
        for side in ["baseline", "candidate"] {
            if comparison[side].is_object() {
                comparison[side]["output"] = Value::Null;
            }
        }
    }
    tx.commit().await?;
    Ok(comparison)
}

pub async fn read_skill_version(
    db: &Database,
    version: uuid::Uuid,
) -> Result<Value, FoundationError> {
    sqlx::query_scalar("SELECT jsonb_build_object('versionRef',v.version_ref,'skillRef',v.skill_ref,'name',v.directory_name,
      'versionNo',v.version_no,'description',v.description,'body',v.body,'frontmatter',v.frontmatter,
      'packageSha256',v.package_sha256,'sourceSha256',v.source_sha256,'loaderVersion',v.loader_version,
      'disableModelInvocation',v.disable_model_invocation,
      'files',(SELECT jsonb_agg(jsonb_build_object('path',path,'sha256',sha256,'bytes',octet_length(bytes),
        'content',text_content,'contentBase64',CASE WHEN text_content IS NULL THEN replace(encode(bytes,'base64'),E'\\n','') ELSE NULL END,
        'kind',CASE WHEN text_content IS NULL THEN 'binary' ELSE 'utf8' END) ORDER BY path COLLATE \"C\")
       FROM linggan_skill_file WHERE version_ref=v.version_ref)) FROM linggan_skill_version v WHERE version_ref=$1")
      .bind(version).fetch_optional(db.pool()).await?.ok_or(FoundationError::NotFound)
}
