//! 发布记录 list (OPS-HTTP-001 §3): kept registrations of the latest export, 50 per page.

use std::collections::HashMap;

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use super::{
    LATEST_VIEWS_CTE, OpsAsOf, OpsBusiness, OpsReadError, ensure_business, like_contains, names,
    read_ops_as_of,
};
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;

pub const OPS_PAGE_SIZE: i64 = 50;

/// Validated filters of the publication list. Dates are Beijing business days, inclusive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpsPublicationFilter {
    pub operator: Option<String>,
    pub platform: Option<String>,
    pub account: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub pending_views: bool,
    pub page: i64,
}

impl OpsPublicationFilter {
    /// Rejects anything the list cannot honour instead of silently ignoring it.
    pub fn validated(mut self) -> Result<Self, OpsReadError> {
        for value in [&mut self.operator, &mut self.account] {
            *value = value
                .take()
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty());
            if value
                .as_ref()
                .is_some_and(|text| text.chars().count() > 200)
            {
                return Err(OpsReadError::InvalidParameter("text_too_long"));
            }
        }
        if let Some(platform) = self.platform.as_deref().filter(|text| !text.is_empty()) {
            if platform != "xhs" && platform != "douyin" {
                return Err(OpsReadError::InvalidParameter("platform"));
            }
        } else {
            self.platform = None;
        }
        for (name, value) in [("from", &mut self.from), ("to", &mut self.to)] {
            *value = value.take().filter(|text| !text.is_empty());
            if value
                .as_deref()
                .is_some_and(|text| !crate::ops_ledger::is_business_date(text))
            {
                return Err(OpsReadError::InvalidParameter(name));
            }
        }
        if self.page < 1 || self.page > 100_000 {
            return Err(OpsReadError::InvalidParameter("page"));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsViews {
    /// `value` filled; `pending` Xiaohongshu not filled; `optional` Douyin not filled.
    pub state: &'static str,
    pub value: Option<i64>,
    pub as_of: Option<String>,
    pub overdue: bool,
    pub hit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsPublicationItem {
    pub r#ref: Uuid,
    /// The workbench's own record number, kept as the provenance anchor (exports carry it).
    pub source_id: String,
    /// `imported` records came from the historical workbench export; `native` records were
    /// created in Operations.
    pub source: &'static str,
    pub business_date: String,
    pub published_at: String,
    pub date_state: String,
    pub platform: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub url: String,
    pub account: Option<OpsAccountRef>,
    pub publisher: Option<String>,
    pub views: OpsViews,
    pub likes: Option<i64>,
    pub collects: Option<i64>,
    pub comments: Option<i64>,
    pub cover_click_rate: Option<f64>,
    pub system_check: &'static str,
    /// The newest explicit Operations sync command for this publication.  It is a read of the
    /// shared collection chain, not a mutation of the ledger or a claim that fields were adopted.
    pub sync: OpsPublicationSync,
    pub duplicates: Vec<OpsDuplicate>,
    pub cover: OpsCoverState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsPublicationSync {
    pub request_ref: Option<Uuid>,
    pub decision: Option<String>,
    pub state: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAccountRef {
    pub name: String,
    pub ledger_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsDuplicate {
    pub business_date: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OpsCoverState {
    pub available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsPublicationPage {
    pub as_of: Option<OpsAsOf>,
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub total_pages: i64,
    pub operators: Vec<String>,
    pub items: Vec<OpsPublicationItem>,
}

/// Rows of the current business that the filters keep. `$1` business domain, `$2` operator,
/// `$3` platform, `$4` account pattern, `$5`/`$6` date range, `$7` pending views only.
const FILTERED: &str = "kept AS ( \
    SELECT p.*, a.account_name, a.ledger_state, publisher.display_name AS native_publisher_name, \
           lv.view_count, lv.views_as_of, sync.request_ref AS sync_request_ref, \
           sync.decision AS sync_decision, sync.sync_state \
      FROM linggan_ops_publication p \
      LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
      LEFT JOIN linggan_ops_member publisher ON publisher.member_ref = p.native_publisher_member_ref \
      LEFT JOIN latest_views lv ON lv.publication_ref = p.publication_ref \
      LEFT JOIN LATERAL ( \
           SELECT item.request_ref,item.decision, \
                  CASE \
                    WHEN item.decision NOT IN ('accepted','existing') THEN 'not_submitted' \
                    WHEN projection.content_public_ref IS NOT NULL THEN 'material_projected' \
                    WHEN receipt.receipt_ref IS NOT NULL AND receipt.execution_effect='COMPLETED_LIVE_STEP' THEN 'receipt_accepted_projection_pending' \
                    WHEN receipt.receipt_ref IS NOT NULL AND receipt.execution_effect='LOST_AUTHORITY' THEN 'receipt_lost_authority' \
                    WHEN receipt.receipt_ref IS NOT NULL THEN 'receipt_accepted' \
                    WHEN package.package_ref IS NOT NULL THEN 'package_pending_receipt' \
                    WHEN attempt.attempt_id IS NOT NULL THEN 'collecting' \
                    WHEN task.execution_state='in_progress' THEN 'awaiting_collection' \
                    WHEN work.queue_state='queued' THEN 'queued' \
                    WHEN work.queue_state='leased' THEN 'waiting_execution' \
                    WHEN work.queue_state='completed' THEN 'no_qualified_result' \
                    WHEN work.queue_state='cancelled' THEN 'stopped' \
                    ELSE 'chain_unavailable' \
                  END AS sync_state \
             FROM linggan_ops_publication_sync_item item \
             LEFT JOIN collection_work_order work ON work.work_order_ref=item.work_order_ref \
             LEFT JOIN LATERAL ( \
                  SELECT lease_task.task_id,lease_task.execution_state \
                    FROM collection_work_order_lease lease \
                    JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
                   WHERE lease.work_order_ref=item.work_order_ref \
                   ORDER BY lease.issued_at DESC,lease_task.sequence_no ASC LIMIT 1 \
             ) task ON true \
             LEFT JOIN LATERAL ( \
                  SELECT attempt_id FROM linggan_runtime_attempt \
                   WHERE task_id=task.task_id ORDER BY started_at DESC,attempt_id DESC LIMIT 1 \
             ) attempt ON true \
             LEFT JOIN linggan_runtime_capture_package package ON package.attempt_id=attempt.attempt_id \
             LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
             LEFT JOIN linggan_ops_publication_material_projection projection ON projection.receipt_ref=receipt.receipt_ref \
            WHERE item.requested_publication_ref=p.publication_ref \
            ORDER BY item.created_at DESC,item.ordinal DESC LIMIT 1 \
      ) sync ON true \
     WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
       AND (p.source_system <> 'linggan_native' OR p.native_record_state = 'active') \
       AND ($1::uuid IS NULL OR a.domain_ref = $1) \
       AND ($2::text IS NULL OR coalesce(p.publisher_name,publisher.display_name) = $2) \
       AND ($3::text IS NULL OR p.platform = $3) \
       AND ($4::text IS NULL OR a.account_name ILIKE $4 ESCAPE '\\') \
       AND ($5::date IS NULL OR coalesce(p.published_business_date,p.native_published_on) >= $5::date) \
       AND ($6::date IS NULL OR coalesce(p.published_business_date,p.native_published_on) <= $6::date) \
       AND (NOT $7 OR (p.platform = 'xhs' AND lv.view_count IS NULL)))";

pub async fn read_ops_publications(
    database: &Database,
    business: OpsBusiness,
    filter: OpsPublicationFilter,
) -> Result<OpsPublicationPage, OpsReadError> {
    let filter = filter.validated()?;
    ensure_business(database, business).await?;
    let count_sql = format!("WITH {LATEST_VIEWS_CTE}, {FILTERED} SELECT count(*) FROM kept");
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql))
        .bind(business.domain_ref())
        .bind(filter.operator.as_deref())
        .bind(filter.platform.as_deref())
        .bind(filter.account.as_deref().map(like_contains))
        .bind(filter.from.as_deref())
        .bind(filter.to.as_deref())
        .bind(filter.pending_views)
        .fetch_one(database.pool())
        .await?;
    let items = read_page(database, business, &filter, Some(OPS_PAGE_SIZE)).await?;
    let operators = names(
        sqlx::query(
            "SELECT DISTINCT coalesce(p.publisher_name,publisher.display_name) AS publisher_name \
               FROM linggan_ops_publication p \
               LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
               LEFT JOIN linggan_ops_member publisher ON publisher.member_ref = p.native_publisher_member_ref \
              WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
                AND (p.source_system <> 'linggan_native' OR p.native_record_state = 'active') \
                AND coalesce(p.publisher_name,publisher.display_name) IS NOT NULL \
                AND ($1::uuid IS NULL OR a.domain_ref = $1) \
              ORDER BY 1",
        )
        .bind(business.domain_ref())
        .fetch_all(database.pool())
        .await?,
    )?;
    Ok(OpsPublicationPage {
        as_of: read_ops_as_of(database).await?,
        page: filter.page,
        page_size: OPS_PAGE_SIZE,
        total,
        total_pages: (total + OPS_PAGE_SIZE - 1) / OPS_PAGE_SIZE,
        operators,
        items,
    })
}

/// The page itself. Pending view lists go oldest first so the most overdue posts lead.
async fn read_page(
    database: &Database,
    business: OpsBusiness,
    filter: &OpsPublicationFilter,
    limit: Option<i64>,
) -> Result<Vec<OpsPublicationItem>, OpsReadError> {
    let sql = format!(
        "WITH {LATEST_VIEWS_CTE}, {FILTERED} \
         SELECT kept.publication_ref, kept.source_record_id, kept.source_system, \
                coalesce(kept.published_business_date,kept.native_published_on)::text AS business_date, \
                coalesce(to_char(kept.published_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'), \
                         kept.native_published_on::text) AS published, \
                kept.published_date_state, kept.platform, kept.title, kept.body, kept.publication_url, \
                kept.account_name, kept.ledger_state, \
                coalesce(kept.publisher_name,kept.native_publisher_name) AS publisher_name, \
                kept.view_count, kept.views_as_of, \
                ((now() AT TIME ZONE 'Asia/Shanghai')::date - coalesce(kept.published_business_date,kept.native_published_on)) >= 7 AS aged, \
                kept.like_count, kept.collect_count, kept.comment_count, kept.cover_click_rate, \
                kept.sync_request_ref, kept.sync_decision, coalesce(kept.sync_state,'not_requested') AS sync_state, \
                cover.publication_ref IS NOT NULL AS has_cover \
           FROM kept LEFT JOIN linggan_ops_publication_cover cover ON cover.publication_ref = kept.publication_ref \
          ORDER BY CASE WHEN $7 THEN coalesce(kept.published_at,kept.native_published_on::timestamp) END ASC, \
                   CASE WHEN NOT $7 THEN coalesce(kept.published_at,kept.native_published_on::timestamp) END DESC, \
                   kept.source_created_at DESC NULLS LAST, kept.publication_ref DESC \
          LIMIT $9 OFFSET $8"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(business.domain_ref())
        .bind(filter.operator.as_deref())
        .bind(filter.platform.as_deref())
        .bind(filter.account.as_deref().map(like_contains))
        .bind(filter.from.as_deref())
        .bind(filter.to.as_deref())
        .bind(filter.pending_views)
        .bind((filter.page - 1) * OPS_PAGE_SIZE)
        .bind(limit)
        .fetch_all(database.pool())
        .await?;
    let mut items = rows.iter().map(item).collect::<Result<Vec<_>, _>>()?;
    attach_duplicates(database, &mut items).await?;
    Ok(items)
}

/// Every kept registration of the business, unpaged, in list order (for 导出).
pub async fn read_ops_publication_export(
    database: &Database,
    business: OpsBusiness,
) -> Result<Vec<OpsPublicationItem>, OpsReadError> {
    ensure_business(database, business).await?;
    let filter = OpsPublicationFilter {
        page: 1,
        ..Default::default()
    };
    read_page(database, business, &filter, None).await
}

fn item(row: &sqlx::postgres::PgRow) -> Result<OpsPublicationItem, OpsReadError> {
    let platform: String = row.try_get("platform")?;
    let views = views(
        &platform,
        row.try_get("view_count")?,
        row.try_get("views_as_of")?,
        row.try_get("aged")?,
    );
    let account_name: Option<String> = row.try_get("account_name")?;
    let ledger_state: Option<String> = row.try_get("ledger_state")?;
    Ok(OpsPublicationItem {
        r#ref: row.try_get("publication_ref")?,
        source_id: row.try_get("source_record_id")?,
        source: source(row.try_get("source_system")?)?,
        business_date: row.try_get("business_date")?,
        published_at: row.try_get("published")?,
        date_state: row.try_get("published_date_state")?,
        platform,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        url: row.try_get("publication_url")?,
        account: account_name
            .zip(ledger_state)
            .map(|(name, ledger_state)| OpsAccountRef { name, ledger_state }),
        publisher: row.try_get("publisher_name")?,
        views,
        likes: row.try_get("like_count")?,
        collects: row.try_get("collect_count")?,
        comments: row.try_get("comment_count")?,
        cover_click_rate: row.try_get("cover_click_rate")?,
        system_check: "not_checked",
        sync: OpsPublicationSync {
            request_ref: row.try_get("sync_request_ref")?,
            decision: row.try_get("sync_decision")?,
            state: sync_state(&row.try_get::<String, _>("sync_state")?)?,
        },
        duplicates: Vec::new(),
        cover: OpsCoverState {
            available: row.try_get("has_cover")?,
        },
    })
}

fn sync_state(value: &str) -> Result<&'static str, OpsReadError> {
    match value {
        "not_requested" => Ok("not_requested"),
        "not_submitted" => Ok("not_submitted"),
        "queued" => Ok("queued"),
        "waiting_execution" => Ok("waiting_execution"),
        "awaiting_collection" => Ok("awaiting_collection"),
        "collecting" => Ok("collecting"),
        "package_pending_receipt" => Ok("package_pending_receipt"),
        "receipt_accepted_projection_pending" => Ok("receipt_accepted_projection_pending"),
        "receipt_lost_authority" => Ok("receipt_lost_authority"),
        "receipt_accepted" => Ok("receipt_accepted"),
        "material_projected" => Ok("material_projected"),
        "no_qualified_result" => Ok("no_qualified_result"),
        "stopped" => Ok("stopped"),
        "chain_unavailable" => Ok("chain_unavailable"),
        _ => Err(OpsReadError::InvalidParameter("publication_sync_state")),
    }
}

fn source(source_system: String) -> Result<&'static str, OpsReadError> {
    match source_system.as_str() {
        "content_workbench" => Ok("imported"),
        "linggan_native" => Ok("native"),
        _ => Err(OpsReadError::InvalidParameter("publication_source")),
    }
}

/// DEC-0009 rules: only Xiaohongshu views are required and judged; unknown is never 0.
pub(crate) fn views(
    platform: &str,
    value: Option<i64>,
    as_of: Option<String>,
    aged: bool,
) -> OpsViews {
    let xhs = platform == "xhs";
    match value {
        Some(count) => OpsViews {
            state: "value",
            value: Some(count),
            as_of,
            overdue: false,
            hit: xhs && count >= XHS_HIT_VIEW_THRESHOLD,
        },
        None => OpsViews {
            state: if xhs { "pending" } else { "optional" },
            value: None,
            as_of: None,
            overdue: xhs && aged,
            hit: false,
        },
    }
}

async fn attach_duplicates(
    database: &Database,
    items: &mut [OpsPublicationItem],
) -> Result<(), OpsReadError> {
    if items.is_empty() {
        return Ok(());
    }
    let refs: Vec<Uuid> = items.iter().map(|item| item.r#ref).collect();
    let rows = sqlx::query(
        "SELECT merged_into_ref, coalesce(published_business_date,native_published_on)::text AS business_date \
           FROM linggan_ops_publication WHERE merged_into_ref = ANY($1) \
          ORDER BY published_at, publication_ref",
    )
    .bind(&refs)
    .fetch_all(database.pool())
    .await?;
    let mut by_kept: HashMap<Uuid, Vec<OpsDuplicate>> = HashMap::new();
    for row in rows {
        by_kept
            .entry(row.try_get("merged_into_ref")?)
            .or_default()
            .push(OpsDuplicate {
                business_date: row.try_get("business_date")?,
            });
    }
    for item in items {
        item.duplicates = by_kept.remove(&item.r#ref).unwrap_or_default();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OpsPublicationFilter, views};

    #[test]
    fn only_xiaohongshu_views_are_required_and_judged() {
        assert!(views("xhs", Some(5000), None, false).hit);
        assert!(!views("xhs", Some(4999), None, true).hit);
        assert!(
            !views("douyin", Some(80000), None, true).hit,
            "Douyin is never a hit"
        );
        let pending = views("xhs", None, None, true);
        assert_eq!(
            (pending.state, pending.value, pending.overdue),
            ("pending", None, true)
        );
        let optional = views("douyin", None, None, true);
        assert_eq!(
            (optional.state, optional.overdue),
            ("optional", false),
            "Douyin is never overdue"
        );
        assert_eq!(
            views("xhs", Some(0), None, true).value,
            Some(0),
            "a reported 0 stays 0"
        );
    }

    #[test]
    fn filters_reject_what_the_list_cannot_honour() {
        let base = OpsPublicationFilter {
            page: 1,
            ..Default::default()
        };
        assert!(
            OpsPublicationFilter {
                platform: Some("weibo".into()),
                ..base.clone()
            }
            .validated()
            .is_err()
        );
        assert!(
            OpsPublicationFilter {
                from: Some("2026-13-01".into()),
                ..base.clone()
            }
            .validated()
            .is_err()
        );
        assert!(
            OpsPublicationFilter {
                page: 0,
                ..base.clone()
            }
            .validated()
            .is_err()
        );
        let cleaned = OpsPublicationFilter {
            operator: Some("  ".into()),
            platform: Some(String::new()),
            ..base
        }
        .validated()
        .unwrap();
        assert_eq!((cleaned.operator, cleaned.platform), (None, None));
    }
}
