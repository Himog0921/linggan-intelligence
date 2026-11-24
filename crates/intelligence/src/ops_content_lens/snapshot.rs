use super::source::read_input;
use super::statistics::{RunAgreement, Vocabulary, summarize};
use super::{AnalysisInput, LabelMethod, LensDefinitionError, MetricDefinition};
use crate::ops_ledger::is_business_date;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, Row, types::Json};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotScope {
    pub domain_ref: Option<Uuid>,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl SnapshotScope {
    pub(super) fn validate(&self) -> Result<(), LensDefinitionError> {
        if self
            .from
            .iter()
            .chain(&self.to)
            .any(|day| !is_business_date(day))
            || matches!((&self.from,&self.to),(Some(f),Some(t)) if f>t)
        {
            return Err(LensDefinitionError::InvalidScope);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReceipt {
    pub snapshot_ref: Uuid,
    pub reused: bool,
}

/// Pending labels may appear in the board's input without contributing to analysis.
#[derive(Clone, Copy)]
pub(super) enum SnapshotMode {
    Strict,
    IncludePending,
}

async fn agreements(
    conn: &mut PgConnection,
    input: &AnalysisInput,
    m: &MetricDefinition,
) -> Result<Vec<RunAgreement>, LensDefinitionError> {
    let runs: Vec<_> = input
        .publications
        .values()
        .flat_map(|r| r.labels.values().map(|l| l.run_ref))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    // Only completed, fully paired audits are eligible, matching effective_label's rule.
    // When a primary has multiple audits the newest finished complete audit is authoritative.
    let rows=sqlx::query(r#"WITH complete AS (
        SELECT r.primary_run_ref,r.run_ref,b.finished_at
        FROM linggan_ops_lens_label_run r JOIN linggan_ai_batch_control b ON b.batch_ref=r.foundation_batch_ref
        WHERE r.primary_run_ref=ANY($1) AND b.state='completed'
          AND (SELECT count(*) FROM linggan_ops_lens_label l WHERE l.run_ref=r.run_ref)=r.title_count
          AND (SELECT count(*) FROM linggan_ops_lens_agreement a WHERE a.audit_run_ref=r.run_ref)=4
    ), newest AS (
        SELECT DISTINCT ON(primary_run_ref) primary_run_ref,run_ref FROM complete
        ORDER BY primary_run_ref,finished_at DESC,run_ref DESC
    ) SELECT a.primary_run_ref,a.audit_run_ref,a.dimension,a.sample_count,a.excluded_example_count,
          a.agreement::float8 AS agreement,a.kappa::float8 AS kappa,a.reason
      FROM newest n JOIN linggan_ops_lens_agreement a ON a.audit_run_ref=n.run_ref
      ORDER BY a.primary_run_ref,a.dimension"#).bind(&runs).fetch_all(&mut *conn).await?;
    if rows.len() != 4 * runs.len() {
        return Err(LensDefinitionError::MissingAgreement);
    }
    let mut series = Vec::new();
    for r in rows {
        let agreement: Option<f64> = r.try_get("agreement")?;
        let kappa: Option<f64> = r.try_get("kappa")?;
        series.push(RunAgreement {
            run_ref: r.try_get("primary_run_ref")?,
            audit_run_ref: r.try_get("audit_run_ref")?,
            dimension: r.try_get("dimension")?,
            sample_count: r.try_get("sample_count")?,
            excluded_example_count: r.try_get("excluded_example_count")?,
            agreement,
            kappa,
            reason: r.try_get("reason")?,
            meets_threshold: agreement.is_some_and(|a| a >= m.min_agreement)
                && kappa.is_some_and(|k| k >= m.min_kappa),
        });
    }
    Ok(series)
}

pub async fn build_snapshot(
    database: &Database,
    scope: SnapshotScope,
) -> Result<SnapshotReceipt, LensDefinitionError> {
    build_for_task(database, scope, "ops_title_label").await
}

pub(super) async fn build_for_task(
    database: &Database,
    scope: SnapshotScope,
    task: &str,
) -> Result<SnapshotReceipt, LensDefinitionError> {
    build_with_mode(database, scope, task, SnapshotMode::Strict).await
}

pub(super) async fn build_with_mode(
    database: &Database,
    scope: SnapshotScope,
    task: &str,
    mode: SnapshotMode,
) -> Result<SnapshotReceipt, LensDefinitionError> {
    // A concurrent generator may win the unique input hash while this transaction
    // still sees an older snapshot. Retry with a fresh PostgreSQL snapshot.
    for attempt in 0..3 {
        let mut tx = database.pool().begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await?;
        sqlx::query("LOCK TABLE linggan_ops_lens_adjudication IN SHARE MODE")
            .execute(&mut *tx)
            .await?;
        match build_in_with_mode(&mut tx, &scope, task, mode).await {
            Err(LensDefinitionError::Database(sqlx::Error::Database(e)))
                if e.code().as_deref() == Some("40001") && attempt < 2 =>
            {
                continue;
            }
            result => {
                let receipt = result?;
                tx.commit().await?;
                return Ok(receipt);
            }
        }
    }
    unreachable!()
}

pub(super) async fn build_in(
    conn: &mut PgConnection,
    scope: &SnapshotScope,
    task: &str,
) -> Result<SnapshotReceipt, LensDefinitionError> {
    build_in_with_mode(conn, scope, task, SnapshotMode::Strict).await
}

async fn build_in_with_mode(
    conn: &mut PgConnection,
    scope: &SnapshotScope,
    task: &str,
    mode: SnapshotMode,
) -> Result<SnapshotReceipt, LensDefinitionError> {
    scope.validate()?;
    if let Some(domain) = scope.domain_ref {
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT FROM linggan_ops_account WHERE domain_ref=$1 AND ((source_system='content_workbench' AND ledger_state='in_ledger') OR (source_system='linggan_native' AND native_record_state='active')))").bind(domain).fetch_one(&mut *conn).await?;
        if !exists {
            return Err(LensDefinitionError::InvalidScope);
        }
    }
    let current=sqlx::query(r#"SELECT t.definition_ref AS taxonomy_ref,t.version AS taxonomy_version,t.body AS vocabulary,
        m.definition_ref AS metric_ref,m.version AS metric_version,m.body AS metric,c.task_config_ref,c.prompt_version_ref,
        to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"') AS as_of
      FROM linggan_ops_lens_definition_current tc JOIN linggan_ops_lens_definition t ON t.definition_ref=tc.definition_ref
      CROSS JOIN linggan_ops_lens_definition_current mc JOIN linggan_ops_lens_definition m ON m.definition_ref=mc.definition_ref
      CROSS JOIN linggan_ai_task_setting ts JOIN linggan_ai_task_config c ON c.task_config_ref=ts.current_config_ref
      WHERE tc.kind='taxonomy' AND mc.kind='metric' AND ts.task_key=$1"#)
        .bind(task).fetch_optional(&mut *conn).await?.ok_or(LensDefinitionError::NoMethod)?;
    let Json(m): Json<MetricDefinition> = current.try_get("metric")?;
    let Json(vocab): Json<Vocabulary> = current.try_get("vocabulary")?;
    let taxonomy: Uuid = current.try_get("taxonomy_ref")?;
    let metric: Uuid = current.try_get("metric_ref")?;
    let method = LabelMethod {
        taxonomy_ref: taxonomy,
        task_config_ref: current.try_get("task_config_ref")?,
        prompt_version_ref: current.try_get("prompt_version_ref")?,
    };
    let mut input = read_input(conn, scope, method, mode).await?;
    let series = agreements(conn, &input, &m).await?;
    let latest = input
        .publications
        .values()
        .filter_map(|r| r.business_date.as_deref())
        .max();
    let stale_days: Option<i32> = sqlx::query_scalar(
        "SELECT (transaction_timestamp() AT TIME ZONE 'Asia/Shanghai')::date - $1::text::date",
    )
    .bind(latest)
    .fetch_one(&mut *conn)
    .await?;
    // A new audit may change the gate without changing the primary or its effective labels.
    input.agreement_sources = series
        .iter()
        .map(|a| (a.run_ref, a.audit_run_ref))
        .collect();
    let hash = input
        .input_hash()
        .map_err(|_| LensDefinitionError::InvalidDefinition)?;
    let find =
        "SELECT snapshot_ref FROM linggan_ops_lens_snapshot WHERE input_hash=$1 AND metric_ref=$2";
    let as_of: String = current.try_get("as_of")?;
    if let Some(snapshot_ref) = sqlx::query_scalar(find)
        .bind(&hash)
        .bind(metric)
        .fetch_optional(&mut *conn)
        .await?
    {
        set_current(conn, scope, snapshot_ref).await?;
        return Ok(SnapshotReceipt {
            snapshot_ref,
            reused: true,
        });
    }
    let id = Uuid::new_v4();
    let frozen =
        serde_json::to_value(&input).map_err(|_| LensDefinitionError::InvalidDefinition)?;
    // A confirmation has a human source but still equals the original machine value.
    // Count distinct titles with at least one currently corrected dimension in this scope.
    let decisions: Vec<_> = input
        .publications
        .values()
        .flat_map(|p| p.labels.values())
        .filter_map(|l| l.adjudication_ref.map(|a| (a, l.run_ref)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let (refs, runs): (Vec<_>, Vec<_>) = decisions.into_iter().unzip();
    let corrected: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT a.title_key)
        FROM unnest($1::uuid[],$2::uuid[]) x(decision,run)
        JOIN linggan_ops_lens_adjudication a ON a.adjudication_ref=x.decision
        JOIN linggan_ops_lens_label l ON l.run_ref=x.run AND l.title_key=a.title_key
        WHERE a.value <> (to_jsonb(l)->>a.dimension)",
    )
    .bind(refs)
    .bind(runs)
    .fetch_one(&mut *conn)
    .await?;
    let stale_after_days = m.stale_after_days;
    let mut body = tokio::task::spawn_blocking(move || summarize(&input, &vocab, &m, &series))
        .await
        .map_err(|_| LensDefinitionError::CalculationUnavailable)??;
    body["snapshotRef"] = json!(id);
    body["asOf"] = json!(as_of);
    let q = &mut body["quality"];
    q["correctedTitles"] = json!(corrected);
    q["taxonomyVersion"] = json!(current.try_get::<i32, _>("taxonomy_version")?);
    q["metricVersion"] = json!(current.try_get::<i32, _>("metric_version")?);
    q["staleDays"] = json!(stale_days);
    q["isStale"] = json!(stale_days.map(|v| i64::from(v) > i64::from(stale_after_days)));
    let inserted:Option<Uuid>=sqlx::query_scalar("INSERT INTO linggan_ops_lens_snapshot(snapshot_ref,as_of,taxonomy_ref,metric_ref,input_hash,analysis_input,body)
      VALUES ($1,$2::text::timestamptz,$3,$4,$5,$6,$7) ON CONFLICT (input_hash,metric_ref) DO NOTHING RETURNING snapshot_ref")
        .bind(id).bind(&as_of).bind(taxonomy).bind(metric).bind(&hash).bind(frozen).bind(body).fetch_optional(&mut *conn).await?;
    let snapshot_ref = match inserted {
        Some(snapshot_ref) => snapshot_ref,
        None => {
            sqlx::query_scalar(find)
                .bind(hash)
                .bind(metric)
                .fetch_one(&mut *conn)
                .await?
        }
    };
    set_current(conn, scope, snapshot_ref).await?;
    Ok(SnapshotReceipt {
        snapshot_ref,
        reused: inserted.is_none(),
    })
}

async fn set_current(
    conn: &mut PgConnection,
    scope: &SnapshotScope,
    snapshot_ref: Uuid,
) -> Result<(), sqlx::Error> {
    // The scoped upsert under REPEATABLE READ serializes concurrent activation.
    // An older MVCC view cannot overwrite a newer commit; it retries with 40001.
    sqlx::query(
        "INSERT INTO linggan_ops_lens_snapshot_current(domain_ref,published_from,published_to,snapshot_ref)
         VALUES($1,$2::text::date,$3::text::date,$4)
         ON CONFLICT (domain_ref,published_from,published_to) DO UPDATE
         SET snapshot_ref=EXCLUDED.snapshot_ref",
    )
    .bind(scope.domain_ref)
    .bind(&scope.from)
    .bind(&scope.to)
    .bind(snapshot_ref)
    .execute(conn)
    .await?;
    Ok(())
}
