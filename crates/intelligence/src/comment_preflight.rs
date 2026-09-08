//! Read-only work planning: selected material is distinct from new model work.
use crate::{
    comment_cleaning::{CLEANER_VERSION, clean},
    comment_packet::{build_packet_with_policy, input_fingerprint},
    comment_runtime::ContextPolicy,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::BTreeMap;
use uuid::Uuid;

pub const MAX_RESEARCH_SOURCES: usize = 3000;

pub async fn inspect(
    db: &Database,
    refs: &[Uuid],
    policy: &ContextPolicy,
    reanalyze: bool,
) -> Result<Value, ModelError> {
    let config = sqlx::query(crate::model_settings::with_model_callability("SELECT c.config_ref,c.input_token_limit,c.output_token_limit,w.workspace_ref,concat(m.connection_version_ref,':',m.model_id) AS identity FROM linggan_model_workspace w JOIN linggan_model_config c ON c.config_ref=w.default_config_ref JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) WHERE w.singleton AND conn.enabled AND __MODEL_CALLABLE__"))
        .fetch_optional(db.pool()).await?;
    let rows=sqlx::query("SELECT c.material_ref,c.content_public_ref,c.comment_external_id,c.body_text FROM linggan_comment_research_readable c WHERE c.material_ref=ANY($1) ORDER BY c.content_public_ref,c.material_ref")
        .bind(refs).fetch_all(db.pool()).await?;
    if rows.len() != refs.len() {
        return Err(ModelError::Source);
    }
    let mut counts = BTreeMap::from([
        ("reusable", 0usize),
        ("newAnalysis", 0),
        ("dropped", 0),
        ("contextMissing", 0),
        ("anomaly", 0),
        ("retryRequired", 0),
        ("inProgress", 0),
        ("inputTooLarge", 0),
    ]);
    let mut fingerprints = serde_json::Map::new();
    let mut new_by_work: BTreeMap<Uuid, usize> = BTreeMap::new();
    let mut per_source_tokens = 0usize;
    for row in rows {
        let reference: Uuid = row.get("material_ref");
        let body: Option<String> = row.get("body_text");
        if let Some(state) = filtered_state(body.as_deref()) {
            *counts
                .get_mut(if state == "dropped" {
                    "dropped"
                } else {
                    "anomaly"
                })
                .unwrap() += 1;
            fingerprints.insert(
                reference.to_string(),
                json!(crate::comment_research::comment_source_hash(
                    body.as_deref().unwrap_or("")
                )),
            );
            continue;
        }
        let Some(cfg) = &config else {
            *counts.get_mut("newAnalysis").unwrap() += 1;
            continue;
        };
        let config_ref: Uuid = cfg.get("config_ref");
        let packet = build_packet_with_policy(
            db,
            &[reference],
            &crate::comment_daily::version(config_ref),
            policy,
        )
        .await?;
        let input = &packet.inputs[0];
        let fingerprint = input_fingerprint(input, &cfg.get::<String, _>("identity"));
        fingerprints.insert(reference.to_string(), json!(fingerprint));
        let identity = crate::comment_research::comment_source_hash(&format!(
            "{}:{}",
            row.get::<Uuid, _>("content_public_ref"),
            row.get::<String, _>("comment_external_id")
        ));
        let old=sqlx::query("SELECT state FROM linggan_comment_semantic_work WHERE workspace_ref=$1 AND identity_key=$2 AND fingerprint=$3")
            .bind(cfg.get::<Uuid,_>("workspace_ref")).bind(identity).bind(&fingerprint).fetch_optional(db.pool()).await?;
        if !reanalyze {
            if let Some(old) = old {
                let state: String = old.get("state");
                let key = match state.as_str() {
                    "succeeded" | "no_signal" => "reusable",
                    "running" => "inProgress",
                    _ => "retryRequired",
                };
                *counts.get_mut(key).unwrap() += 1;
                continue;
            }
        }
        if !crate::comment_daily_runner::missing_context(&packet).is_empty() {
            *counts.get_mut("contextMissing").unwrap() += 1;
            continue;
        }
        let bytes = packet.prompt.len() + crate::comment_packet::SYSTEM.len() + 512;
        if bytes > cfg.get::<i32, _>("input_token_limit") as usize {
            *counts.get_mut("inputTooLarge").unwrap() += 1;
            continue;
        }
        *counts.get_mut("newAnalysis").unwrap() += 1;
        *new_by_work
            .entry(row.get("content_public_ref"))
            .or_default() += 1;
        per_source_tokens += bytes + cfg.get::<i32, _>("output_token_limit") as usize;
    }
    let max_packet = config
        .as_ref()
        .map(|c| (c.get::<i32, _>("output_token_limit") / 256).clamp(1, 30) as usize)
        .unwrap_or(1)
        .min(policy.max_comments);
    let minimum_calls: usize = new_by_work.values().map(|n| n.div_ceil(max_packet)).sum();
    Ok(
        json!({"total":refs.len(),"counts":counts,"configRef":config.as_ref().map(|c|c.get::<Uuid,_>("config_ref")),"fingerprints":fingerprints,"cleanerVersion":CLEANER_VERSION,"contractVersion":crate::comment_daily::DAILY_RULE,"estimatedCalls":{"min":minimum_calls,"max":counts["newAnalysis"],"available":config.is_some()},"tokenUpperBound":if config.is_some(){Some(per_source_tokens)}else{None},"estimateMethod":"单条输入UTF8字节保守上界加输出预算；实际按作品分包，用量以供应商回执为准"}),
    )
}

fn filtered_state(body: Option<&str>) -> Option<&'static str> {
    let Some(body) = body else {
        return Some("anomaly");
    };
    match clean(body).state.as_str() {
        "dropped" => Some("dropped"),
        "anomaly" => Some("anomaly"),
        _ => None,
    }
}
