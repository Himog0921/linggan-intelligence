//! A process cannot claim work on an unrecorded or different foundation schema.
use super::ExecutorError;
use crate::research_text::content_hash;
use linggan_storage_postgres::Database;

macro_rules! migration {
    ($id:literal) => {
        (
            $id,
            include_str!(concat!("../../../../../database/migrations/", $id, ".sql")),
        )
    };
}
const MIGRATIONS: &[(&str, &str)] = &[
    migration!("0131_ai_gen1_provider"),
    migration!("0132_ai_gen1_control"),
    migration!("0133_ai_gen1_skill"),
    migration!("0134_ai_gen1_task_config"),
    migration!("0135_ai_gen1_batch"),
    migration!("0136_ai_gen1_skill_binding"),
    migration!("0137_ai_gen1_round"),
    migration!("0138_ai_gen1_invocation"),
    migration!("0139_ai_gen1_executor"),
    migration!("0140_ai_gen1_writing"),
    migration!("0141_ai_gen1_legacy_settings"),
    migration!("0146_research_dossier_v2"),
    migration!("0148_dossier_consumer_contracts"),
    migration!("0169_ai_manual_skill_activation"),
    migration!("0170_ai_prompt_versions"),
    migration!("0171_ai_writing_comparison_prompts"),
    migration!("0172_research_method_settings"),
    migration!("0173_creator_research_execution"),
    migration!("0174_comment_research_execution"),
    migration!("0175_creator_account_research"),
    migration!("0176_topic_research_execution"),
    migration!("0177_comment_problem_workflow"),
    migration!("0178_creator_manual_annotations"),
    migration!("0179_retire_research_relations"),
];

pub(super) async fn require_ready(db: &Database) -> Result<(), ExecutorError> {
    let ledger: bool = sqlx::query_scalar("SELECT to_regclass(format('%I.linggan_local_schema_migration',current_schema())) IS NOT NULL")
        .fetch_one(db.pool()).await?;
    if !ledger {
        return Err(ExecutorError::SchemaNotReady);
    }
    for (id, source) in MIGRATIONS {
        let hash: Option<String> = sqlx::query_scalar(
            "SELECT migration_sha256 FROM linggan_local_schema_migration WHERE migration_id=$1",
        )
        .bind(id)
        .fetch_optional(db.pool())
        .await?;
        if hash.as_deref() != Some(content_hash(source).as_str()) {
            return Err(ExecutorError::SchemaNotReady);
        }
    }
    let shape: bool = sqlx::query_scalar(
        "SELECT
        to_regclass('linggan_ai_executor_current') IS NOT NULL
        AND to_regclass('linggan_ai_batch_item_control') IS NOT NULL
        AND to_regclass('linggan_creation_ai_request') IS NOT NULL
        AND EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema()
            AND table_name='linggan_creation_ai_request' AND column_name='task_config_ref')
        AND to_regprocedure('linggan_ai_executor_is_owner(uuid,bigint,boolean)') IS NOT NULL
        AND NOT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema()
            AND table_name='linggan_model_workspace' AND column_name IN('default_config_ref','worker_last_seen_at','worker_state','worker_last_error'))",
    )
    .fetch_one(db.pool())
    .await?;
    if !shape {
        return Err(ExecutorError::SchemaNotReady);
    }
    Ok(())
}
