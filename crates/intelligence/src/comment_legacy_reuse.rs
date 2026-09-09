//! Additive compatibility proof for historical v4 work; never rewrite its fingerprint or result.
use crate::{
    comment_packet::{build_packet_with_rule, input_fingerprint},
    comment_research::comment_source_hash,
    comment_research_fingerprint::{RuleFingerprint, SourceIdentity, build_semantic_input},
    comment_research_rules::{self, RuleVersion},
    comment_runtime::ContextPolicy,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn reconcile(db: &Database) -> Result<usize, ModelError> {
    let rule = comment_research_rules::read_active_rule(db).await?;
    if rule.rule_version != RuleVersion::V4 {
        return Ok(0);
    }
    let policy =
        ContextPolicy::parse(crate::comment_runtime::settings(db).await?["policy"].clone())?;
    let candidates = sqlx::query(r#"
      SELECT sw.semantic_ref,sw.identity_key,sw.fingerprint,sw.analysis_ref,a.result,
        current.material_ref,source.content_public_ref,source.comment_external_id,
        concat(m.connection_version_ref,':',m.model_id) AS original_config_identity
      FROM linggan_comment_semantic_work sw
      JOIN linggan_comment_analysis_work a ON a.work_ref=sw.analysis_ref
      JOIN linggan_material_comment source ON source.material_ref=sw.source_ref
      JOIN linggan_material_comment_current current ON current.content_public_ref=source.content_public_ref
        AND current.comment_external_id=source.comment_external_id
      JOIN linggan_comment_research_readable allowed ON allowed.material_ref=current.material_ref
      JOIN linggan_model_config config ON config.config_ref=sw.config_ref
      JOIN linggan_model_entry m ON m.model_ref=config.model_ref
      WHERE sw.input_manifest IS NULL AND sw.state IN('succeeded','no_signal')
        AND a.rule_version='comment-research.v4' AND linggan_ci_analysis_context_readable(a.result)
        AND NOT EXISTS(SELECT 1 FROM linggan_comment_legacy_fingerprint_alias alias
          WHERE alias.source_identity=sw.identity_key AND alias.old_fingerprint=sw.fingerprint)
      ORDER BY sw.compatibility_checked_at NULLS FIRST,sw.updated_at DESC,sw.semantic_ref LIMIT 100
    "#).fetch_all(db.pool()).await?;
    let mut added = 0;
    for row in candidates {
        let source_ref: Uuid = row.get("material_ref");
        sqlx::query("UPDATE linggan_comment_semantic_work SET compatibility_checked_at=scope_001_now() WHERE semantic_ref=$1")
            .bind(row.get::<Uuid,_>("semantic_ref")).execute(db.pool()).await?;
        let packet = match build_packet_with_rule(
            db,
            &[source_ref],
            "compatibility-no-call",
            &policy,
            &rule,
        )
        .await
        {
            Ok(packet) => packet,
            Err(ModelError::Source | ModelError::NotFound) => continue,
            Err(error) => return Err(error),
        };
        let old: String = row.get("fingerprint");
        // Recompute the exact historical algorithm with its recorded execution identity. If
        // original context, source, selector or generation cannot be reproduced, no alias exists.
        if input_fingerprint(
            &packet.inputs[0],
            &row.get::<String, _>("original_config_identity"),
        ) != old
        {
            continue;
        }
        let result: Value = row.get("result");
        if !crate::comment_daily_read::context_readable(db, &result).await? {
            continue;
        }
        let semantic = build_semantic_input(
            &packet.inputs[0],
            &SourceIdentity {
                work_ref: row.get("content_public_ref"),
                comment_external_id: row.get("comment_external_id"),
            },
            &RuleFingerprint::from(&rule),
        );
        if semantic.source_identity != row.get::<String, _>("identity_key") {
            continue;
        }
        let analysis: Uuid = row.get("analysis_ref");
        let proof = comment_source_hash(
            &json!({"method":"recomputed-v4-input.v1","old":old,
            "analysisRef":analysis,"newManifest":semantic.input_manifest})
            .to_string(),
        );
        added += sqlx::query(
            r#"INSERT INTO linggan_comment_legacy_fingerprint_alias
          (source_identity,old_fingerprint,new_fingerprint,analysis_ref,proof_hash)
          VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING"#,
        )
        .bind(semantic.source_identity)
        .bind(old)
        .bind(semantic.fingerprint)
        .bind(analysis)
        .bind(proof)
        .execute(db.pool())
        .await?
        .rows_affected() as usize;
    }
    Ok(added)
}
