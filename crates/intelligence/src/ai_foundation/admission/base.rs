//! Prequeue checks only. Dispatch must recheck permission, material, budget and lease at its boundary.
use super::{
    TaskKey,
    batch_store::{InputRef, ProbeOperation},
    store::FoundationError,
};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
pub(crate) async fn check_queue_configuration(
    tx: &mut Transaction<'_, Postgres>,
    task: TaskKey,
    config: Uuid,
    trial: bool,
) -> Result<(), FoundationError> {
    let row=sqlx::query("SELECT t.lifecycle,t.requires_skill,t.handler_contract,c.execution_contract,c.model_config_ref,c.skill_version_ref,c.budget_mode,
        m.capability_ref,p.endpoint_ref,cap.endpoint_ref AS frozen_endpoint,b.current_version_ref,
        connection.enabled AS connection_enabled,probe.state AS probe_state,probe.result AS probe_result
        FROM linggan_ai_task_config c JOIN linggan_ai_task t USING(task_key)
        JOIN linggan_skill_binding b USING(task_key)
        LEFT JOIN linggan_model_config m ON m.config_ref=c.model_config_ref
        LEFT JOIN linggan_model_capability cap ON cap.capability_ref=m.capability_ref
        LEFT JOIN linggan_ai_model_identity identity ON identity.identity_ref=cap.identity_ref
        LEFT JOIN linggan_model_connection connection ON connection.connection_ref=identity.connection_ref
        LEFT JOIN LATERAL(SELECT version_ref FROM linggan_model_connection_version WHERE connection_ref=identity.connection_ref ORDER BY revision DESC LIMIT 1)v ON true
        LEFT JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref
        LEFT JOIN linggan_model_entry current_model ON current_model.connection_version_ref=v.version_ref AND current_model.model_id=identity.model_id
        LEFT JOIN LATERAL(SELECT state,result FROM linggan_model_invocation
         WHERE model_ref=current_model.model_ref AND connection_version_ref=v.version_ref AND operation='probe'
         ORDER BY created_at DESC,invocation_ref DESC LIMIT 1)probe ON true
        WHERE c.task_config_ref=$1 AND c.task_key=$2")
        .bind(config).bind(task.as_str()).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let mut denied = Vec::new();
    if row.get::<String, _>("lifecycle") != "active" {
        denied.push("task_not_connected".into());
    }
    let contract: Option<String> = row.get("execution_contract");
    if contract.is_none() || contract != row.get::<Option<String>, _>("handler_contract") {
        denied.push("handler_version_not_connected".into());
    }
    if row.get::<String, _>("budget_mode") == "unconfigured" {
        denied.push("daily_budget_unconfigured".into());
    }
    let skill: Option<Uuid> = row.get("skill_version_ref");
    if row.get::<bool, _>("requires_skill") && skill.is_none() {
        denied.push("skill_not_configured".into());
    }
    if !trial && skill != row.get::<Option<Uuid>, _>("current_version_ref") {
        denied.push("skill_not_enabled".into());
    }
    if task != TaskKey::ModelProbe
        && (row.get::<Option<Uuid>, _>("capability_ref").is_none()
            || row.get::<Option<Uuid>, _>("endpoint_ref")
                != row.get::<Option<Uuid>, _>("frozen_endpoint"))
    {
        denied.push("model_configuration_unqualified".into());
    }
    if task != TaskKey::ModelProbe {
        if row.get::<Option<bool>, _>("connection_enabled") == Some(false) {
            denied.push("connection_disabled".into());
        }
        let result: Option<serde_json::Value> = row.get("probe_result");
        if row.get::<Option<Uuid>, _>("capability_ref").is_some()
            && (row.get::<Option<String>, _>("probe_state").as_deref() != Some("succeeded")
                || !result.is_some_and(|value| {
                    value.get("modelCallable") == Some(&serde_json::Value::Bool(true))
                }))
        {
            denied.push("current_model_not_probed".into());
        }
    }
    let controls=sqlx::query("SELECT scope_key,enabled FROM linggan_ai_control WHERE scope_key IN ('global','executor',$1) ORDER BY scope_key FOR SHARE")
        .bind(format!("task:{}",task.as_str())).fetch_all(&mut **tx).await?;
    if controls.len() != 3 {
        denied.push("control_not_configured".into());
    }
    for control in controls {
        if !control.get::<bool, _>("enabled") {
            denied.push(format!(
                "{}_off",
                control.get::<String, _>("scope_key").replace(':', "_")
            ));
        }
    }
    if denied.is_empty() {
        Ok(())
    } else {
        Err(FoundationError::AdmissionDenied(denied))
    }
}
pub(crate) async fn check_queue_input_scope(
    tx: &mut Transaction<'_, Postgres>,
    task: TaskKey,
    config: Uuid,
    input: &InputRef,
) -> Result<(), FoundationError> {
    let (endpoint, material) = if let InputRef::Probe {
        connection_version_ref,
        model_ref,
        operation,
        profile,
        ..
    } = input
    {
        let row=sqlx::query("SELECT c.enabled,p.endpoint_ref,current_endpoint.endpoint_ref AS latest_endpoint,
          ($2::uuid IS NULL OR EXISTS(SELECT 1 FROM linggan_model_entry WHERE model_ref=$2 AND connection_version_ref=$1)) AS model_matches
          FROM linggan_model_connection_version v JOIN linggan_model_connection c USING(connection_ref)
          JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref
          JOIN LATERAL(SELECT version_ref FROM linggan_model_connection_version WHERE connection_ref=c.connection_ref ORDER BY revision DESC LIMIT 1)latest ON true
          JOIN linggan_ai_connection_endpoint current_endpoint ON current_endpoint.version_ref=latest.version_ref WHERE v.version_ref=$1 FOR SHARE OF c")
            .bind(connection_version_ref).bind(model_ref).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
        if !row.get::<bool, _>("enabled")
            || !row.get::<bool, _>("model_matches")
            || row.get::<Uuid, _>("endpoint_ref") != row.get::<Uuid, _>("latest_endpoint")
        {
            return Err(FoundationError::AdmissionDenied(vec![
                "connection_changed_or_disabled".into(),
            ]));
        }
        if let Some(profile) = profile {
            super::transport_store::load_probe_execution_in(
                &mut **tx,
                model_ref.ok_or(FoundationError::Invalid)?,
                row.get("endpoint_ref"),
                profile.capability_ref,
                &profile.limits,
            )
            .await?;
        }
        (
            row.get::<Uuid, _>("endpoint_ref"),
            if *operation == ProbeOperation::Probe {
                "synthetic_probe"
            } else {
                "model_catalog"
            },
        )
    } else {
        let endpoint:Uuid=sqlx::query_scalar("SELECT cap.endpoint_ref FROM linggan_ai_task_config t JOIN linggan_model_config m ON m.config_ref=t.model_config_ref JOIN linggan_model_capability cap ON cap.capability_ref=m.capability_ref WHERE t.task_config_ref=$1")
            .bind(config).fetch_one(&mut **tx).await?;
        let material = match task {
            TaskKey::OcrQualityAudit => {
                let InputRef::OcrAuditSample { sample_ref } = input else {
                    return Err(FoundationError::Invalid);
                };
                let pool: String = sqlx::query_scalar(
                    "SELECT pool FROM linggan_ocr_audit_sample WHERE sample_ref=$1",
                )
                .bind(sample_ref)
                .fetch_one(&mut **tx)
                .await?;
                if pool == "probe" {
                    "synthetic_image_probe"
                } else {
                    let adoption: Uuid = sqlx::query_scalar(
                        "SELECT adoption_ref FROM linggan_ocr_audit_sample WHERE sample_ref=$1",
                    )
                    .bind(sample_ref)
                    .fetch_one(&mut **tx)
                    .await?;
                    if !crate::ocr_quality_audit::source::external_clearance_in(&mut **tx, adoption)
                        .await?
                    {
                        return Err(FoundationError::AdmissionDenied(vec![
                            "audit_privacy_unreviewed".into(),
                        ]));
                    }
                    "ocr_quality_image"
                }
            }
            TaskKey::OcrRecognition => {
                let InputRef::OcrRecognitionItem { item_ref } = input else {
                    return Err(FoundationError::Invalid);
                };
                let available: bool = sqlx::query_scalar(
                    "SELECT NOT EXISTS(SELECT 1 FROM linggan_ocr_recognition_item i
                     WHERE i.item_ref=$1 AND EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d
                       WHERE d.slot_key=i.slot_key OR d.blob_sha256=i.blob_sha256 OR d.materialization_ref=i.materialization_ref))",
                ).bind(item_ref).fetch_one(&mut **tx).await?;
                if !available {
                    return Err(FoundationError::AdmissionDenied(vec![
                        "ocr_source_unavailable".into(),
                    ]));
                }
                "manual_ocr_image"
            }
            TaskKey::NoteInterpretation
            | TaskKey::DirectionListDraft
            | TaskKey::DirectionProposal => "note_material",
            TaskKey::AuthorOrgIdentity => "public_author_bio",
            TaskKey::WritingAssistant | TaskKey::WritingLegacy => "selected_writing_material",
            _ => return Err(FoundationError::Invalid),
        };
        (endpoint, material)
    };
    let allowed:bool=sqlx::query_scalar("SELECT COALESCE((SELECT allowed FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key=$2 AND material_class=$3),false)")
        .bind(endpoint).bind(task.as_str()).bind(material).fetch_one(&mut **tx).await?;
    if !allowed {
        return Err(FoundationError::AdmissionDenied(vec![
            "egress_not_approved".into(),
        ]));
    }
    Ok(())
}
