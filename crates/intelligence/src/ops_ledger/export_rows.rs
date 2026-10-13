//! Runtime contract of the three content-workbench operation exports (`main@489b9cb8`).
//! Every row must carry exactly the known key set; values are type- and range-checked.
//! Errors name the file, row and field but never echo a value.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::OpsImportError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Platform {
    Xhs,
    Douyin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AccountStatus {
    Active,
    ViolationDisabled,
    Warming,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReviewStatus {
    Confirmed,
    Draft,
    PendingReview,
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EntrySource {
    Manual,
    CsvImport,
    TopicPublicationSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DataSyncStatus {
    Syncable,
    Synced,
    ContentUnavailable,
    NeedsLink,
    ManualFill,
    Excluded,
}

macro_rules! db_vocabulary {
    ($kind:ty { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl $kind {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            pub fn from_db(value: &str) -> Option<Self> {
                match value { $($text => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

db_vocabulary!(Platform { Xhs => "xhs", Douyin => "douyin" });
db_vocabulary!(AccountStatus {
    Active => "active", ViolationDisabled => "violation_disabled", Warming => "warming",
    Paused => "paused", Archived => "archived",
});
db_vocabulary!(ReviewStatus {
    Confirmed => "confirmed", Draft => "draft", PendingReview => "pending_review",
    Unassigned => "unassigned",
});
db_vocabulary!(EntrySource {
    Manual => "manual", CsvImport => "csv_import", TopicPublicationSync => "topic_publication_sync",
});
db_vocabulary!(DataSyncStatus {
    Syncable => "syncable", Synced => "synced", ContentUnavailable => "content_unavailable",
    NeedsLink => "needs_link", ManualFill => "manual_fill", Excluded => "excluded",
});

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountExportRow {
    pub id: String,
    pub workspace_id: String,
    pub platform: Platform,
    pub account_name: String,
    pub handle: Option<String>,
    pub device_code: Option<String>,
    pub profile_url: Option<String>,
    pub follower_count: Option<i64>,
    pub ip_location: Option<String>,
    pub operator_name: Option<String>,
    pub responsible_user_id: Option<String>,
    pub created_by_user_id: Option<String>,
    pub review_status: ReviewStatus,
    pub is_retained: bool,
    pub status: AccountStatus,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PublicationExportRow {
    pub id: String,
    pub workspace_id: String,
    pub platform: Platform,
    pub publication_url: String,
    pub canonical_url: String,
    pub published_at: String,
    pub title: Option<String>,
    pub copy: Option<String>,
    pub view_count: Option<i64>,
    pub like_count: Option<i64>,
    pub collect_count: Option<i64>,
    pub comment_count: Option<i64>,
    pub cover_click_rate: Option<f64>,
    pub publisher_name: Option<String>,
    pub responsible_user_id: Option<String>,
    pub created_by_user_id: Option<String>,
    pub review_status: ReviewStatus,
    pub account_name: Option<String>,
    pub operation_account_id: Option<String>,
    pub source: EntrySource,
    pub effective_publication: bool,
    pub data_sync_status: DataSyncStatus,
    pub data_synced_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DailyExportRow {
    pub id: String,
    pub workspace_id: String,
    pub operator_name: String,
    pub date_key: String,
    pub date: String,
    pub platform: String,
    pub account_name: String,
    pub links_opened_count: i32,
    pub followers_gained: i32,
    pub published_count: i64,
    pub viral_count: i64,
    pub responsible_user_id: Option<String>,
    pub created_by_user_id: Option<String>,
    pub review_status: ReviewStatus,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

pub(super) const ACCOUNT_KEYS: [&str; 24] = [
    "accountHealthAction",
    "accountHealthLabel",
    "accountHealthScore",
    "accountHealthStats",
    "accountHealthStatus",
    "accountName",
    "createdAt",
    "createdByUserId",
    "deletedAt",
    "deviceCode",
    "followerCount",
    "handle",
    "id",
    "ipLocation",
    "isRetained",
    "notes",
    "operatorName",
    "platform",
    "profileUrl",
    "responsibleUserId",
    "reviewStatus",
    "status",
    "updatedAt",
    "workspaceId",
];

/// Fields Mog confirmed are not migrated (filled 0–0.5%) or that the workbench never filled.
/// Non-null values are counted so that late entries before the cutover stay visible.
pub(super) const PUBLICATION_ABANDONED_KEYS: [&str; 15] = [
    "topicTitle",
    "topicKeyword",
    "imageType",
    "postType",
    "contentPosition",
    "trafficCount",
    "leadCount",
    "viralAnalysis",
    "issueAnalysis",
    "notes",
    "sourceTopicId",
    "contentAssetId",
    "contentCode",
    "originalUrl",
    "platformContentId",
];

pub(super) const PUBLICATION_KEYS: [&str; 49] = [
    "accountName",
    "canonicalUrl",
    "collectCount",
    "commentCount",
    "contentAssetId",
    "contentCode",
    "contentPosition",
    "copy",
    "coverClickRate",
    "createdAt",
    "createdByUserId",
    "dataResultStatus",
    "dataSyncObservedAt",
    "dataSyncReason",
    "dataSyncRevisionKey",
    "dataSyncStatus",
    "dataSyncedAt",
    "deletedAt",
    "duplicateLinkCount",
    "effectivePublication",
    "hasDuplicateLink",
    "id",
    "imageType",
    "issueAnalysis",
    "latestDataSyncTask",
    "leadCount",
    "likeCount",
    "media",
    "notes",
    "operationAccountId",
    "originalUrl",
    "platform",
    "platformContentId",
    "postType",
    "publicationUrl",
    "publishedAt",
    "publisherName",
    "responsibleUserId",
    "reviewStatus",
    "source",
    "sourceTopicId",
    "title",
    "topicKeyword",
    "topicTitle",
    "trafficCount",
    "updatedAt",
    "viewCount",
    "viralAnalysis",
    "workspaceId",
];

pub(super) const DAILY_KEYS: [&str; 28] = [
    "accountName",
    "createdAt",
    "createdByUserId",
    "date",
    "dateKey",
    "deletedAt",
    "effectivePublicationCount",
    "effectivePublicationRate",
    "executionStatus",
    "executionStatusAction",
    "executionStatusLabel",
    "followersGained",
    "id",
    "linkAddRate",
    "linksOpenedCount",
    "monthlyFollowersGained",
    "monthlyPublishedCount",
    "notes",
    "operatorName",
    "platform",
    "publicationStats",
    "publishedCount",
    "responsibleUserId",
    "reviewStatus",
    "updatedAt",
    "viralCount",
    "weekday",
    "workspaceId",
];

/// Which export a problem was found in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFile {
    Accounts,
    Publications,
    DailyMetrics,
}

impl ExportFile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accounts => "账号",
            Self::Publications => "发布记录",
            Self::DailyMetrics => "每日数据",
        }
    }
}

/// One parsed export row plus its raw object, kept only for counting dropped fields.
pub(super) struct ParsedRow<T> {
    pub row: T,
    pub raw: Map<String, Value>,
}

/// Parses a JSON array of objects whose keys must equal `keys` exactly.
pub(super) fn parse_rows<T: DeserializeOwned>(
    file: ExportFile,
    bytes: &[u8],
    keys: &[&str],
) -> Result<Vec<ParsedRow<T>>, OpsImportError> {
    let rows: Vec<Value> = serde_json::from_slice(bytes)
        .map_err(|_| OpsImportError::invalid(file, None, "整个文件", "不是 JSON 数组"))?;
    rows.into_iter()
        .enumerate()
        .map(|(index, value)| parse_row(file, index + 1, value, keys))
        .collect()
}

fn parse_row<T: DeserializeOwned>(
    file: ExportFile,
    row: usize,
    value: Value,
    keys: &[&str],
) -> Result<ParsedRow<T>, OpsImportError> {
    let Value::Object(raw) = value else {
        return Err(OpsImportError::invalid(file, Some(row), "整行", "不是对象"));
    };
    if let Some(missing) = keys.iter().find(|key| !raw.contains_key(**key)) {
        return Err(OpsImportError::invalid(
            file,
            Some(row),
            missing,
            "缺少字段",
        ));
    }
    if let Some(extra) = raw.keys().find(|key| !keys.contains(&key.as_str())) {
        return Err(OpsImportError::invalid(
            file,
            Some(row),
            extra,
            "未知字段，导出格式与已核对的版本不同",
        ));
    }
    if raw.values().any(contains_nul) {
        return Err(OpsImportError::invalid(
            file,
            Some(row),
            "文本字段",
            "含有空字符",
        ));
    }
    let parsed = serde_path_to_error::deserialize(Value::Object(raw.clone())).map_err(|error| {
        let field = error.path().to_string();
        OpsImportError::invalid(file, Some(row), &field, "类型或取值不符合导出格式")
    })?;
    Ok(ParsedRow { row: parsed, raw })
}

fn contains_nul(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains('\0'),
        Value::Array(items) => items.iter().any(contains_nul),
        Value::Object(map) => map.values().any(contains_nul),
        _ => false,
    }
}
