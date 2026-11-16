//! Capture one current card snapshot while source writers are briefly excluded.
use super::CardError;
use sqlx::{AssertSqlSafe, Postgres, Transaction};

// Base relations reached by the existing card calculation; the final audit
// rejects any successfully used relation absent from this list.
const SOURCE_TABLES: &[&str] = &[
    "collection_observation_target",
    "linggan_ai_batch_item_control",
    "linggan_ai_task_config",
    "linggan_comment_clean_cache",
    "linggan_comment_problem_definition",
    "linggan_comment_problem_membership",
    "linggan_comment_research_request",
    "linggan_comment_research_stage",
    "linggan_local_schema_migration",
    "linggan_material_author_profile",
    "linggan_material_comment",
    "linggan_material_comment_engagement",
    "linggan_material_comment_first_admission",
    "linggan_material_comment_platform_time",
    "linggan_material_comment_restriction",
    "linggan_material_content",
    "linggan_material_content_detail",
    "linggan_material_content_hashtag",
    "linggan_material_discovery_finding",
    "linggan_material_domain_usage",
    "linggan_material_hashtag_projection",
    "linggan_material_lane_observation",
    "linggan_material_media_disposition_event",
    "linggan_material_media_origin",
    "linggan_material_reading",
    "linggan_material_reading_command",
    "linggan_material_reading_selection",
    "linggan_material_unit",
    "linggan_media_blob",
    "linggan_media_derivative",
    "linggan_media_download_attempt",
    "linggan_media_materialization",
    "linggan_media_ocr_retirement",
    "linggan_media_processing_job",
    "linggan_media_processing_job_event",
    "linggan_model_entry",
    "linggan_model_invocation",
    "linggan_opportunity_card_change",
    "linggan_opportunity_card_period_index",
    "linggan_opportunity_card_source_guard",
    "linggan_opportunity_card_voice_ref",
    "linggan_opportunity_comment_mention",
    "linggan_opportunity_creator_problem_density",
    "linggan_opportunity_digest_verifier",
    "linggan_opportunity_metric_definition",
    "linggan_opportunity_metric_definition_current",
    "linggan_opportunity_metric_shadow",
    "linggan_opportunity_opportunity_card",
    "linggan_opportunity_problem",
    "linggan_opportunity_problem_lineage",
    "linggan_opportunity_problem_metric_period",
    "linggan_opportunity_taxonomy_version",
    "linggan_opportunity_taxonomy_version_history",
    "linggan_runtime_capture_package",
    "linggan_runtime_record_disposition",
    "linggan_runtime_task",
    "observation_domain_target",
];

/// Evidence reads never inspect model invocations; keep their writers out of the lock set.
pub(super) fn evidence_read_tables() -> Vec<&'static str> {
    SOURCE_TABLES
        .iter()
        .copied()
        .filter(|table| {
            !matches!(
                *table,
                "linggan_model_invocation" | "linggan_opportunity_card_period_index"
            )
        })
        .collect()
}

/// Capture the wall clock while all known card sources are locked. Rolling
/// back only this savepoint releases the locks but preserves the outer RR
/// snapshot, so the subsequent card calculation uses that same view.
pub(super) async fn capture(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(String, bool), CardError> {
    capture_for(tx, SOURCE_TABLES).await
}

/// Apply the same snapshot signing rule to a narrower, explicitly audited reader.
pub(super) async fn capture_for(
    tx: &mut Transaction<'_, Postgres>,
    tables: &[&str],
) -> Result<(String, bool), CardError> {
    sqlx::query("SAVEPOINT opportunity_card_capture")
        .execute(&mut **tx)
        .await?;
    for setting in [
        "SET LOCAL lock_timeout='100ms'",
        "SET LOCAL statement_timeout='500ms'",
        "SET LOCAL idle_in_transaction_session_timeout='1000ms'",
    ] {
        sqlx::query(setting).execute(&mut **tx).await?;
    }
    let statement = format!("LOCK TABLE {} IN SHARE MODE NOWAIT", tables.join(","));
    if let Err(error) = sqlx::query(AssertSqlSafe(statement))
        .execute(&mut **tx)
        .await
    {
        match error
            .as_database_error()
            .and_then(|db| db.code())
            .as_deref()
        {
            Some("55P03") => return Err(CardError::SourcesBusy),
            Some("42501") => return Err(CardError::SourceLockPermission),
            _ => return Err(error.into()),
        }
    }
    let captured: (String, bool) = sqlx::query_as(
        "SELECT to_char(captured.snapshot_time AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'), \
         captured.snapshot_time>=transaction_timestamp() FROM (SELECT scope_001_now() AS snapshot_time) captured",
    )
    .fetch_one(&mut **tx).await?;
    sqlx::query("ROLLBACK TO SAVEPOINT opportunity_card_capture")
        .execute(&mut **tx)
        .await?;
    sqlx::query("RELEASE SAVEPOINT opportunity_card_capture")
        .execute(&mut **tx)
        .await?;
    Ok(captured)
}

/// A hidden restriction row must never be mistaken for an eligible source.
pub(super) async fn require_visible_restrictions(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), CardError> {
    let filtered: bool = sqlx::query_scalar(
        "SELECT row_security_active('linggan_material_comment_restriction'::regclass)",
    )
    .fetch_one(&mut **tx)
    .await?;
    if filtered {
        Err(CardError::RestrictionVisibility)
    } else {
        Ok(())
    }
}

/// Verify relations actually used by the completed calculation before commit.
/// Current card code does not retain results from rolled-back subtransactions.
pub(super) async fn verify(tx: &mut Transaction<'_, Postgres>) -> Result<(), CardError> {
    verify_for(tx, SOURCE_TABLES).await
}

pub(super) async fn verify_for(
    tx: &mut Transaction<'_, Postgres>,
    tables: &[&str],
) -> Result<(), CardError> {
    sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut **tx)
        .await?;
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT namespace.nspname::text||'.'||relation.relname::text \
         FROM pg_catalog.pg_locks held JOIN pg_catalog.pg_class relation ON relation.oid=held.relation \
         JOIN pg_catalog.pg_namespace namespace ON namespace.oid=relation.relnamespace \
         WHERE held.pid=pg_backend_pid() AND held.granted AND relation.relkind IN ('r','p','f') \
           AND namespace.nspname NOT IN ('pg_catalog','information_schema') \
           AND namespace.nspname NOT LIKE 'pg_toast%' \
           AND (namespace.nspname<>current_schema() OR NOT relation.relname::text=ANY($1::text[])) \
         ORDER BY 1",
    )
    .bind(tables)
    .fetch_all(&mut **tx).await?;
    if missing.is_empty() {
        Ok(())
    } else {
        Err(CardError::SourceCoverage(missing))
    }
}

#[cfg(test)]
mod tests {
    use super::{CardError, capture, require_visible_restrictions};
    use sqlx::AssertSqlSafe;
    use uuid::Uuid;

    mod full_schema {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/api/src/local_web/full_schema_fixture.rs"
        ));
    }

    #[tokio::test]
    #[ignore = "requires the disposable opportunity PostgreSQL proof"]
    async fn select_only_role_cannot_capture_locked_sources() {
        let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
        let db = linggan_storage_postgres::testing::isolated_proof_schema(
            &url,
            "opportunity_card_snapshot_role",
            full_schema::FULL_MIGRATIONS,
        )
        .await
        .unwrap();
        let schema: String = sqlx::query_scalar("SELECT current_schema()")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert!(
            schema
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        );
        let role = format!("card_snapshot_reader_{}", Uuid::new_v4().simple());
        sqlx::raw_sql(AssertSqlSafe(format!("CREATE ROLE {role} NOLOGIN; GRANT USAGE ON SCHEMA {schema} TO {role}; GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO {role}")))
            .execute(db.pool()).await.unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("SET LOCAL ROLE {role}")))
            .execute(&mut *tx)
            .await
            .unwrap();
        let error = capture(&mut tx).await.unwrap_err();
        assert!(
            matches!(error, CardError::SourceLockPermission),
            "{error:?}"
        );
        tx.rollback().await.unwrap();
        println!(
            "SELECT-only role receives source-lock permission error; no grants were added to it"
        );
    }

    #[tokio::test]
    #[ignore = "requires the disposable opportunity PostgreSQL proof"]
    async fn filtered_restrictions_stop_card_builds() {
        let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
        let db = linggan_storage_postgres::testing::isolated_proof_schema(
            &url,
            "opportunity_card_restriction_rls",
            full_schema::FULL_MIGRATIONS,
        )
        .await
        .unwrap();
        let schema: String = sqlx::query_scalar("SELECT current_schema()")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert!(
            schema
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        );
        let role = format!("card_restriction_reader_{}", Uuid::new_v4().simple());
        sqlx::raw_sql(AssertSqlSafe(format!("CREATE ROLE {role} NOLOGIN; GRANT USAGE ON SCHEMA {schema} TO {role}; GRANT SELECT ON {schema}.linggan_material_comment_restriction TO {role}; ALTER TABLE {schema}.linggan_material_comment_restriction ENABLE ROW LEVEL SECURITY; ALTER TABLE {schema}.linggan_material_comment_restriction FORCE ROW LEVEL SECURITY; CREATE POLICY synthetic_hidden_restrictions ON {schema}.linggan_material_comment_restriction USING (false)")))
            .execute(db.pool()).await.unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        sqlx::query(AssertSqlSafe(format!("SET LOCAL ROLE {role}")))
            .execute(&mut *tx)
            .await
            .unwrap();
        let error = require_visible_restrictions(&mut tx).await.unwrap_err();
        assert!(
            matches!(error, CardError::RestrictionVisibility),
            "{error:?}"
        );
        tx.rollback().await.unwrap();
        println!("RLS-filtered restrictions are rejected before card calculation");
    }
}
