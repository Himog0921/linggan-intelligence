use super::{Error, METHOD_VERSION, PAGE_SIZE, Round, RoundPage, Scope, scope};
use crate::directory_boundary::patrol_scan_qualified_sql;
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const ROUNDS_SQL: &str = concat!(
    "SELECT package.attempt_id AS round_ref,package.package_ref,revision.rule_ref, \
      work_order.monitor_rule_revision_ref AS rule_revision_ref,rule.slot_key, \
      to_char(package.observed_at::timestamptz AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS observed_at, \
      (extract(epoch FROM package.observed_at::timestamptz)*1000)::bigint AS observed_epoch_ms, \
      to_char(package.accepted_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS accepted_at, \
      CASE WHEN layer IS NULL THEN 'unknown' \
        WHEN receipt.execution_effect='COMPLETED_LIVE_STEP' AND ",
    patrol_scan_qualified_sql!(),
    " THEN 'complete' ELSE 'partial' END AS coverage, \
      (SELECT count(DISTINCT finding.content_public_ref) \
       FROM linggan_material_discovery_finding finding \
       JOIN linggan_runtime_record_disposition disposition \
         ON disposition.package_ref=finding.package_ref AND disposition.record_ordinal=finding.record_ordinal \
       WHERE finding.package_ref=package.package_ref \
         AND disposition.disposition='accepted_for_library_discovery') AS hits, \
      jsonb_build_object('surface',package.package_kind,'target',task.task_spec->'target', \
        'scrollRounds',task.task_spec->'scrollRounds','topByLikes',task.task_spec->'topByLikes', \
        'publishedWithinDays',task.task_spec->'publishedWithinDays', \
        'maximumQuota',task.task_spec->'maximumQuota','expectedCount',task.task_spec->'expectedCount', \
        'ranking',revision.ranking_key) AS conditions, \
      lease.capture_identity->>'accountRef' AS account_ref \
     FROM collection_work_order work_order \
     JOIN collection_work_order_lease lease USING(work_order_ref) \
     JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
     JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
     JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
     JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
     LEFT JOIN collection_monitor_rule_revision revision \
       ON revision.rule_revision_ref=work_order.monitor_rule_revision_ref \
     LEFT JOIN collection_monitor_rule rule ON rule.rule_ref=revision.rule_ref \
     LEFT JOIN LATERAL (SELECT value AS layer FROM jsonb_array_elements( \
       CASE WHEN jsonb_typeof(package.coverage->'layers')='array' THEN package.coverage->'layers' \
       ELSE '[]'::jsonb END) value WHERE value->>'capability'=package.package_kind \
       AND (SELECT count(*) FROM jsonb_array_elements( \
         CASE WHEN jsonb_typeof(package.coverage->'layers')='array' THEN package.coverage->'layers' \
         ELSE '[]'::jsonb END) candidate WHERE candidate->>'capability'=package.package_kind)=1) coverage ON true \
     WHERE work_order.target_ref=$1 \
       AND package.package_kind IN ('discovery_search','profile_discovery') \
       AND package.platform=task.platform \
       AND task.task_spec->'capabilitiesRequested' ? package.package_kind \
       AND receipt.material_admission='ACCEPTED' \
       AND package.accepted_at <= $3::timestamptz \
       AND package.observed_at::timestamptz <= $3::timestamptz \
       AND ($4::uuid IS NULL OR revision.rule_ref=$4) \
       AND ($5::uuid[] IS NULL OR package.attempt_id=ANY($5)) \
       AND EXISTS(SELECT 1 FROM collection_work_order_domain_usage usage \
         WHERE usage.work_order_ref=work_order.work_order_ref AND ($2::uuid IS NULL OR usage.domain_ref=$2)) \
     ORDER BY package.observed_at::timestamptz DESC,package.attempt_id DESC \
     LIMIT $6 OFFSET $7"
);

pub async fn read_rounds(
    database: &Database,
    request: &Scope,
    rule: Option<Uuid>,
    offset: usize,
) -> Result<RoundPage, Error> {
    let (mut tx, cutoff) = scope::begin(database, request).await?;
    if let Some(rule) = rule {
        require_rule(&mut tx, request.target_ref, rule).await?;
    }
    let mut rounds = read_in(&mut tx, request, &cutoff, rule, None, PAGE_SIZE + 1, offset).await?;
    let next_offset = (rounds.len() > PAGE_SIZE).then_some(offset + PAGE_SIZE);
    rounds.truncate(PAGE_SIZE);
    tx.commit().await?;
    Ok(RoundPage {
        target_ref: request.target_ref,
        domain_ref: request.domain_ref,
        knowledge_cutoff: cutoff,
        method_version: METHOD_VERSION,
        rounds,
        next_offset,
    })
}

pub(crate) async fn require_rule(
    tx: &mut Transaction<'_, Postgres>,
    target: Uuid,
    rule: Uuid,
) -> Result<(), Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collection_monitor_rule WHERE target_ref=$1 AND rule_ref=$2)",
    )
    .bind(target)
    .bind(rule)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(Error::NotFound);
    }
    Ok(())
}

pub(crate) async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    request: &Scope,
    cutoff: &str,
    rule: Option<Uuid>,
    only: Option<&[Uuid]>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Round>, Error> {
    let offset = scope::check_offset(offset)?;
    if limit > PAGE_SIZE + 1 {
        return Err(Error::Invalid("round_limit_out_of_range"));
    }
    let rows = sqlx::query(ROUNDS_SQL)
        .bind(request.target_ref)
        .bind(request.domain_ref)
        .bind(cutoff)
        .bind(rule)
        .bind(only)
        .bind(i64::try_from(limit).unwrap_or(51))
        .bind(offset)
        .fetch_all(&mut **tx)
        .await?;
    rows.into_iter()
        .map(|row| {
            let account: Option<String> = row.try_get("account_ref")?;
            Ok(Round {
                round_ref: row.try_get("round_ref")?,
                package_ref: row.try_get("package_ref")?,
                rule_ref: row.try_get("rule_ref")?,
                rule_revision_ref: row.try_get("rule_revision_ref")?,
                slot: row.try_get("slot_key")?,
                observed_at: row.try_get("observed_at")?,
                observed_epoch_ms: row.try_get("observed_epoch_ms")?,
                accepted_at: row.try_get("accepted_at")?,
                coverage: row.try_get("coverage")?,
                hits: row.try_get("hits")?,
                conditions: row.try_get("conditions")?,
                account_ref: account.as_deref().and_then(|v| Uuid::parse_str(v).ok()),
            })
        })
        .collect()
}
