//! CORPUS-CROSS-INDUSTRY-001 · 跨行业样本的只读投影。
//!
//! 这是与证据侧**完全分开的一条查询路径**。规格的接口红线写着：不得为跨行业给证据库
//! 接口增加任何参数或字段。两条路径不共享查询、不共享接口，隔离才不依赖任何人记得
//! 在某处加一个条件。
//!
//! 返回形状刻意与作品资源读取合同对齐（`items[].identity/display/...`），这样同一套
//! 页面组件可以直接渲染两种领域的材料——**界面统一，数据不合并**。

use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

/// 一次读取最多扫多少行。与证据侧同量级：页面一次能看的东西是有限的，
/// 读取预算要写在代码里，而不是等某个领域样本堆到几万条时才发现没有上限。
const CROSS_INDUSTRY_PAGE_SIZE: i64 = 50;

#[derive(Debug, thiserror::Error)]
pub enum CrossIndustryReadError {
    #[error("cross industry schema is not applied")]
    SchemaUnavailable,
    #[error("that domain is the home domain and has no cross-industry samples")]
    HomeDomainHasNoSamples,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// 读一个外部领域的样本。
///
/// 传入本领域会返回 [`CrossIndustryReadError::HomeDomainHasNoSamples`] 而不是空列表：
/// 本领域的材料在证据侧，它在这里「没有样本」不是一个关于数据的事实，而是问错了地方。
/// 空列表会让调用方以为「这个领域还没采过」，那是另一件事。
pub async fn read_cross_industry_samples(
    database: &Database,
    domain_ref: Uuid,
) -> Result<Value, CrossIndustryReadError> {
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample') IS NOT NULL \
                AND to_regclass('observation_domain') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !schema_ready {
        return Err(CrossIndustryReadError::SchemaUnavailable);
    }

    let is_own_domain: Option<bool> =
        sqlx::query_scalar("SELECT is_own_domain FROM observation_domain WHERE domain_ref = $1")
            .bind(domain_ref)
            .fetch_optional(database.pool())
            .await?;
    if is_own_domain == Some(true) {
        return Err(CrossIndustryReadError::HomeDomainHasNoSamples);
    }

    let rows: Vec<CrossIndustrySampleRow> = sqlx::query_as(
        "SELECT sample.sample_ref, sample.platform, sample.content_external_id, \
                sample.title, sample.author_name, sample.cover_local_asset_path, \
                sample.like_count, sample.collect_count, sample.comment_count, \
                to_char(sample.published_at, 'YYYY-MM-DD') AS published_on, \
                sample.keyword, sample.sort_order, \
                to_char(sample.last_observed_at, 'YYYY-MM-DD HH24:MI') AS last_observed_at, \
                target.display_name, \
                (SELECT count(*) FROM cross_industry_note note \
                 WHERE note.sample_ref = sample.sample_ref) AS note_count \
         FROM cross_industry_sample sample \
         LEFT JOIN collection_observation_target target \
                ON target.target_ref = sample.target_ref \
         WHERE sample.domain_ref = $1 \
         ORDER BY sample.last_observed_at DESC, sample.sample_ref \
         LIMIT $2",
    )
    .bind(domain_ref)
    .bind(CROSS_INDUSTRY_PAGE_SIZE)
    .fetch_all(database.pool())
    .await?;

    let truncated = rows.len() as i64 == CROSS_INDUSTRY_PAGE_SIZE;
    let items: Vec<Value> = rows.into_iter().map(sample_item).collect();
    Ok(json!({
        // 与证据侧不同的 scope 名。读的是什么，回执上必须能分辨，不能靠调用方记得自己问了谁。
        "queryScope": "cross_industry_sample",
        "items": items,
        "truncated": truncated,
        // 跨行业侧目前不做游标翻页：一个领域的样本量按设计是「每轮 20 篇」量级，
        // 先给出真实的截断标记，等真实数据超过一页再谈翻页，不预先造一个没人走过的分支。
        "cursor": Value::Null,
    }))
}

/// 一条评论正文最多返回多少字符。
///
/// 按**字符**而非字节切，中文一个字是 3 字节，按字节切会把一个汉字劈成半个。证据侧
/// 的有界摘录早已踩过这一条。
const CROSS_INDUSTRY_BODY_CHARS: usize = 500;

/// 读一个外部领域的评论原声。
///
/// 返回形状刻意与评论研究页现有的原声列表对齐（`page.items[]` + `works[]`），同一套
/// 渲染因此可以直接吃两种领域的数据——**界面统一，数据不合并**。这里读的是
/// `cross_industry_comment`，一个字段都不经过证据侧接口。
pub async fn read_cross_industry_comments(
    database: &Database,
    domain_ref: Uuid,
) -> Result<Value, CrossIndustryReadError> {
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_comment') IS NOT NULL \
                AND to_regclass('observation_domain') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !schema_ready {
        return Err(CrossIndustryReadError::SchemaUnavailable);
    }

    let is_own_domain: Option<bool> =
        sqlx::query_scalar("SELECT is_own_domain FROM observation_domain WHERE domain_ref = $1")
            .bind(domain_ref)
            .fetch_optional(database.pool())
            .await?;
    if is_own_domain == Some(true) {
        return Err(CrossIndustryReadError::HomeDomainHasNoSamples);
    }

    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cross_industry_comment WHERE domain_ref = $1")
            .bind(domain_ref)
            .fetch_one(database.pool())
            .await?;

    let rows: Vec<CrossIndustryCommentRow> = sqlx::query_as(
        "SELECT comment.comment_ref, comment.sample_ref, comment.body_text, comment.is_reply, \
                to_char(comment.observed_at, 'YYYY-MM-DD HH24:MI') AS observed_at, \
                sample.title, sample.author_name \
         FROM cross_industry_comment comment \
         JOIN cross_industry_sample sample ON sample.sample_ref = comment.sample_ref \
         WHERE comment.domain_ref = $1 \
         ORDER BY comment.observed_at DESC, comment.comment_ref \
         LIMIT $2",
    )
    .bind(domain_ref)
    .bind(CROSS_INDUSTRY_PAGE_SIZE)
    .fetch_all(database.pool())
    .await?;

    let truncated = rows.len() as i64 == CROSS_INDUSTRY_PAGE_SIZE;
    let mut works: Vec<Value> = Vec::new();
    let mut seen: Vec<Uuid> = Vec::new();
    let mut items: Vec<Value> = Vec::new();
    for (comment_ref, sample_ref, body_text, is_reply, observed_at, title, author_name) in rows {
        if !seen.contains(&sample_ref) {
            seen.push(sample_ref);
            works.push(json!({
                "workRef": sample_ref,
                "title": title,
                "creatorDisplayName": author_name,
            }));
        }
        let full = body_text.unwrap_or_default();
        let clipped: String = full.chars().take(CROSS_INDUSTRY_BODY_CHARS).collect();
        let body_truncated = clipped.chars().count() < full.chars().count();
        items.push(json!({
            "sourceRef": comment_ref,
            "workRef": sample_ref,
            // 正文缺失给 null 而不是空串：「没取到」与「是一条空评论」不是一件事。
            "body": if clipped.is_empty() { Value::Null } else { Value::String(clipped) },
            "isReply": is_reply,
            "bodyTruncated": body_truncated,
            "observedAt": observed_at,
        }));
    }

    Ok(json!({
        "queryScope": "cross_industry_comment",
        "page": { "total": total, "items": items, "nextCursor": Value::Null },
        "works": works,
        "truncated": truncated,
    }))
}

type CrossIndustryCommentRow = (
    Uuid,
    Uuid,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
);

type CrossIndustrySampleRow = (
    Uuid,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
);

/// 一条样本的展示形状。
///
/// 每个可能缺失的字段都带自己的状态：`UNKNOWN` 表示还没观察到，不是 0 也不是空串。
/// 跨行业样本大量来自列表面，互动数缺失是常态，把它显示成 0 会让「没看到」变成
/// 「确实是零」——这正是本项目在评论数上刚踩过的坑。
fn sample_item(row: CrossIndustrySampleRow) -> Value {
    let (
        sample_ref,
        platform,
        content_external_id,
        title,
        author_name,
        cover_local_asset_path,
        like_count,
        collect_count,
        comment_count,
        published_on,
        keyword,
        sort_order,
        last_observed_at,
        target_display_name,
        note_count,
    ) = row;
    json!({
        "identity": {
            "sampleRef": sample_ref,
            "platform": platform,
            "contentExternalId": content_external_id,
        },
        "display": {
            "title": title,
            "titleState": state_of(&title),
            "authorName": author_name,
            "authorNameState": state_of(&author_name),
            "publishedOn": published_on,
            "publishedOnState": state_of(&published_on),
            "engagement": {
                "likeCount": like_count,
                "likeCountState": count_state(like_count),
                "collectCount": collect_count,
                "collectCountState": count_state(collect_count),
                "commentCount": comment_count,
                "commentCountState": count_state(comment_count),
            },
        },
        // 「这条为什么会出现在这里」。没有它就无法复核一条样本的来路，
        // 所以关键词与排序即使为空也要显式给出，而不是省略字段。
        "provenance": {
            "keyword": keyword,
            "sortOrder": sort_order,
            "observationTarget": target_display_name,
            "lastObservedAt": last_observed_at,
        },
        "cover": {
            "localAssetUrl": cover_local_asset_path,
            "state": state_of(&cover_local_asset_path),
        },
        "noteCount": note_count,
    })
}

fn state_of(value: &Option<String>) -> &'static str {
    match value {
        Some(value) if !value.trim().is_empty() => "KNOWN",
        _ => "UNKNOWN",
    }
}

fn count_state(value: Option<i64>) -> &'static str {
    match value {
        Some(_) => "KNOWN",
        None => "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: Option<&str>, likes: Option<i64>) -> CrossIndustrySampleRow {
        (
            Uuid::new_v4(),
            "xhs".to_owned(),
            "note-1".to_owned(),
            title.map(str::to_owned),
            Some("某作者".to_owned()),
            None,
            likes,
            None,
            None,
            None,
            Some("学不进去".to_owned()),
            Some("most_liked".to_owned()),
            Some("2026-09-07 10:00".to_owned()),
            None,
            0,
        )
    }

    #[test]
    fn a_missing_count_stays_unknown_instead_of_zero() {
        // 跨行业样本大量来自列表面，互动数缺失是常态。把它显示成 0 会把「没看到」
        // 说成「确实是零」——本项目刚在评论数上踩过这个坑。
        let item = sample_item(row(Some("标题"), None));
        assert_eq!(item["display"]["engagement"]["likeCount"], Value::Null);
        assert_eq!(item["display"]["engagement"]["likeCountState"], "UNKNOWN");

        let observed = sample_item(row(Some("标题"), Some(0)));
        assert_eq!(observed["display"]["engagement"]["likeCount"], 0);
        // 真实观察到的零是 KNOWN，与「没看到」必须分得开。
        assert_eq!(observed["display"]["engagement"]["likeCountState"], "KNOWN");
    }

    #[test]
    fn an_empty_title_is_unknown_not_an_empty_string() {
        assert_eq!(
            sample_item(row(None, None))["display"]["titleState"],
            "UNKNOWN"
        );
        assert_eq!(
            sample_item(row(Some("   "), None))["display"]["titleState"],
            "UNKNOWN"
        );
        assert_eq!(
            sample_item(row(Some("标题"), None))["display"]["titleState"],
            "KNOWN"
        );
    }

    #[test]
    fn provenance_is_always_present_even_when_empty() {
        // 「这条为什么在这里」必须可复核。字段可以为空，但不能省略——省略会让调用方
        // 无法区分「这条没有采样口径」和「这个接口不返回口径」。
        let item = sample_item(row(Some("标题"), Some(1)));
        assert!(item["provenance"].is_object());
        assert!(
            item["provenance"]
                .as_object()
                .unwrap()
                .contains_key("keyword")
        );
        assert!(
            item["provenance"]
                .as_object()
                .unwrap()
                .contains_key("sortOrder")
        );
        assert!(
            item["provenance"]
                .as_object()
                .unwrap()
                .contains_key("observationTarget")
        );
    }
}
