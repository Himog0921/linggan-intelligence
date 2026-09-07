//! CORPUS-CROSS-INDUSTRY-001 · 落库分流。
//!
//! 采集链路对两种领域完全相同：外部领域的博主与关键词就是普通观察目标，走同一套
//! 目标 → 准入 → 工单 → 租约 → 任务 → Package 的路，插件不需要知道领域的存在。
//! **领域只在落库那一刻决定材料去哪张表**，分流就发生在这里。
//!
//! 这条路径是三选一的结果。只靠读取时按领域过滤最省事，但那与「一个接口加参数」
//! 同构——漏写一次即破且无声，将来任何读证据的地方（选题评估、Topic 分析、AI 判断、
//! 导出）忘了加条件，外部领域内容就会被当作本领域证据使用。为跨行业重建一套采集
//! 链路则代价过高。复用链路 + 落库分流两者兼得。
//!
//! 分流本身仍是应用层判断，所以 `0044` 给证据侧补上了对称约束
//! （`CHECK (is_own_domain = true)` + 复合外键）：万一这里判错，写入会被数据库拒绝
//! 而不是静默把参照物混进证据。**失败远好过污染。**

use crate::material_admission::{
    exact_nonnegative_count, exact_string, known_state, observed_cover_url, target_string,
};
use crate::producer_runtime::ProducerRuntimeError;
use linggan_contracts::ProducerCapturePackage;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

/// 一批材料归属的领域。
pub(crate) enum PackageDomain {
    /// 本领域，或无法判定。材料走证据侧，与本模块无关。
    Home,
    External(ExternalDomain),
}

pub(crate) struct ExternalDomain {
    pub domain_ref: Uuid,
    /// 带回这批材料的观察目标。样本存着它，是为了日后能回答「这条是谁带回来的」。
    pub target_ref: Option<Uuid>,
}

/// 这批材料属于哪个领域。
///
/// 判不出来时一律按本领域处理，与 `0041` 对既有观察目标的处置一致（列可空 + 读取时
/// 回落）：历史上的采集全部发生在只有 ADHD 的时候，把它们算作别的领域会改写历史。
///
/// 这个回落有一个已知的方向性风险：外部领域的 package 若因链路异常查不到目标，会被
/// 当作本领域。它不会造成静默污染——证据侧的 `CHECK (is_own_domain = true)` 与复合
/// 外键会在写入时拒绝，采集失败并留下错误，而不是把参照物混进证据。
pub(crate) async fn resolve_package_domain(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
) -> Result<PackageDomain, ProducerRuntimeError> {
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('observation_domain') IS NOT NULL \
                AND to_regclass('cross_industry_sample') IS NOT NULL",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !schema_ready {
        return Ok(PackageDomain::Home);
    }

    let row: Option<(Uuid, bool, Option<Uuid>)> = sqlx::query_as(
        "SELECT domain.domain_ref, domain.is_own_domain, target.target_ref \
         FROM linggan_runtime_capture_package package \
         JOIN collection_work_order_lease_task lease_task ON lease_task.task_id = package.task_id \
         JOIN collection_work_order_lease lease ON lease.lease_ref = lease_task.lease_ref \
         JOIN collection_work_order work_order ON work_order.work_order_ref = lease.work_order_ref \
         JOIN collection_observation_target target ON target.target_ref = work_order.target_ref \
         JOIN observation_domain domain ON domain.domain_ref = target.domain_ref \
         WHERE package.package_ref = $1",
    )
    .bind(package.package_ref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;

    Ok(match row {
        Some((domain_ref, false, target_ref)) => PackageDomain::External(ExternalDomain {
            domain_ref,
            target_ref,
        }),
        _ => PackageDomain::Home,
    })
}

/// 把一批外部领域的材料写进跨行业侧。
///
/// 只处理列表、详情与评论三种。媒体（封面与多图下载）尚未接通，遇到时原样跳过而不是
/// 假装处理过——规格要求跨行业复用同一条媒体链路，那是下一步的工作。
pub(crate) async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    domain: &ExternalDomain,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    match package.package_kind() {
        "discovery_search" | "profile_discovery" => {
            insert_samples(tx, package, domain, accepted_ordinals).await
        }
        "content_detail" => insert_detail(tx, package, domain, accepted_ordinals).await,
        "comments" => insert_comments(tx, package, domain, false, accepted_ordinals).await,
        "replies" => insert_comments(tx, package, domain, true, accepted_ordinals).await,
        _ => Ok(()),
    }
}

/// 列表面带回来的样本。
///
/// 采样口径（关键词、排序、下拉次数、目标数、实际取得数）**不在这里写**：它属于一轮
/// 采集的事实，不属于单条记录，且要么整套要么没有（`0044` 的 CHECK 拦着半套）。采集
/// 入口接通后由那一侧补齐；在此之前这些样本没有口径，那是真话。
async fn insert_samples(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    domain: &ExternalDomain,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        let Some(payload) = record.get("payload").and_then(Value::as_object) else {
            continue;
        };
        let Some(content_id) = exact_string(payload, "noteId")
            .or_else(|| exact_string(payload, "contentId").or_else(|| exact_string(payload, "id")))
        else {
            continue;
        };
        upsert_sample(
            tx,
            package,
            domain,
            content_id,
            exact_string(payload, "title"),
            exact_string(payload, "authorId"),
            exact_string(payload, "authorName"),
            observed_cover_url(payload),
            exact_nonnegative_count(payload, &["likeCount", "likes"]),
            exact_nonnegative_count(payload, &["collectCount", "collects"]),
            exact_nonnegative_count(payload, &["commentCount", "comments"]),
        )
        .await?;
    }
    Ok(())
}

/// 详情面把标题、作者与互动数补准。
async fn insert_detail(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    domain: &ExternalDomain,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    let Some(expected) = target_string(package, "contentExternalId") else {
        return Ok(());
    };
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        if record.get("kind").and_then(Value::as_str) != Some("content_detail") {
            continue;
        }
        let Some(payload) = record.get("payload").and_then(Value::as_object) else {
            continue;
        };
        if exact_string(payload, "noteId") != Some(expected) {
            continue;
        }
        upsert_sample(
            tx,
            package,
            domain,
            expected,
            exact_string(payload, "title"),
            exact_string(payload, "authorId"),
            exact_string(payload, "authorName"),
            observed_cover_url(payload),
            exact_nonnegative_count(payload, &["likes", "likeCount", "likedCount"]),
            exact_nonnegative_count(payload, &["collects", "collectCount", "collectedCount"]),
            exact_nonnegative_count(payload, &["publicCommentCount", "comments", "commentCount"]),
        )
        .await?;
    }
    Ok(())
}

/// 评论与回复。跨行业要看的正是这些原话——「最多评论」那条排序之所以是选题富矿，
/// 靠的就是这一层。
async fn insert_comments(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    domain: &ExternalDomain,
    replies: bool,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    let Some(content_id) = target_string(package, "contentExternalId") else {
        return Ok(());
    };
    // 评论必须挂在样本上。样本尚不存在时先立一条只有身份的：材料先到、列表后到是
    // 可能的，丢掉评论比留一条没有标题的样本更糟。
    let sample_ref = upsert_sample(
        tx, package, domain, content_id, None, None, None, None, None, None, None,
    )
    .await?;
    let expected_kind = if replies { "reply" } else { "comment" };
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        if record.get("kind").and_then(Value::as_str) != Some(expected_kind) {
            continue;
        }
        let Some(payload) = record.get("payload").and_then(Value::as_object) else {
            continue;
        };
        if exact_string(payload, "noteId") != Some(content_id) {
            continue;
        }
        let Some(comment_id) = exact_string(payload, "commentId") else {
            continue;
        };
        // 回复必须真的指得出父评论。指不出来的，宁可不收：一条挂不上任何位置的回复
        // 在阅读时会被当成顶层原声，那是假事实。
        let parent = if replies {
            let Some(parent) = exact_string(payload, "parentCommentId")
                .or_else(|| exact_string(payload, "replyToCommentId"))
            else {
                continue;
            };
            Some(parent)
        } else {
            None
        };
        let body = exact_string(payload, "text");
        sqlx::query(
            "INSERT INTO cross_industry_comment \
             (comment_ref,sample_ref,domain_ref,comment_external_id,parent_comment_external_id,\
              is_reply,body_text,body_state,like_count,author_external_id,author_display_name,observed_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12::timestamptz) \
             ON CONFLICT (sample_ref, comment_external_id) DO UPDATE SET \
               body_text = COALESCE(EXCLUDED.body_text, cross_industry_comment.body_text), \
               body_state = CASE WHEN EXCLUDED.body_text IS NOT NULL THEN 'KNOWN' \
                                 ELSE cross_industry_comment.body_state END, \
               like_count = COALESCE(EXCLUDED.like_count, cross_industry_comment.like_count), \
               observed_at = EXCLUDED.observed_at",
        )
        .bind(Uuid::new_v4())
        .bind(sample_ref)
        .bind(domain.domain_ref)
        .bind(comment_id)
        .bind(parent)
        .bind(replies)
        .bind(body)
        .bind(known_state(body))
        .bind(exact_nonnegative_count(payload, &["likeCount", "likes"]))
        .bind(exact_string(payload, "authorId"))
        .bind(exact_string(payload, "authorName"))
        .bind(package.observed_at())
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    }
    Ok(())
}

/// 立一条样本，或把这次看到的补进已有那条。
///
/// 缺失一律不覆盖已知值（`COALESCE`）：同一篇作品会被列表面与详情面先后看到，后一次
/// 没看到标题不等于它没有标题。这张表存的是「最近一次看到的样子」，不是时间线——
/// 互动数的时间线属于追踪功能，尚未实现。
#[allow(clippy::too_many_arguments)]
async fn upsert_sample(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    domain: &ExternalDomain,
    content_id: &str,
    title: Option<&str>,
    author_external_id: Option<&str>,
    author_name: Option<&str>,
    cover: Option<&str>,
    like_count: Option<i64>,
    collect_count: Option<i64>,
    comment_count: Option<i64>,
) -> Result<Uuid, ProducerRuntimeError> {
    sqlx::query_scalar(
        "INSERT INTO cross_industry_sample \
         (sample_ref,domain_ref,target_ref,platform,content_external_id,title,\
          author_external_id,author_name,cover_source_url,like_count,collect_count,comment_count) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) \
         ON CONFLICT (domain_ref, platform, content_external_id) DO UPDATE SET \
           title = COALESCE(EXCLUDED.title, cross_industry_sample.title), \
           author_external_id = COALESCE(EXCLUDED.author_external_id, cross_industry_sample.author_external_id), \
           author_name = COALESCE(EXCLUDED.author_name, cross_industry_sample.author_name), \
           cover_source_url = COALESCE(EXCLUDED.cover_source_url, cross_industry_sample.cover_source_url), \
           like_count = COALESCE(EXCLUDED.like_count, cross_industry_sample.like_count), \
           collect_count = COALESCE(EXCLUDED.collect_count, cross_industry_sample.collect_count), \
           comment_count = COALESCE(EXCLUDED.comment_count, cross_industry_sample.comment_count), \
           target_ref = COALESCE(cross_industry_sample.target_ref, EXCLUDED.target_ref), \
           last_observed_at = scope_001_now() \
         RETURNING sample_ref",
    )
    .bind(Uuid::new_v4())
    .bind(domain.domain_ref)
    .bind(domain.target_ref)
    .bind(package.platform())
    .bind(content_id)
    .bind(title)
    .bind(author_external_id)
    .bind(author_name)
    // 只记来源地址，不碰 cover_local_asset_path：那一列的语义是「已下载到本地」，
    // 拿远端 URL 填进去会让读取侧以为图片已经在手上。
    .bind(cover)
    .bind(like_count)
    .bind(collect_count)
    .bind(comment_count)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)
}
