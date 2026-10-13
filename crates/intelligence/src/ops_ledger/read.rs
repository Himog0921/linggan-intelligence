//! Read models behind the 「运营」 pages (OPS-HTTP-001). Read-only: nothing here writes.
//! Only rows present in the latest export count; merged duplicate registrations only appear under
//! their kept record. Unknown stays `None`, never 0.

mod accounts;
mod daily;
mod overview;
mod publications;
mod summary;

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

pub use accounts::{OpsAccountItem, OpsAccountPage, read_ops_accounts};
pub use daily::{OpsDailyItem, OpsDailyPage, read_ops_daily};
pub use overview::{
    OpsOverview, OpsOverviewQuery, OpsOverviewTrend, OverviewSlice, read_ops_overview,
    read_ops_overview_trend,
};
pub use publications::{
    OpsPublicationFilter, OpsPublicationItem, OpsPublicationPage, OpsViews,
    read_ops_publication_export, read_ops_publications,
};
pub use summary::{OpsSummary, read_ops_summary};

/// Shared CTE: the latest view-count report per publication (`value` or `cleared`).
pub(super) const LATEST_VIEWS_CTE: &str = "latest_views AS ( \
    SELECT DISTINCT ON (publication_ref) publication_ref, view_count, \
           to_char(as_of AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS views_as_of \
      FROM linggan_ops_view_report \
     ORDER BY publication_ref, report_sequence DESC)";

/// The business selection of every operations read: all businesses or one domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpsBusiness {
    All,
    Domain(Uuid),
}

impl OpsBusiness {
    /// `None` and `all` mean every business; anything else must be a domain reference.
    pub fn parse(value: Option<&str>) -> Result<Self, OpsReadError> {
        match value.map(str::trim) {
            None | Some("") | Some("all") => Ok(Self::All),
            Some(text) => Uuid::parse_str(text)
                .map(Self::Domain)
                .map_err(|_| OpsReadError::InvalidParameter("domain")),
        }
    }

    pub(super) fn domain_ref(self) -> Option<Uuid> {
        match self {
            Self::All => None,
            Self::Domain(domain_ref) => Some(domain_ref),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpsReadError {
    #[error("ops_parameter_invalid")]
    InvalidParameter(&'static str),
    #[error("ops_business_not_found")]
    BusinessNotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// When the mirror was last refreshed: the latest applied import batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAsOf {
    pub exported_at: String,
    /// The same instant in Beijing time, `YYYY-MM-DD HH:MM`: the side-bar foot drops the year,
    /// the CSV export keeps it.
    pub exported_at_beijing: String,
    pub batch_ref: Uuid,
}

/// A business with at least one account still in the ledger; only those can be selected. A domain
/// whose accounts were all deleted in the workbench is history, not a current business.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsBusinessOption {
    pub domain_ref: Uuid,
    pub name: String,
    pub account_count: i64,
}

pub async fn read_ops_businesses(
    database: &Database,
) -> Result<Vec<OpsBusinessOption>, OpsReadError> {
    let rows = sqlx::query(
        "SELECT d.domain_ref, d.name, count(*) AS account_count \
           FROM observation_domain d \
           JOIN linggan_ops_account a \
             ON a.domain_ref = d.domain_ref AND a.ledger_state = 'in_ledger' \
          GROUP BY d.domain_ref, d.name, d.created_at \
          ORDER BY d.created_at, d.domain_ref",
    )
    .fetch_all(database.pool())
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(OpsBusinessOption {
                domain_ref: row.try_get("domain_ref")?,
                name: row.try_get("name")?,
                account_count: row.try_get("account_count")?,
            })
        })
        .collect()
}

pub async fn read_ops_as_of(database: &Database) -> Result<Option<OpsAsOf>, OpsReadError> {
    let row = sqlx::query(
        "SELECT batch_ref, to_char(exported_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS exported, \
                to_char(exported_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD HH24:MI') AS exported_beijing \
           FROM linggan_ops_import_batch ORDER BY applied_at DESC, batch_ref DESC LIMIT 1",
    )
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(OpsAsOf {
            exported_at: row.try_get("exported")?,
            exported_at_beijing: row.try_get("exported_beijing")?,
            batch_ref: row.try_get("batch_ref")?,
        })
    })
    .transpose()
}

/// A registered historical cover file, as the read route needs it to stream the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsCoverFile {
    pub storage_key: String,
    pub sha256: String,
    pub byte_size: i64,
    pub mime_type: String,
}

pub async fn read_ops_cover(
    database: &Database,
    publication_ref: Uuid,
) -> Result<Option<OpsCoverFile>, OpsReadError> {
    let row = sqlx::query(
        "SELECT storage_key, content_sha256, byte_size, mime_type \
           FROM linggan_ops_publication_cover WHERE publication_ref = $1",
    )
    .bind(publication_ref)
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(OpsCoverFile {
            storage_key: row.try_get("storage_key")?,
            sha256: row.try_get("content_sha256")?,
            byte_size: row.try_get("byte_size")?,
            mime_type: row.try_get("mime_type")?,
        })
    })
    .transpose()
}

/// A selected domain must exist and own at least one account; otherwise the selection is refused
/// instead of silently showing an empty business.
pub(super) async fn ensure_business(
    database: &Database,
    business: OpsBusiness,
) -> Result<(), OpsReadError> {
    let Some(domain_ref) = business.domain_ref() else {
        return Ok(());
    };
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_ops_account \
                         WHERE domain_ref = $1 AND ledger_state = 'in_ledger')",
    )
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await?;
    if known {
        Ok(())
    } else {
        Err(OpsReadError::BusinessNotFound)
    }
}

/// Distinct names for operator tabs, already sorted by the database.
pub(super) fn names(rows: Vec<sqlx::postgres::PgRow>) -> Result<Vec<String>, OpsReadError> {
    rows.into_iter()
        .map(|row| Ok(row.try_get::<String, _>(0)?))
        .collect()
}

/// Escapes `%`, `_` and `\` so that a user's text is matched literally inside `ILIKE`.
pub(super) fn like_contains(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('%');
    for character in text.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped.push('%');
    escaped
}

#[cfg(test)]
mod tests {
    use super::{OpsBusiness, like_contains};
    use uuid::Uuid;

    #[test]
    fn business_selection_accepts_all_or_a_domain_reference_only() {
        assert_eq!(OpsBusiness::parse(None).unwrap(), OpsBusiness::All);
        assert_eq!(OpsBusiness::parse(Some("all")).unwrap(), OpsBusiness::All);
        let domain = Uuid::new_v4();
        assert_eq!(
            OpsBusiness::parse(Some(&domain.to_string())).unwrap(),
            OpsBusiness::Domain(domain)
        );
        assert!(OpsBusiness::parse(Some("ADHD")).is_err());
    }

    #[test]
    fn account_search_matches_the_typed_text_literally() {
        assert_eq!(like_contains("a_b%c\\"), "%a\\_b\\%c\\\\%");
        assert_eq!(like_contains("账号"), "%账号%");
    }
}
