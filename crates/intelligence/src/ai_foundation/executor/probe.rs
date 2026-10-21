//! Explicit synthetic/catalog probes through the same fenced queue and typed ledger as business work.
use super::{ExecutorError, queue::ClaimedItem};
use crate::{
    ai_foundation::{
        TaskKey, admission,
        batch_store::{InputRef, ProbeOperation},
        store::FoundationError,
        transport::ResponseSchema,
        transport_store,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PI_PROTOCOL, PiAdapter, PiRequest, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

struct Prepared {
    invocation: Uuid,
    request: PiRequest,
    tested_profile: Value,
}

async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::Probe {
        connection_version_ref,
        model_ref,
        operation,
        ..
    } = item.input
    else {
        return Err(ExecutorError::Invalid);
    };
    if item.task_key != TaskKey::ModelProbe {
        return Err(ExecutorError::Invalid);
    }
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
        .await?;
    admission::check_queue_input_scope(&mut tx, item.task_key, item.task_config_ref, &item.input)
        .await?;
    // The immutable input may name an older key version at the same route. Use the current key,
    // record that exact version, and never inherit a former version's callability fact.
    let row=sqlx::query("SELECT v.*,route.endpoint_ref,m.model_id FROM linggan_model_connection_version original
        JOIN LATERAL(SELECT * FROM linggan_model_connection_version WHERE connection_ref=original.connection_ref ORDER BY revision DESC LIMIT 1)v ON true
        JOIN linggan_ai_connection_endpoint route ON route.version_ref=v.version_ref
        LEFT JOIN linggan_model_entry m ON m.model_ref=$2 WHERE original.version_ref=$1")
        .bind(connection_version_ref).bind(model_ref).fetch_one(&mut *tx).await?;
    let current_version: Uuid = row.get("version_ref");
    let model_id: Option<String> = row.get("model_id");
    let current_model = if let Some(model_id) = &model_id {
        sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,$3,'manual') ON CONFLICT(connection_version_ref,model_id) DO NOTHING")
            .bind(Uuid::new_v4()).bind(current_version).bind(model_id).execute(&mut *tx).await?;
        Some(sqlx::query_scalar::<_,Uuid>("SELECT model_ref FROM linggan_model_entry WHERE connection_version_ref=$1 AND model_id=$2")
            .bind(current_version).bind(model_id).fetch_one(&mut *tx).await?)
    } else {
        None
    };
    let workspace: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    let base_url: String = row.get("base_url");
    let local_endpoint: bool = row.get("local_endpoint");
    if secrets.is_synthetic() && !(local_endpoint && base_url.starts_with("http://127.0.0.1:")) {
        return Err(
            FoundationError::AdmissionDenied(vec!["synthetic_endpoint_required".into()]).into(),
        );
    }
    let key = secrets.get(workspace, row.get("secret_ref"))?;
    let config=sqlx::query("SELECT model_config_ref,budget_mode,daily_token_limit FROM linggan_ai_task_config WHERE task_config_ref=$1")
        .bind(item.task_config_ref).fetch_one(&mut *tx).await?;
    let mut request = PiRequest {
        images: vec![],
        version: PI_PROTOCOL,
        operation: operation.as_str().into(),
        api: row.get("api"),
        base_url,
        local_endpoint,
        api_key: key,
        model_id: model_id.clone().unwrap_or_default(),
        timeout_ms: 10_000,
        max_output_tokens: 1024,
        system: "Return only the JSON object requested by the synthetic test. Do not use tools."
            .into(),
        prompt:
            "{\"contract\":\"ai-probe.synthetic.v1\",\"respondWith\":{\"lingganProbe\":\"ok\"}}"
                .into(),
        execution: None,
    };
    let mut tested_profile = Value::Null;
    if let InputRef::Probe {
        profile: Some(profile),
        ..
    } = &item.input
    {
        let mut execution = transport_store::load_probe_execution_in(
            &mut tx,
            model_ref.ok_or(ExecutorError::Invalid)?,
            row.get("endpoint_ref"),
            profile.capability_ref,
            &profile.limits,
        )
        .await?;
        execution.response_schema = Some(ResponseSchema {
            name: "ai_probe".into(),
            schema: json!({"type":"object","properties":{"lingganProbe":{"type":"string","enum":["ok"]}},"required":["lingganProbe"],"additionalProperties":false}),
        });
        tested_profile = json!({"execution":&execution,"selection":&profile.limits});
        request.max_output_tokens = profile.limits.output_tokens as i32;
        request.timeout_ms = profile.limits.timeout_seconds * 1000;
        request.execution = Some(execution);
    }
    request.wire_bytes()?;
    let reservation = if operation == ProbeOperation::Probe {
        crate::ai_foundation::transport::estimate_input_tokens(&request.system, &request.prompt)
            .checked_add(request.max_output_tokens as u64)
            .ok_or(ExecutorError::Invalid)?
    } else {
        0
    };
    let admitted_at =
        super::budget::admit_in(&mut tx, item.task_key, item.task_config_ref, reservation).await?;
    let price =
        super::budget::latest_price_in(&mut tx, current_model, row.get("endpoint_ref")).await?;
    let material = if operation == ProbeOperation::Probe {
        "synthetic_probe"
    } else {
        "model_catalog"
    };
    let egress:Uuid=sqlx::query_scalar("SELECT event_ref FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key='model_probe' AND material_class=$2 AND allowed")
        .bind(row.get::<Uuid,_>("endpoint_ref")).bind(material).fetch_one(&mut *tx).await?;
    let invocation = Uuid::new_v4();
    let hash = content_hash(
        &json!([
            "ai-probe.synthetic.v1",
            item.item_ref,
            item.task_config_ref,
            current_version,
            model_id,
            operation,
            &tested_profile
        ])
        .to_string(),
    );
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,
        task_key,task_config_ref,batch_ref,round_no,quota_bucket,classification_source,dispatch_admitted_at,deadline_at,egress_event_ref,result,created_at,price_ref)
        VALUES($1,$2,$3,$4,$5,$6,'running',$7,$7,'model_probe',$8,$9,$13,'probe','producer_v1',$14::text::timestamptz,$14::text::timestamptz+make_interval(secs=>$10),$11,$12,$14::text::timestamptz,$15)")
        .bind(invocation).bind(current_version).bind(current_model).bind(config.get::<Option<Uuid>,_>("model_config_ref"))
        .bind(operation.as_str()).bind(hash).bind(reservation as i64).bind(item.task_config_ref).bind(item.batch_ref)
        .bind((request.timeout_ms as f64/1000.0)+AI_LIMITS.adapter_deadline_allowance_seconds as f64)
        .bind(egress).bind(json!({"producer":"ai-gen1-executor.v1","testedProfile":tested_profile}))
        .bind((item.position/AI_LIMITS.round_size+1) as i32).bind(admitted_at).bind(price)
        .execute(&mut *tx).await?;
    let changed=sqlx::query("UPDATE linggan_ai_batch_item_control c SET invocation_ref=$2,attempt_count=attempt_count+1,
        executed_config_ref=cfg.task_config_ref,executed_skill_version_ref=cfg.skill_version_ref,execution_contract=cfg.execution_contract
        FROM linggan_ai_task_config cfg WHERE item_ref=$1 AND cfg.task_config_ref=$3 AND attempt_count=0")
        .bind(item.item_ref).bind(invocation).bind(item.task_config_ref).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(ExecutorError::FenceLost);
    }
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        tested_profile,
    })
}

pub async fn execute_probe(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let prepared = match prepare(db, item, secrets).await {
        Ok(p) => p,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons))) => {
            let reason = reasons
                .first()
                .map(String::as_str)
                .unwrap_or("admission_denied");
            item.defer(db, reason).await?;
            return Ok(());
        }
        Err(error) => return super::preparation::handle(db, item, error).await,
    };
    let response = super::dispatch::call(db, item, adapter, &prepared.request).await;
    super::receipt::finalize(db, item, prepared.invocation, response.as_ref(), || {
        settle(db, item, &prepared, response.as_ref())
    })
    .await
}

async fn settle(
    db: &Database,
    item: &ClaimedItem,
    prepared: &Prepared,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let (mut tx, cancelled) =
        super::receipt::lock_in(db, item, prepared.invocation, response).await?;
    let ok = response.is_some_and(|r| r.ok);
    let not_dispatched =
        response.is_some_and(|r| r.diagnostic.as_ref().and_then(|d| d.request_sent) == Some(false));
    let failure = if ok {
        None
    } else if not_dispatched {
        Some("ai_adapter_rejected_before_dispatch")
    } else {
        Some(if response.is_some() {
            "ai_provider_failed"
        } else {
            "ai_dispatch_unknown"
        })
    };
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["testedProfile"] = prepared.tested_profile.clone();
    result["adapterVersion"] = json!(PI_PROTOCOL);
    result["executorReceipt"] = json!(item.lease_token);
    if let InputRef::Probe {
        operation: ProbeOperation::Probe,
        ..
    } = item.input
    {
        let format_ok = response
            .and_then(|r| r.text.as_deref())
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            == Some(json!({"lingganProbe":"ok"}));
        result["modelCallable"] = json!(ok);
        result["syntheticFormatMatched"] = json!(format_ok);
        result["semanticQualified"] = json!(false); // This is not a comment-study contract test.
    }
    super::receipt::observe_in(&mut tx, prepared.invocation, response).await?;
    finish_invocation_in(&mut tx, prepared.invocation, response, ok, failure, &result).await?;
    let counts_as_failure = !ok && !cancelled && !not_dispatched;
    super::receipt::write_item_in(
        &mut tx,
        item,
        super::receipt::ItemResult {
            success: ok,
            cancelled,
            output: Some(&result),
            failure,
            counts_as_failure,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
