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

use crate::cross_industry_observation::{
    ObservationReading, record_creator_sample_observation, record_sample_observation,
};
use crate::material_admission::{
    exact_nonnegative_count, exact_scalar_text, exact_string, known_state, observed_cover_url,
    target_string,
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
/// **采样口径来自工单冻结的那张任务单，不来自插件的回执**：插件的 `coverage.target`
/// 只回显身份（搜的是哪个词），从不回显下发给它的排序、下拉次数与取样上限——它也没有
/// 义务回显。此前这里只看回执，于是五列口径一直全空，同一个词按「最多点赞」和按
/// 「综合」采回来的笔记混在一张表里分不出来源，一个词的面貌也就拼不起来。
///
/// 口径要么整套要么没有（`0041` 的 CHECK 拦着半套）。博主来源没有排序口径可言，
/// 于是整套留空——那是真话，不是缺失。
async fn insert_samples(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    domain: &ExternalDomain,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    let sampling = sampling_provenance(tx, package).await?;
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        let Some(payload) = record.get("payload").and_then(Value::as_object) else {
            continue;
        };
        let Some(content_id) = record
            .get("sourceObject")
            .and_then(Value::as_object)
            .and_then(|source| exact_string(source, "externalId"))
        else {
            continue;
        };
        let sample_ref = upsert_sample(
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
            signed_source_url(payload),
            sampling.as_ref(),
            discovery_order(payload),
        )
        .await?;
        record_creator_sample_observation(
            tx,
            package,
            sample_ref,
            domain.domain_ref,
            i32::try_from(ordinal).expect("record count is bounded"),
            discovery_order(payload),
        )
        .await?;
    }
    Ok(())
}

/// 一轮关键词采集的口径。只有关键词来源才有；博主来源返回 `None`。
pub(crate) struct SamplingProvenance {
    pub keyword: String,
    pub sort_order: String,
    scroll_rounds: Option<i32>,
    requested_count: Option<i32>,
    actual_count: Option<i32>,
}

/// 从这批材料所属的任务单上取回口径。
///
/// 包此刻已经写进 `linggan_runtime_capture_package`（同一事务），所以能由 `package_ref`
/// 找回它的任务单。**任务单是发租那一刻冻结的**，中途有人改了监控规则也不会改写这一轮
/// 已经发生的事实。
///
/// `keyword` 与 `sort_order` 缺一不可（CHECK 如此要求，语义上也如此：不知道按什么排序
/// 取回来的一批笔记，说不清代表什么）。任何一项缺失就整套留空，不用默认值补。
async fn sampling_provenance(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
) -> Result<Option<SamplingProvenance>, ProducerRuntimeError> {
    let task_spec: Option<Value> = sqlx::query_scalar(
        "SELECT task.task_spec FROM linggan_runtime_capture_package pkg \
         JOIN linggan_runtime_task task ON task.task_id = pkg.task_id \
         WHERE pkg.package_ref = $1",
    )
    .bind(package.package_ref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let Some(target) = task_spec
        .as_ref()
        .and_then(|spec| spec.get("target"))
        .and_then(Value::as_object)
    else {
        return Ok(None);
    };
    let text = |key: &str| {
        target
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let count = |key: &str| {
        target
            .get(key)
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0)
            .and_then(|value| i32::try_from(value).ok())
    };
    let (Some(keyword), Some(sort_order)) = (text("query"), text("ranking")) else {
        return Ok(None);
    };
    Ok(Some(SamplingProvenance {
        keyword,
        sort_order,
        scroll_rounds: count("scrollRounds"),
        // 「要了多少」就是这一单**该拿回多少**：派发那一刻算一次、冻在说明书里的 `expectedCount`。
        // 不再从口径或配额现推——规则「取赞前 N」与这一单篇数上限谁小，派发侧已经答过；这里再
        // 推一遍就是同一件事的第二个家，两处一旦不一致，复核的基数从起点就是错的。
        // 说明书里没有这个数（本次上线前派出的任务）就留空：宁可没有基数，不可记一个错的。
        requested_count: task_spec
            .as_ref()
            .and_then(|spec| spec.get("expectedCount"))
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0)
            .and_then(|value| i32::try_from(value).ok()),
        // 「实际拿到多少」用插件如实报告的取得数，而不是本次写库条数：采不满是常态，
        // 复核时基数错了比没有基数更糟。
        actual_count: crate::material_contract_validation::unique_coverage_layer(
            package.coverage(),
            package.package_kind(),
        )
        .and_then(|layer| layer.get("acquired"))
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok()),
    }))
}

/// 插件在这一轮结果流里发现这篇时的位次。
///
/// 原样取插件报告的 `_discoveryOrder`（从 0 起计），不加一改写成「第几名」。按点赞取
/// 前 N 时插件另给一个 `__topRank`，那是它自己排出来的名次，由点赞数本身就能说明，
/// 不再存一份。
fn discovery_order(payload: &serde_json::Map<String, Value>) -> Option<i32> {
    payload
        .get("_discoveryOrder")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .and_then(|value| i32::try_from(value).ok())
}

/// 平台返回的、带短期签名的作品链接。
///
/// 小红书的作品必须用来源页给出的 `xsec_token` 才能打开，裸 `/explore/{id}` 会被拒绝。
/// 所以这里**只保存平台实际返回的那一条**，绝不由作品 ID 拼一个——拼出来的链接是一个
/// 从未被平台返回过的事实，既打不开，也会让后续详情采集以为自己有合法入口。
fn signed_source_url(payload: &serde_json::Map<String, Value>) -> Option<&str> {
    exact_string(payload, "url").filter(|url| url.starts_with("https://"))
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
        let sample_ref = upsert_sample(
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
            // 详情面按已知作品去采，没有「按什么排序搜到的」这回事；口径留给列表面写。
            // 详情页返回的链接不带来源页签名，不覆盖列表面记下的那条。
            None,
            None,
            None,
        )
        .await?;
        record_sample_detail(
            tx,
            package,
            domain,
            sample_ref,
            ordinal,
            exact_string(payload, "bodyText"),
            exact_scalar_text(payload, "publishedAtText"),
        )
        .await?;
    }
    Ok(())
}

/// 详情落库这件事本身。
///
/// 上面那次 upsert 只是把标题、作者、封面、互动数刷新到样本行上——**看不出详情到过手**。
/// 此前读取侧只能从运行任务反推（任务声明里要的能力是 `content_detail`、`task_spec` 里的
/// 外部 ID 对得上、任务状态是 completed），而「任务跑完」与「材料进来」是两件事：整包被
/// 隔离时一个字段都没写进样本行，租约任务照样是 completed。`0079` 把这条事实真的记下来。
///
/// **正文只在这里有。** 样本表没有正文列，此前详情采回来的正文被直接丢掉——「补详情」
/// 除了刷新互动数之外什么都没留下，而正文正是详情这一轮存在的理由。
async fn record_sample_detail(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    domain: &ExternalDomain,
    sample_ref: Uuid,
    ordinal: usize,
    body_text: Option<&str>,
    published_at_source_text: Option<String>,
) -> Result<(), ProducerRuntimeError> {
    let published = published_at_source_text.as_deref();
    sqlx::query(
        "INSERT INTO cross_industry_sample_detail              (detail_ref,sample_ref,domain_ref,package_ref,record_ordinal,               body_text,body_state,published_at_source_text,published_at_source_text_state,               observed_at)          VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)          ON CONFLICT (package_ref, record_ordinal) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(sample_ref)
    .bind(domain.domain_ref)
    .bind(package.package_ref())
    .bind(i32::try_from(ordinal).expect("package record count is bounded"))
    .bind(body_text)
    .bind(known_state(body_text))
    .bind(published)
    .bind(known_state(published))
    .bind(package.observed_at())
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
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
        tx, package, domain, content_id, None, None, None, None, None, None, None, None, None, None,
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
    source_url: Option<&str>,
    sampling: Option<&SamplingProvenance>,
    discovery_order: Option<i32>,
) -> Result<Uuid, ProducerRuntimeError> {
    let sample_ref: Uuid = sqlx::query_scalar(
        // 口径描述的是**最近这一轮是怎么看到它的**：同一篇笔记这周由「最多点赞」带回、
        // 下周由「综合」带回，该记的就是最近那一次。所以带口径的这一轮整套覆盖。
        //
        // **五列必须整套一起换或一起保**，判据是这一轮有没有口径（`EXCLUDED.keyword`）。
        // 逐列 COALESCE 会把这一轮的关键词配上上一轮的下拉次数，拼出一份从未发生过的
        // 口径；而详情面与评论面本来就没有排序口径，它们更新同一行时必须原样保留列表面
        // 记下的那一套，不能把它抹成空。
        //
        // 标题、作者、封面、互动数是作品自身的事实，保旧补新——这一轮没读到不等于它没有。
        // 签名链接同理：旧的那条即便已过期，也好过没有。
        "INSERT INTO cross_industry_sample \
         (sample_ref,domain_ref,target_ref,platform,content_external_id,title,\
          author_external_id,author_name,cover_source_url,like_count,collect_count,comment_count,\
          source_url,keyword,sort_order,scroll_rounds,requested_count,actual_count) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18) \
         ON CONFLICT (domain_ref, platform, content_external_id) DO UPDATE SET \
           title = COALESCE(EXCLUDED.title, cross_industry_sample.title), \
           author_external_id = COALESCE(EXCLUDED.author_external_id, cross_industry_sample.author_external_id), \
           author_name = COALESCE(EXCLUDED.author_name, cross_industry_sample.author_name), \
           cover_source_url = COALESCE(EXCLUDED.cover_source_url, cross_industry_sample.cover_source_url), \
           like_count = COALESCE(EXCLUDED.like_count, cross_industry_sample.like_count), \
           collect_count = COALESCE(EXCLUDED.collect_count, cross_industry_sample.collect_count), \
           comment_count = COALESCE(EXCLUDED.comment_count, cross_industry_sample.comment_count), \
           source_url = COALESCE(EXCLUDED.source_url, cross_industry_sample.source_url), \
           keyword = CASE WHEN EXCLUDED.keyword IS NULL THEN cross_industry_sample.keyword ELSE EXCLUDED.keyword END, \
           sort_order = CASE WHEN EXCLUDED.keyword IS NULL THEN cross_industry_sample.sort_order ELSE EXCLUDED.sort_order END, \
           scroll_rounds = CASE WHEN EXCLUDED.keyword IS NULL THEN cross_industry_sample.scroll_rounds ELSE EXCLUDED.scroll_rounds END, \
           requested_count = CASE WHEN EXCLUDED.keyword IS NULL THEN cross_industry_sample.requested_count ELSE EXCLUDED.requested_count END, \
           actual_count = CASE WHEN EXCLUDED.keyword IS NULL THEN cross_industry_sample.actual_count ELSE EXCLUDED.actual_count END, \
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
    .bind(source_url)
    .bind(sampling.map(|sampling| sampling.keyword.as_str()))
    .bind(sampling.map(|sampling| sampling.sort_order.as_str()))
    .bind(sampling.and_then(|sampling| sampling.scroll_rounds))
    .bind(sampling.and_then(|sampling| sampling.requested_count))
    .bind(sampling.and_then(|sampling| sampling.actual_count))
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    // 观察记录用的是**这一轮读到的数**，与样本行上那份保旧补新的当前事实分开：
    // 这一轮没读到收藏数，就该是「这次没读到」，不能借用上一轮的值凑一份完整读数。
    record_sample_observation(
        tx,
        package,
        sample_ref,
        domain.domain_ref,
        sampling,
        &ObservationReading {
            like_count,
            comment_count,
            collect_count,
            discovery_order,
        },
    )
    .await?;
    Ok(sample_ref)
}
