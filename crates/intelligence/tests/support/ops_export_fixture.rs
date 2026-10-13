//! Synthetic content-workbench operation exports for OPS-MIGRATION-001 D1 proofs. Every row has
//! the exact key set of the real export (`main@489b9cb8`); all values are invented.
#![allow(dead_code)]

use linggan_intelligence::ops_ledger::{ExportFiles, ImportRequest};
use serde_json::{Value, json};
use uuid::Uuid;

pub const ADHD_DOMAIN: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0001);
pub const WORKSPACE: &str = "synthetic-workspace";
pub const NOTE_1: &str = "6650a1b2c3d4e5f60718293a";
pub const NOTE_2: &str = "6650a1b2c3d4e5f60718293b";
pub const NOTE_3: &str = "6650a1b2c3d4e5f60718293c";
pub const NOTE_9: &str = "6650a1b2c3d4e5f607182939";

pub fn xhs_note(id: &str) -> String {
    format!("https://www.xiaohongshu.com/explore/{id}")
}

pub fn account(id: &str, platform: &str, name: &str, profile_url: Option<&str>) -> Value {
    json!({
        "accountHealthAction": "连续观察", "accountHealthLabel": "观察", "accountHealthScore": 60,
        "accountHealthStats": {"duplicateLinkCount": 0, "effectivePublicationCount": 0,
            "followersGained": 0, "linksOpenedCount": 0, "publicationCount": 0,
            "syncFailedPublicationCount": 0},
        "accountHealthStatus": "watch", "accountName": name,
        "createdAt": "2026-05-08T02:00:00.000Z", "createdByUserId": "user-owner", "deletedAt": null,
        "deviceCode": "SYN-01", "followerCount": 1200, "handle": null, "id": id,
        "ipLocation": "SYNTHETIC", "isRetained": true, "notes": null, "operatorName": "运营甲",
        "platform": platform, "profileUrl": profile_url, "responsibleUserId": "user-a",
        "reviewStatus": "confirmed", "status": "active", "updatedAt": "2026-10-01T02:00:00.000Z",
        "workspaceId": WORKSPACE,
    })
}

/// A publication registration. `account` is `(operationAccountId, accountName)`.
pub fn publication(
    id: &str,
    account: Option<(&str, &str)>,
    platform: &str,
    url: &str,
    published_at: &str,
    views: Option<i64>,
) -> Value {
    json!({
        "accountName": account.map(|(_, name)| name), "canonicalUrl": url, "collectCount": null,
        "commentCount": null, "contentAssetId": null, "contentCode": null, "contentPosition": null,
        "copy": null, "coverClickRate": null, "createdAt": published_at, "createdByUserId": "user-a",
        "dataResultStatus": "no_sync", "dataSyncObservedAt": null, "dataSyncReason": null,
        "dataSyncRevisionKey": null, "dataSyncStatus": "syncable", "dataSyncedAt": null,
        "deletedAt": null, "duplicateLinkCount": 1, "effectivePublication": false,
        "hasDuplicateLink": false, "id": id, "imageType": null, "issueAnalysis": null,
        "latestDataSyncTask": null, "leadCount": null, "likeCount": null,
        "media": {"contentKind": "image", "cover": null, "images": [], "label": "图文",
            "missing": [], "schemaVersion": "media-presentation-v1", "state": "historical_unavailable",
            "video": null},
        "notes": null, "operationAccountId": account.map(|(account_id, _)| account_id),
        "originalUrl": null, "platform": platform, "platformContentId": null, "postType": null,
        "publicationUrl": url, "publishedAt": published_at,
        "publisherName": account.map(|_| "运营甲"), "responsibleUserId": "user-a",
        "reviewStatus": "confirmed", "source": "manual", "sourceTopicId": null, "title": null,
        "topicKeyword": null, "topicTitle": null, "trafficCount": null,
        "updatedAt": "2026-10-01T03:00:00.000Z", "viewCount": views, "viralAnalysis": null,
        "workspaceId": WORKSPACE,
    })
}

pub fn daily(
    id: &str,
    operator: &str,
    date_key: &str,
    links: i64,
    followers: i64,
    published: i64,
) -> Value {
    json!({
        "accountName": "", "createdAt": format!("{date_key}T12:00:00.000Z"),
        "createdByUserId": "user-a", "date": format!("{date_key}T00:00:00.000Z"),
        "dateKey": date_key, "deletedAt": null, "effectivePublicationCount": 0,
        "effectivePublicationRate": null, "executionStatus": "needs_sync",
        "executionStatusAction": "补同步", "executionStatusLabel": "待同步",
        "followersGained": followers, "id": id, "linkAddRate": null, "linksOpenedCount": links,
        "monthlyFollowersGained": 0, "monthlyPublishedCount": 0, "notes": null,
        "operatorName": operator, "platform": "all",
        "publicationStats": {"effectivePublicationCount": 0, "publicationCount": published,
            "syncFailedPublicationCount": 0, "unsyncedPublicationCount": published},
        "publishedCount": published, "responsibleUserId": "user-a", "reviewStatus": "confirmed",
        "updatedAt": format!("{date_key}T13:00:00.000Z"), "viralCount": i64::from(published > 1),
        "weekday": "星期二", "workspaceId": WORKSPACE,
    })
}

/// The first synthetic export: duplicates by note id and by short link, a deleted account, a
/// 2024 date, a publication without account, Douyin and Xiaohongshu view counts around 5000.
pub struct Export {
    pub accounts: Vec<Value>,
    pub publications: Vec<Value>,
    pub daily: Vec<Value>,
}

pub fn first_export() -> Export {
    let profile = format!("https://www.xiaohongshu.com/user/profile/{NOTE_1}");
    let a1 = Some(("acc-1", "SYNTHETIC 账号甲"));
    let gone = Some(("acc-gone", "SYNTHETIC 已删号"));
    let mut p2 = publication(
        "pub-2",
        a1,
        "xhs",
        &format!("https://www.xiaohongshu.com/discovery/item/{NOTE_1}"),
        "2026-09-02T02:00:00.000Z",
        None,
    );
    p2["title"] = json!("SYNTHETIC 标题");
    let mut p4 = publication(
        "pub-4",
        a1,
        "xhs",
        "https://xhslink.com/a/SYN1",
        "2026-09-03T05:00:00.000Z",
        Some(200),
    );
    p4["canonicalUrl"] = json!("https://xhslink.com/a/SYN1");
    let mut p8 = publication(
        "pub-8",
        None,
        "xhs",
        &xhs_note(NOTE_2),
        "2026-09-30T16:30:00.000Z",
        Some(5000),
    );
    p8["publisherName"] = Value::Null;
    Export {
        accounts: vec![
            account("acc-1", "xhs", "SYNTHETIC 账号甲", Some(&profile)),
            account(
                "acc-2",
                "douyin",
                "SYNTHETIC 账号乙",
                Some("https://v.douyin.com/SYN/"),
            ),
            {
                let mut a3 = account("acc-3", "xhs", "SYNTHETIC 账号丙", None);
                a3["followerCount"] = Value::Null;
                a3
            },
        ],
        publications: vec![
            publication(
                "pub-1",
                a1,
                "xhs",
                &xhs_note(NOTE_1),
                "2026-09-01T02:00:00.000Z",
                Some(6000),
            ),
            p2,
            publication(
                "pub-3",
                a1,
                "xhs",
                "https://xhslink.com/a/SYN1",
                "2026-09-03T01:00:00.000Z",
                Some(100),
            ),
            p4,
            publication(
                "pub-5",
                gone,
                "xhs",
                &xhs_note(NOTE_3),
                "2026-09-04T02:00:00.000Z",
                Some(4999),
            ),
            publication(
                "pub-6",
                gone,
                "xhs",
                "https://xhslink.com/a/SYN6",
                "2024-03-05T02:00:00.000Z",
                None,
            ),
            publication(
                "pub-7",
                Some(("acc-2", "SYNTHETIC 账号乙")),
                "douyin",
                "https://v.douyin.com/SYN7/",
                "2026-09-05T02:00:00.000Z",
                Some(8000),
            ),
            p8,
        ],
        daily: vec![
            daily("day-1", "运营甲", "2026-09-01", 10, 4, 2),
            daily("day-2", "运营甲", "2026-09-02", 0, 0, 1),
            daily("day-3", "运营乙", "2026-10-01", 3, 1, 0),
        ],
    }
}

impl Export {
    pub fn request(&self, exported_at: &str) -> ImportRequest {
        ImportRequest {
            files: ExportFiles {
                accounts: serde_json::to_vec(&self.accounts).unwrap(),
                publications: serde_json::to_vec(&self.publications).unwrap(),
                daily: serde_json::to_vec(&self.daily).unwrap(),
            },
            exported_at: exported_at.to_owned(),
            domain_ref: ADHD_DOMAIN,
        }
    }
}
