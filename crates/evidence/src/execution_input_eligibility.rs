//! 执行资格：「这一篇现在能不能被执行」这件事的唯一判据。
//!
//! 采集链上有一件事被两个语义共用着一段代码：**页面读了一次没读成**，与**这一篇根本
//! 没有可用的执行地址**。它们此前都被记成「可恢复的派发失败」——释放整张租约、按 60/120/
//! 240/480/900 秒阶梯把整张工单放回队列。于是缺地址的那一篇每一轮都重新消耗一次租约，
//! 同批里地址完好的作品永远轮不到；每重排一次还多一条失败事件，看上去像「一直在重试」，
//! 其实从来没开始过。共享库 2026-09-21 的 3 张工单、35 条 `execution_locator_unavailable`
//! 就是这条路径连转三小时的产物。
//!
//! 分开它们的判据不是「谁记的失败」，而是**输入还在不在**：
//!
//! * 缺 locator / 必要输入 → 停止这一篇，不自动重试。新的有效输入或被审计的输入修正才让它
//!   重新可执行。它不消耗页面失败预算——因为根本没有打开过页面。
//! * 页面读取临时失败 → 有界退避，且预算按**需求范围跨工单**累计。
//!
//! 这个模块给出的判据是**一处定义、多处使用**的：
//!
//! 1. `keyword_archive_detail.rs` 的候选与批量待办——不该执行的，一开始就不排队；
//! 2. `work_order_lease.rs` 的工单展开——已经停过的成员，不再随新租约复活；
//! 3. `dispatch.rs` 的解析与停止——地址在排队之后失效的，停它自己，不动同批其余成员；
//! 4. `acquisition_chain.rs` 的准入冻结——新工单的输入在建立那一刻被记下来。
//!
//! 同一件事在四处各写一遍 SQL，日后必然各自漂移，于是又长出一个「候选说能跑、派发说没
//! 地址」的新版本——那正是这次要修的缺陷的形状。所以地址长什么样、怎么算同一个输入、
//! 什么算「还是当初那一个」，都只写在这里。
//!
//! **它不复制任何事实。** 台账回答「还能不能执行、为什么不能」；Package、Receipt、Attempt
//! 和材料事实仍只由各自的表表达。这里既不写 Attempt，也不写 Evidence。

use serde_json::Value;
use uuid::Uuid;

/// 解析规则的版本。规则变了就是另一套输入语义，旧指纹不能与新指纹直接比较。
pub(crate) const EXECUTION_INPUT_RESOLVER_VERSION: &str = "signed_locator_v1";

/// 停止策略的版本。与解析版本分开：换一套停止判据不等于换一套地址解析。
pub(crate) const EXECUTION_INPUT_POLICY_VERSION: &str = "missing_input_stop_v1";

/// 缺输入时记在台账上的原因码。
pub(crate) const MISSING_EXECUTION_INPUT_REASON: &str = "execution_input_missing";

/// 需要一条带签名的执行地址才能开始的能力。
///
/// 小红书把「打开哪一篇」编码在地址的 `xsec_token` 里，没有它就没有入口——这不是某个能力
/// 的偏好，是这四个能力共用同一次页面打开。定义放在这里而不是派发模块里：准入冻结与候选
/// 筛选问的是同一个问题。
pub(crate) fn requires_signed_execution_source(task_spec: &Value) -> bool {
    task_spec.get("platform").and_then(Value::as_str) == Some("xhs")
        && task_spec
            .get("capabilitiesRequested")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .is_some_and(|capability| {
                matches!(
                    capability,
                    "content_detail" | "media_slots" | "comments" | "replies"
                )
            })
}

/// 这一条任务说的是哪一篇作品。用于把任务与它的作用域行对上。
pub(crate) fn task_content_external_id(task_spec: &Value) -> Option<&str> {
    task_spec
        .get("target")
        .and_then(|target| target.get("contentExternalId"))
        .and_then(Value::as_str)
}

/// 「这条地址能不能当执行入口」这一条判据本身。
///
/// 平台不同规则不同：目前只有小红书有签名定位（`xsec_token`）。判据写在函数里而不是散落
/// 在各条 SQL 里，是为了让「候选筛选说它有地址」与「派发解析说它不是地址」这两句话永远
/// 指同一件事。
pub(crate) fn signed_locator_predicate(url_expression: &str) -> String {
    format!(
        "({url_expression} LIKE 'https://www.xiaohongshu.com/%' \
         AND position('xsec_token=' IN {url_expression}) > 0)"
    )
}

/// 一个执行输入的指纹。两侧同一算法：地址文本的 sha256。
///
/// 与 Rust 侧 `dispatch::execution_source_url_sha256` 同值（`convert_to(url,'UTF8')` 就是它的
/// 字节），所以「服务端算出来的同一个输入」在 SQL 与 Rust 里可以互相核对。
pub(crate) fn locator_fingerprint_sql(url_expression: &str) -> String {
    format!("encode(sha256(convert_to({url_expression},'UTF8')),'hex')")
}

/// 此刻这一篇内容可用于执行的地址，以及它出自哪条记录。
///
/// 形状与 `dispatch.rs::execution_source_url_for_task` 一致——按内容外部 ID 取**最新已接纳**
/// 的发现地址，证据侧优先，跨行业样本兜底。两处必须同形：不一致的话，候选认为「有地址」
/// 而派发解析不出来，就正好复现了这次要修的缺陷。
///
/// 返回三列：`url`、`source_kind`、`source_ref`。找不到可用的地址时返回零行——**零行是一条
/// 真实观察到的事实**（这一篇此刻没有入口），不是一个待补的默认值，所以调用方用
/// `IS NULL` / 零行来读它，而不是拿一个空串当默认。
///
/// 调用方把这个片段当子查询或 `LATERAL` 用，所以它自带 `SELECT … FROM … WHERE … LIMIT 1`。
/// `cross_industry_ready` 为假时只给证据侧那一段：控制面的证明库没有 `0044` 的样本表，
/// 「这个环境没有跨行业这一侧」与「这一篇没有地址」是两件事，不能混成同一个零行。
pub(crate) fn candidate_locator_sql(
    content_external_id_expression: &str,
    cross_industry_ready: bool,
) -> String {
    let evidence_signed = signed_locator_predicate("record.value->'payload'->>'url'");
    let evidence = format!(
        "SELECT record.value->'payload'->>'url' AS url, \
                'discovery_finding'::text AS source_kind, \
                finding.material_ref AS source_ref, \
                package.accepted_at AS observed_at \
         FROM linggan_material_discovery_finding finding \
         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records') \
              WITH ORDINALITY AS record(value,ordinality) \
         WHERE content.platform='xhs' \
           AND content.content_external_id={content_external_id_expression} \
           AND record.ordinality=finding.record_ordinal+1 \
           AND record.value->'sourceObject'->>'externalId'={content_external_id_expression} \
           AND {evidence_signed} \
         ORDER BY package.accepted_at DESC,finding.created_at DESC LIMIT 1"
    );
    if !cross_industry_ready {
        return evidence;
    }
    let sample_signed = signed_locator_predicate("sample.source_url");
    // 证据侧优先，跨行业样本只在证据侧解析不出来时兜底——与 `execution_source_url_for_task`
    // 的先后完全一致。写成显式的 `priority` 而不是把两段 `UNION ALL` 起来再排序：后者在
    // 「两侧都有地址」时挑到的可能是另一条，于是候选筛选与派发解析对着同一篇作品各说各话。
    format!(
        "SELECT locator.url,locator.source_kind,locator.source_ref FROM ( \
             SELECT 1 AS priority,evidence.url,evidence.source_kind,evidence.source_ref \
             FROM ({evidence}) evidence \
             UNION ALL \
             SELECT 2 AS priority,sample.source_url,'cross_industry_sample'::text, \
                    sample.sample_ref \
             FROM cross_industry_sample sample \
             WHERE sample.platform='xhs' \
               AND sample.content_external_id={content_external_id_expression} \
               AND {sample_signed} \
         ) locator \
         ORDER BY locator.priority LIMIT 1"
    )
}

/// 「这一篇现在仍然不该被执行」——此前在这里停过，而且**输入没有变**。
///
/// 前半句是状态，后半句才是关键。只按状态排除，会让「后来真的拿到了新地址」的那一篇永远
/// 回不到候选里：停止就从「等一个更好的输入」变成了「永久封禁」。反过来只看地址不看状态，
/// 同一篇会每一轮重新排队一次——那正是这次要修的东西。两句话必须一起说，而且要在同一处说，
/// 所以它们是同一个谓词。
///
/// 输入是否相同按 `input_fingerprint` 比：刷新一次签名令牌会得到另一条地址，于是指纹不同、
/// 这一篇重新可执行——这是对的，它确实拿到了另一次打开的机会。指纹为空表示「当初连一条
/// 地址都没有」，它只在新地址真的出现时才不相等。**失败预算不跟着清零**，那按另一条键
/// （不含 locator 指纹）累计。
pub(crate) fn unchanged_input_block_predicate(
    target_ref_expression: &str,
    domain_scope: &str,
    object_kind: &str,
    object_ref_expression: &str,
    content_external_id_expression: &str,
    cross_industry_ready: bool,
) -> String {
    let locator = candidate_locator_sql(content_external_id_expression, cross_industry_ready);
    let fingerprint = locator_fingerprint_sql("locator.url");
    format!(
        "EXISTS ( \
             SELECT 1 FROM collection_execution_input_eligibility eligibility \
             WHERE eligibility.target_ref={target_ref_expression} \
               AND eligibility.domain_scope='{domain_scope}' \
               AND eligibility.object_kind='{object_kind}' \
               AND eligibility.object_ref={object_ref_expression} \
               AND eligibility.capability='content_detail' \
               AND eligibility.state='input_blocked' \
               AND eligibility.resolver_version='{EXECUTION_INPUT_RESOLVER_VERSION}' \
               AND eligibility.input_fingerprint IS NOT DISTINCT FROM \
                   (SELECT {fingerprint} FROM ({locator}) locator))"
    )
}

/// 「这一篇此刻有没有可执行的地址」——准入用这条判据把不该排队的挡在门外。
///
/// 与上面那条不同：这里不看台账，只问此刻能不能解析出地址。它是**入口**判据——一个从来没被
/// 排过队的对象没有台账行，但一样可能没有地址。
pub(crate) fn has_executable_locator_predicate(
    content_external_id_expression: &str,
    cross_industry_ready: bool,
) -> String {
    let locator = candidate_locator_sql(content_external_id_expression, cross_industry_ready);
    format!("(SELECT locator.url FROM ({locator}) locator) IS NOT NULL")
}

/// 这张工单里已经因为缺输入停过、且输入未变的成员。
///
/// 展开工单时用它把停过的成员摘掉。摘掉，而不是「照常展开再停一次」：每发一次租约就给同一篇
/// 重建一组任务，那一组任务会在下一次派发里被再停一次——停止本身变成一个新的循环。真实材料
/// 仍然留在工单作用域里（那是冻结过的授权范围，不因为一次停止而改写），只是这一轮不再为它
/// 建任务。
pub(crate) async fn blocked_object_refs_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<BlockedObjects, sqlx::Error> {
    // 这一侧要用**两张**表：样本表记材料（`0044`），工单作用域表记「哪张工单冻了它」
    // （`0087`）。只检查前者会让控制面证明库（有样本表、没有作用域表）在一句不可能有结果的
    // 查询上直接报 42P01——「这个环境没有跨行业这一侧」是一次能力缺失，不是一次故障。
    let cross_industry_sample_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('cross_industry_sample') IS NOT NULL")
            .fetch_one(&mut **transaction)
            .await?;
    let cross_industry_scope_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample') IS NOT NULL \
             AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(&mut **transaction)
    .await?;
    let material_predicate = unchanged_input_block_predicate(
        "order_row.target_ref",
        "own_domain",
        "material_content",
        "scope.content_public_ref",
        "content.content_external_id",
        cross_industry_sample_ready,
    );
    let material: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT scope.content_public_ref \
         FROM collection_work_order_material_target scope \
         JOIN collection_work_order order_row USING(work_order_ref) \
         JOIN linggan_material_content content ON content.public_ref=scope.content_public_ref \
         WHERE scope.work_order_ref=$1 AND {material_predicate}",
    )))
    .bind(work_order_ref)
    .fetch_all(&mut **transaction)
    .await?;
    let cross_industry: Vec<Uuid> = if cross_industry_scope_ready {
        let sample_predicate = unchanged_input_block_predicate(
            "order_row.target_ref",
            "cross_industry",
            "cross_industry_sample",
            "scope.sample_ref",
            "sample.content_external_id",
            true,
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT scope.sample_ref \
             FROM collection_work_order_cross_industry_target scope \
             JOIN collection_work_order order_row USING(work_order_ref) \
             JOIN cross_industry_sample sample USING(sample_ref) \
             WHERE scope.work_order_ref=$1 AND {sample_predicate}",
        )))
        .bind(work_order_ref)
        .fetch_all(&mut **transaction)
        .await?
    } else {
        Vec::new()
    };
    Ok(BlockedObjects {
        material,
        cross_industry,
    })
}

/// 一个工单里被停掉的成员，两侧分开。
#[derive(Debug, Default, Clone)]
pub(crate) struct BlockedObjects {
    pub material: Vec<Uuid>,
    pub cross_industry: Vec<Uuid>,
}

/// 停掉一个成员的执行资格：台账记下原因，同一篇的其余通道一并收束。
///
/// 三件事在同一事务里做完——**记录原因、收束相关任务、更新资格台账**。分三次写会留下
/// 「台账说停了、任务还在 pending」的中间态，而那种半停状态正是下一次「为什么它又跑起来了」
/// 的来源。
///
/// 收束的是**同一篇作品的其余通道**，不是整张租约：同批别的作品没有被牵连的理由，它们的
/// 地址完好。这与 `requeue_failed_dispatch` 里既有的兄弟通道收束同形。
///
/// 返回 `content_detail` 那条台账行的 `eligibility_ref`；任务对不上任何作用域行时返回
/// `None`——那说明这张工单不是按已知作品创建的，缺地址这件事在这里无从记起，如实不记。
pub(crate) async fn stop_material_for_missing_execution_input_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
    reason_code: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let Some(subject) = load_task_execution_subject_in_transaction(transaction, task_id).await?
    else {
        return Ok(None);
    };
    let Some(target_ref) = subject.target_ref else {
        return Ok(None);
    };
    let (domain_scope, object_kind, object_ref) = match (&subject.content_public_ref, &subject.sample_ref)
    {
        (Some(content_public_ref), _) => ("own_domain", "material_content", *content_public_ref),
        (_, Some(sample_ref)) => ("cross_industry", "cross_industry_sample", *sample_ref),
        _ => return Ok(None),
    };
    let locator = candidate_locator_sql("$9", subject.cross_industry_ready);
    // 台账是 upsert 而不是 insert：同一篇在同一个 epoch 里，**同一份输入**只有一条资格
    // （身份键含指纹），同一件事重复上报不该长出第二行。`input_fingerprint` 按**此刻**重新
    // 解析，所以「输入变了没有」是这一行随时可核对的当前值，而不是一次性的历史快照。
    //
    // 冲突目标用约束名而不是列清单：这条键是 `NULLS NOT DISTINCT` 的，缺地址时的指纹是空值，
    // 而空值能不能参与推断是一件依赖 PostgreSQL 版本行为的事。按名字指定，语义就写作字面上
    // 的样子——「和这条约束冲突时改写它」，与推断规则无关。
    let rows: Vec<(Uuid, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO collection_execution_input_eligibility \
             (eligibility_ref,target_ref,domain_scope,object_kind,object_ref,capability, \
              state,reason_code,input_fingerprint,input_source_kind,input_source_ref, \
              resolver_version,input_source_status,last_event_ref,policy_version) \
         SELECT gen_random_uuid(),$1,$2,$3,$4,capability.capability, \
                'input_blocked',$5, \
                CASE WHEN locator.url IS NULL THEN NULL \
                     ELSE encode(sha256(convert_to(locator.url,'UTF8')),'hex') END, \
                locator.source_kind,locator.source_ref, \
                $6, \
                CASE WHEN $7 THEN 'frozen' ELSE 'legacy_input_unfrozen' END, \
                $8,$10 \
         FROM (VALUES ('content_detail'),('media_slots'),('comments'),('replies')) \
              AS capability(capability) \
         LEFT JOIN LATERAL ({locator}) locator ON TRUE \
         ON CONFLICT ON CONSTRAINT collection_execution_input_eligibility_input_identity \
         DO UPDATE SET state=EXCLUDED.state,reason_code=EXCLUDED.reason_code, \
             input_fingerprint=EXCLUDED.input_fingerprint, \
             input_source_kind=EXCLUDED.input_source_kind, \
             input_source_ref=EXCLUDED.input_source_ref, \
             resolver_version=EXCLUDED.resolver_version, \
             input_source_status=EXCLUDED.input_source_status, \
             last_event_ref=EXCLUDED.last_event_ref,updated_at=scope_001_now() \
         RETURNING eligibility_ref,capability",
    )))
    .bind(target_ref)
    .bind(domain_scope)
    .bind(object_kind)
    .bind(object_ref)
    .bind(reason_code)
    .bind(EXECUTION_INPUT_RESOLVER_VERSION)
    .bind(subject.scope_was_frozen)
    .bind(Uuid::new_v4())
    .bind(&subject.content_external_id)
    .bind(EXECUTION_INPUT_POLICY_VERSION)
    .fetch_all(&mut **transaction)
    .await?;
    let eligibility_ref = rows
        .iter()
        .find(|(_, capability)| capability == "content_detail")
        .map(|(eligibility_ref, _)| *eligibility_ref);
    // 收束同一篇的其余通道：一条详情、一组媒体、一批评论与回复共用同一次页面打开，没有地址
    // 就是四者都没有入口。留下 pending 的那三条会在后面的派发里各自再触发一次同样的停止——
    // 同一件事在同一个批里被记四遍。
    //
    // **只收 `pending`。** `in_progress` 的意思是这一次已经交给浏览器了：可能已经开过页面、
    // 甚至已经把包做完只等交付。把它按「从未开始」停下，既与「已产生会话或 Attempt 的对象先
    // 对账、保护交付」相反，也过不去表上的约束（`input_blocked` 要求 `claimed_at IS NULL`）——
    // 那会让整次停止在写库时失败，停下来变成一件做不到的事。已在执行的那条通道按它自己的
    // 结果收尾，它的交付不受这次停止影响。
    sqlx::query(
        "UPDATE collection_work_order_lease_task sibling \
         SET execution_state='input_blocked',claimed_at=NULL,claimed_by_installation_ref=NULL \
         FROM linggan_runtime_task sibling_runtime \
         WHERE sibling.lease_ref=$1 \
           AND sibling.task_id=sibling_runtime.task_id \
           AND sibling.execution_state='pending' \
           AND sibling_runtime.task_spec #>> '{target,contentExternalId}'=$2",
    )
    .bind(subject.lease_ref)
    .bind(&subject.content_external_id)
    .execute(&mut **transaction)
    .await?;
    Ok(eligibility_ref)
}

/// 把一条已经停下的资格交还给「可以再试」，用于输入真的变了之后的后继工作。
///
/// 旧行不删、不改：它是「当时确实没有可用地址」这条历史事实。新的那一条通过
/// `successor_eligibility_ref` 指回旧行——于是「这一次为什么又能跑了」有据可查，而「那时它
/// 停过」也没有被抹掉。失败次数随行继承，不因为换了一条地址就归零。
///
/// **`retry_epoch` 不增加。** 换了一条地址不是故障修复证明，也不是受控重新准入；让它开一个
/// 新 epoch，等于让地址变化把跨工单的失败预算清零——而那正是合同里点名不许的
/// （「不能靠刷新签名 token 清零页面失败次数」）。所以这里开的是**同一个 epoch 里的当前资格
/// 行**：一个需求范围在同一 epoch 内至多一条「现在能执行」的行（表上的部分唯一索引
/// `collection_execution_input_eligibility_current_idx`），地址变化是**更新那一条的当前输入**，
/// 不是再长出一条。
///
/// 冲突目标因此落在**当前行**这条部分唯一索引上，而不是整张表的身份键上：身份键含指纹，
/// 指纹一变就是另一个键，`ON CONFLICT` 根本不会命中——于是每换一次地址就多出一条并行的可执行
/// 资格，同一篇作品被两个 scheduler 各派一张工单（合同里「不能因输入指纹不同就并行派发同一
/// 缺口」说的就是这件事）。命中当前行之后只改**输入那几列**：`state` 与
/// `deduplicated_failure_count` 原样不动——输入变化既不能把一条 `budget_exhausted` 悄悄改回
/// `eligible`，也不能把已经累计的失败次数抹掉。
///
/// **新行记的是此刻解析出来的那条输入**，不是照抄前驱那一行的。前驱是一个「当时没有地址」
/// 的停止行，它的指纹为空正是它要说的那件事；把空值抄到后继行上，等于新开的这一条又在说
/// 「没有地址」，只是换了个 epoch——而它存在的全部理由恰恰是现在有了地址，且和当初那条不同。
/// 于是「下一轮输入还是不是这一条」无从判断：比较的两边永远是「有」与「没有」，
/// 同一份输入每被准入一次就再开一条后继，停止记录会被这种噪音淹没。
/// 来源三列同理：它们要回答的是「这一次的地址从哪来」。
///
/// `input_source_status` 记 `frozen`：后继产生在准入事务里，与它同一笔事务刚刚写下这张
/// 工单的执行输入冻结，所以它的输入从出生起就是冻的。前驱那一行的取值说的是前驱自己的
/// 来历，不能转述给后继。
pub(crate) async fn open_successor_eligibility_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    predecessor_ref: Uuid,
    resolved: &ResolvedExecutionInput,
) -> Result<Option<Uuid>, sqlx::Error> {
    let fingerprint = locator_fingerprint_sql("$3::text");
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "INSERT INTO collection_execution_input_eligibility \
             (eligibility_ref,target_ref,domain_scope,object_kind,object_ref,capability, \
              state,reason_code,input_fingerprint,input_source_kind,input_source_ref, \
              resolver_version,input_source_status,deduplicated_failure_count,retry_epoch, \
              last_event_ref,policy_version,successor_eligibility_ref) \
         SELECT $1,predecessor.target_ref,predecessor.domain_scope,predecessor.object_kind, \
                predecessor.object_ref,predecessor.capability,'eligible',NULL, \
                {fingerprint},$4, $5, \
                '{EXECUTION_INPUT_RESOLVER_VERSION}','frozen', \
                predecessor.deduplicated_failure_count, \
                predecessor.retry_epoch,predecessor.last_event_ref, \
                predecessor.policy_version,predecessor.eligibility_ref \
         FROM collection_execution_input_eligibility predecessor \
         WHERE predecessor.eligibility_ref=$2 \
         ON CONFLICT (target_ref,domain_scope,object_kind,object_ref,capability) \
             WHERE state <> 'input_blocked' \
         DO UPDATE SET input_fingerprint=EXCLUDED.input_fingerprint, \
             input_source_kind=EXCLUDED.input_source_kind, \
             input_source_ref=EXCLUDED.input_source_ref, \
             resolver_version=EXCLUDED.resolver_version, \
             successor_eligibility_ref=EXCLUDED.successor_eligibility_ref, \
             last_event_ref=EXCLUDED.last_event_ref,updated_at=scope_001_now() \
         RETURNING eligibility_ref",
    )))
    .bind(Uuid::new_v4())
    .bind(predecessor_ref)
    .bind(&resolved.url)
    .bind(&resolved.source_kind)
    .bind(resolved.source_ref)
    .fetch_optional(&mut **transaction)
    .await
}

/// 此刻解析出来的执行输入：地址本体、它出自哪类记录、以及那条记录是谁。
///
/// 三列一起出现、一起缺失——「有地址但说不出它从哪来」与「没有地址」是两种不同的未知，
/// 这一路只处理前者。
pub(crate) struct ResolvedExecutionInput {
    pub url: String,
    pub source_kind: String,
    pub source_ref: Uuid,
}

/// 准入冻结这一次的执行输入：为「停过、而输入真的变了」的对象开启后继资格。
///
/// 工单写下的那几篇就是这一次要执行的东西，所以交还执行资格的时机**就是这里**：早于此没有
/// 新输入可冻结，晚于此这一次执行就没有身份。旧行不改——它是「当时确实没有可用地址」那条
/// 历史事实；可执行的那一条指回它，于是「这一次为什么又能跑了」有据可查。
///
/// 判据用的是与候选筛选同一个指纹：不相等（含「当初一条地址都没有、现在有了」）才算输入变了，
/// 而且此刻真的要解析得出一条地址——「输入变成了没有输入」不是一次重新可执行的机会，那一篇
/// 仍然没有入口，给它开一条 `eligible` 的资格等于凭空说它可以跑了。只改发现时间、或只是同一个
/// 地址被又发现了一次，指纹一样，不开启任何东西——否则每轮发现都会制造一次「新的执行机会」，
/// 而真实的停止记录会被这种噪音淹没。
///
/// **不靠「已经开过后继就不再开」来防重**：停过的那一行指纹是空的，「与此刻不同」对它永远成立，
/// 所以它每一轮都会被重新认成前驱，而多开一条的后果不是多一行历史、是同一篇同时有两条可执行
/// 资格。防重住在写入那一步——`ON CONFLICT` 命中当前行就地更新，重复准入写进去的是同一份输入，
/// 于是幂等，且与「这条前驱是不是已经交接过」无关。
///
/// 返回值是**被写入的资格行数**（新开的与就地更新的都算），不是「新增了几条」；调用方按
/// 「可追溯性住在台账行上，不靠一个计数转述」处理它，不计入契约。
pub(crate) async fn open_successors_for_reopened_inputs_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    material_objects: &[Uuid],
    cross_industry_objects: &[Uuid],
) -> Result<u32, sqlx::Error> {
    let cross_industry_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('cross_industry_sample') IS NOT NULL")
            .fetch_one(&mut **transaction)
            .await?;
    let mut predecessors: Vec<(Uuid, ResolvedExecutionInput)> = Vec::new();
    if !material_objects.is_empty() {
        let locator = candidate_locator_sql("content.content_external_id", cross_industry_ready);
        let rows = reopened_input_predecessors(
            transaction,
            &format!(
                "JOIN linggan_material_content content ON content.public_ref=eligibility.object_ref"
            ),
            "own_domain",
            "material_content",
            target_ref,
            material_objects,
            &locator,
        )
        .await?;
        predecessors.extend(rows);
    }
    if !cross_industry_objects.is_empty() && cross_industry_ready {
        let locator = candidate_locator_sql("sample.content_external_id", true);
        let rows = reopened_input_predecessors(
            transaction,
            "JOIN cross_industry_sample sample ON sample.sample_ref=eligibility.object_ref",
            "cross_industry",
            "cross_industry_sample",
            target_ref,
            cross_industry_objects,
            &locator,
        )
        .await?;
        predecessors.extend(rows);
    }
    let mut reconciled = 0_u32;
    for (predecessor_ref, resolved) in predecessors {
        if open_successor_eligibility_in_transaction(transaction, predecessor_ref, &resolved)
            .await?
            .is_some()
        {
            reconciled += 1;
        }
    }
    Ok(reconciled)
}

/// 「停过、而输入真的变了」的那些前驱行，连同**此刻**解析出来的输入。
///
/// 两侧（本领域材料 / 跨行业样本）只差一个 JOIN 与作用域取值，判据本身逐字相同，所以判据
/// 只写一份：两份写法任何一处漂了，就会在某一侧多开或少开一条后继，而这类差异极难在结果里
/// 看出来。`locator_url IS NOT NULL` 是「真的变了」的一部分——输入变成「没有地址」不是一次
/// 重新可执行的机会，那种情况下这一篇仍然没有入口，写出来的只会是一条谎称可执行的资格。
///
/// **每个需求范围只取一条前驱**（`DISTINCT ON`，取 epoch 最大的那条）。同一个范围可能留下
/// 多条停止行（历次受控重试各留一条），它们说的是同一件事、要写的也是同一条当前行；一条一条
/// 都写一遍，最后落在「谁接替了它」那一列上的是哪一条就由算子说了算——审计读到的来路会随机
/// 变成某一次更早的停止。
async fn reopened_input_predecessors(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope_join: &str,
    domain_scope: &str,
    object_kind: &str,
    target_ref: Uuid,
    object_refs: &[Uuid],
    locator: &str,
) -> Result<Vec<(Uuid, ResolvedExecutionInput)>, sqlx::Error> {
    let fingerprint = locator_fingerprint_sql("locator.url");
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT ON (eligibility.object_ref,eligibility.capability) \
                eligibility.eligibility_ref,locator.url,locator.source_kind,locator.source_ref \
         FROM collection_execution_input_eligibility eligibility \
         {scope_join} \
         LEFT JOIN LATERAL ({locator}) locator ON TRUE \
         WHERE eligibility.target_ref=$1 \
           AND eligibility.domain_scope='{domain_scope}' \
           AND eligibility.object_kind='{object_kind}' \
           AND eligibility.object_ref=ANY($2) \
           AND eligibility.state='input_blocked' \
           AND eligibility.resolver_version='{EXECUTION_INPUT_RESOLVER_VERSION}' \
           AND locator.url IS NOT NULL \
           AND eligibility.input_fingerprint IS DISTINCT FROM {fingerprint} \
         ORDER BY eligibility.object_ref,eligibility.capability,eligibility.retry_epoch DESC",
    )))
    .bind(target_ref)
    .bind(object_refs)
    .fetch_all(&mut **transaction)
    .await
    .map(|rows: Vec<(Uuid, String, String, Uuid)>| {
        rows.into_iter()
            .map(|(eligibility_ref, url, source_kind, source_ref)| {
                (
                    eligibility_ref,
                    ResolvedExecutionInput {
                        url,
                        source_kind,
                        source_ref,
                    },
                )
            })
            .collect()
    })
}

/// 一个租约任务说的是「哪一篇作品、属于哪个需求范围」。
#[derive(Debug)]
struct TaskExecutionSubject {
    target_ref: Option<Uuid>,
    content_public_ref: Option<Uuid>,
    sample_ref: Option<Uuid>,
    content_external_id: String,
    lease_ref: Uuid,
    cross_industry_ready: bool,
    /// 这张工单作用域内的输入此前有没有被冻结过。决定台账行如实记 `frozen` 还是
    /// `legacy_input_unfrozen`——不补造历史，也不声称「它从出生起就没有地址」。
    scope_was_frozen: bool,
}

async fn load_task_execution_subject_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
) -> Result<Option<TaskExecutionSubject>, sqlx::Error> {
    // 与 `blocked_object_refs_in_transaction` 同一个判据：这一侧要用两张表——样本表
    // （`0044`）与工单作用域表（`0087`）。只查前者，控制面证明库会在 `sample_sql` 上
    // 报 42P01，把「这个环境没有这一侧」变成一次故障。
    let cross_industry_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample') IS NOT NULL \
             AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(&mut **transaction)
    .await?;
    // 一篇作品属于哪一侧，由它落在哪张作用域表上回答，不由任务形状回答——同一次页面打开
    // 在两侧长得一样，落点不同（`0044` 的隔离）。
    // 「这张工单冻过输入没有」问的是**工单**，不是台账。反过来由台账推会得出相反的答案：
    // 一次停止也会为某个对象写下 `input_source_status='frozen'`，于是「这个对象停过」会被
    // 读成「这张工单冻过输入」。冻结发生在建单那一刻，标记就记在建单事务里（`0097`）。
    let material_sql = "SELECT order_row.target_ref,material.content_public_ref,NULL::uuid, \
                material_content.content_external_id,lease.lease_ref, \
                order_row.execution_input_frozen_at IS NOT NULL \
         FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         JOIN collection_work_order order_row USING(work_order_ref) \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         JOIN collection_work_order_material_target material \
           ON material.work_order_ref=order_row.work_order_ref \
         JOIN linggan_material_content material_content \
           ON material_content.public_ref=material.content_public_ref \
         WHERE task.task_id=$1 \
           AND material_content.content_external_id=runtime.task_spec #>> '{target,contentExternalId}'";
    let sample_sql = "SELECT order_row.target_ref,NULL::uuid,cross_scope.sample_ref, \
                sample.content_external_id,lease.lease_ref, \
                order_row.execution_input_frozen_at IS NOT NULL \
         FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         JOIN collection_work_order order_row USING(work_order_ref) \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         JOIN collection_work_order_cross_industry_target cross_scope \
           ON cross_scope.work_order_ref=order_row.work_order_ref \
         JOIN cross_industry_sample sample ON sample.sample_ref=cross_scope.sample_ref \
         WHERE task.task_id=$1 \
           AND sample.content_external_id=runtime.task_spec #>> '{target,contentExternalId}'";
    let sql = if cross_industry_ready {
        format!("{material_sql} UNION ALL {sample_sql} LIMIT 1")
    } else {
        format!("{material_sql} LIMIT 1")
    };
    let row: Option<(Option<Uuid>, Option<Uuid>, Option<Uuid>, String, Uuid, bool)> =
        sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(task_id)
            .fetch_optional(&mut **transaction)
            .await?;
    Ok(row.map(
        |(
            target_ref,
            content_public_ref,
            sample_ref,
            content_external_id,
            lease_ref,
            scope_was_frozen,
        )| TaskExecutionSubject {
            target_ref,
            content_public_ref,
            sample_ref,
            content_external_id,
            lease_ref,
            cross_industry_ready,
            scope_was_frozen,
        },
    ))
}

/// 页面失败预算：同一个需求范围**跨工单**累计几次 `page_read_failed` 之后停止自动重试。
///
/// 数值与既有规则相同（同一详情最多三次），改的是**计数范围**：从前每次建一张新工单都从零
/// 开始数，于是同一个缺口可以永远「再试三次」——共享库 2026-09-21 里同一篇作品进入过
/// 21、13、20、18 张有匹配任务的工单，每一张都从零开始。
pub(crate) const DETAIL_PAGE_READ_BUDGET: i64 = 3;

/// 预算用尽时记在台账上的原因码。
pub(crate) const PAGE_READ_BUDGET_EXHAUSTED_REASON: &str = "page_read_budget_exhausted";

/// 预算策略的版本。与停止策略分开：换一套计数范围不等于换一套停止判据。
pub(crate) const PAGE_READ_BUDGET_POLICY_VERSION: &str = "page_read_budget_v1";

/// 一次页面读取失败之后，这个需求范围的预算落点。
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageReadBudget {
    /// 台账上记到的失败次数（去重后，跨工单累计）。
    pub count: i64,
    /// 是否该停止自动重试。为真时调用方按既有终态收束该成员，不在下一张工单里复活它。
    pub exhausted: bool,
}

/// 「这个需求范围的预算已经不允许再排新工作」——候选与建单用的判据。
///
/// 两件事都算：预算已经用尽（`budget_exhausted`），以及还在冷却里（`next_retry_at` 未到）。
/// 前者是停止，后者是等一下——对「要不要现在排一张新工单」这个问题，两者的答案都是「先不排」，
/// 但它们在台账上仍是两种不同的状态，看板上分得开。
///
/// 只看**当前行**（`state <> 'input_blocked'`）：停止行是历史，一个范围可以留下多条，
/// 拿它们来挡执行等于把「当时停过」变成永久封禁。
pub(crate) fn budget_blocks_new_work_predicate(
    target_ref_expression: &str,
    domain_scope: &str,
    object_kind: &str,
    object_ref_expression: &str,
) -> String {
    format!(
        "EXISTS ( \
             SELECT 1 FROM collection_execution_input_eligibility budget \
             WHERE budget.target_ref={target_ref_expression} \
               AND budget.domain_scope='{domain_scope}' \
               AND budget.object_kind='{object_kind}' \
               AND budget.object_ref={object_ref_expression} \
               AND budget.capability='content_detail' \
               AND budget.state <> 'input_blocked' \
               AND (budget.state='budget_exhausted' \
                    OR (budget.next_retry_at IS NOT NULL \
                        AND budget.next_retry_at > scope_001_now())))"
    )
}

/// 这张工单里预算已经用尽、不该再被展开的成员。
///
/// 与「缺输入停过」分开：缺输入的成员会在输入真的变了之后回来，而预算用尽的成员**不会**
/// 自己回来——它要的是受控重新准入（另一次明确的决定），不是等一个更好的地址。所以两条判据
/// 不能合用一个函数：合起来之后，「什么时候它会回来」就没有单一答案了。
pub(crate) async fn budget_exhausted_object_refs_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<BlockedObjects, sqlx::Error> {
    let cross_industry_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample') IS NOT NULL \
             AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(&mut **transaction)
    .await?;
    let material: Vec<Uuid> = sqlx::query_scalar(
        "SELECT scope.content_public_ref \
         FROM collection_work_order_material_target scope \
         JOIN collection_work_order order_row USING(work_order_ref) \
         JOIN collection_execution_input_eligibility budget \
           ON budget.target_ref=order_row.target_ref \
          AND budget.domain_scope='own_domain' \
          AND budget.object_kind='material_content' \
          AND budget.object_ref=scope.content_public_ref \
          AND budget.capability='content_detail' \
          AND budget.state='budget_exhausted' \
         WHERE scope.work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_all(&mut **transaction)
    .await?;
    let cross_industry: Vec<Uuid> = if cross_industry_ready {
        sqlx::query_scalar(
            "SELECT scope.sample_ref \
             FROM collection_work_order_cross_industry_target scope \
             JOIN collection_work_order order_row USING(work_order_ref) \
             JOIN collection_execution_input_eligibility budget \
               ON budget.target_ref=order_row.target_ref \
              AND budget.domain_scope='cross_industry' \
              AND budget.object_kind='cross_industry_sample' \
              AND budget.object_ref=scope.sample_ref \
              AND budget.capability='content_detail' \
              AND budget.state='budget_exhausted' \
             WHERE scope.work_order_ref=$1",
        )
        .bind(work_order_ref)
        .fetch_all(&mut **transaction)
        .await?
    } else {
        Vec::new()
    };
    Ok(BlockedObjects {
        material,
        cross_industry,
    })
}

/// 记一次「页面读了一次没读成」，把预算累计到台账的当前行上。
///
/// 计数落在**当前行**而不是某张工单上，因为要回答的问题变了：从前问「这张工单里试了几次」，
/// 于是每建一张新工单都从零开始，同一个缺口可以永远「再试三次」。现在问「这个需求范围一共
/// 试了几次」——换工单不是新事实，换地址也不是（预算键不含 locator 指纹，见迁移头注释），
/// 只有受控重新准入才开新的 `retry_epoch`。
///
/// **同一个事件重报只算一次**（按 `last_event_ref` 去重）：上报路径可能重试，重试不该让预算
/// 往前走一格。
///
/// **旧归档不进预算。** 本表建立之前的工单没有冻过输入（`execution_input_frozen_at IS NULL`），
/// 它们的失败证明「试过」，不证明「试的是同一份输入」；按合同不算进预算，更不允许凭一批无法
/// 对齐输入的历史失败把一篇作品按 content ID 封禁。那些次数记进 `prior_failures_unverified`，
/// 让「过去失败过多少次、其中多少次算数」两个数都读得出来。
///
/// 返回 `None` 表示这次上报不归预算管，三种情形都如实说而不是编一个数：任务对不上任何已知的
/// 作品作用域；这个范围此刻正停在 `input_blocked` 上（停止有自己的判据，一次读失败不能改写
/// 它）；或这张工单出生时没有冻过输入（旧工单，这次失败只记进待核实）。调用方遇到 `None`
/// 就只按原有的「同一工单内」规则走。
pub(crate) async fn record_detail_page_read_failure_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
    failure_ref: Uuid,
) -> Result<Option<PageReadBudget>, sqlx::Error> {
    let Some(subject) = load_task_execution_subject_in_transaction(transaction, task_id).await?
    else {
        return Ok(None);
    };
    let Some(target_ref) = subject.target_ref else {
        return Ok(None);
    };
    let (domain_scope, object_kind, object_ref) = match (&subject.content_public_ref, &subject.sample_ref)
    {
        (Some(content_public_ref), _) => ("own_domain", "material_content", *content_public_ref),
        (_, Some(sample_ref)) => ("cross_industry", "cross_industry_sample", *sample_ref),
        _ => return Ok(None),
    };
    let mut current = current_budget_row_in_transaction(
        transaction,
        target_ref,
        domain_scope,
        object_kind,
        object_ref,
    )
    .await?;
    if current.is_none() {
        // 这个范围在台账里还没有行，这一次失败就是它的第一条记录。新建时如实写下此刻解析出来
        // 的输入，以及**这张工单有没有冻过输入**——它的来历决定这些次数算不算数。
        //
        // 冲突什么都不做：并发的另一条上报、或一条已经停下的行，都可能占住这个身份。占了就
        // 让位，随后重新读一次当前行——**不覆盖**别人写下的事实，也不把停止行改写成可执行。
        let locator = candidate_locator_sql("$9", subject.cross_industry_ready);
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "WITH resolved AS ( \
                 SELECT locator.url,locator.source_kind,locator.source_ref \
                 FROM (VALUES (1)) singleton \
                 LEFT JOIN LATERAL ({locator}) locator ON TRUE \
             ) \
             INSERT INTO collection_execution_input_eligibility \
                 (eligibility_ref,target_ref,domain_scope,object_kind,object_ref,capability, \
                  state,reason_code,input_fingerprint,input_source_kind,input_source_ref, \
                  resolver_version,input_source_status,last_event_ref,policy_version, \
                  prior_failures_unverified) \
             SELECT gen_random_uuid(),$1,$2,$3,$4,'content_detail','eligible',NULL, \
                    CASE WHEN resolved.url IS NULL THEN NULL \
                         ELSE {fingerprint} END, \
                    resolved.source_kind,resolved.source_ref,$6, \
                    CASE WHEN $7 THEN 'frozen' ELSE 'legacy_input_unfrozen' END, \
                    NULL,$8,{unverified} \
             FROM resolved \
             ON CONFLICT DO NOTHING",
            fingerprint = locator_fingerprint_sql("resolved.url"),
            unverified = unverified_prior_failures_sql(
                "$1",
                domain_scope,
                object_kind,
                "$9",
                subject.cross_industry_ready,
            ),
        )))
        .bind(target_ref)
        .bind(domain_scope)
        .bind(object_kind)
        .bind(object_ref)
        .bind(&subject.content_external_id)
        .bind(EXECUTION_INPUT_RESOLVER_VERSION)
        .bind(subject.scope_was_frozen)
        .bind(PAGE_READ_BUDGET_POLICY_VERSION)
        .bind(&subject.content_external_id)
        .execute(&mut **transaction)
        .await?;
        current = current_budget_row_in_transaction(
            transaction,
            target_ref,
            domain_scope,
            object_kind,
            object_ref,
        )
        .await?;
    }
    let Some(BudgetRow {
        eligibility_ref,
        count,
        last_event_ref,
        state,
    }) = current
    else {
        // 插不进去也读不到当前行：这个范围此刻停在「没有输入」上。停止行是历史，一次读失败
        // 不改写它，也不在它身上累计预算。
        return Ok(None);
    };
    if last_event_ref == Some(failure_ref) {
        // 同一个事件又报了一次：预算不动，把当前的落点如实答回去。
        return Ok(Some(PageReadBudget {
            count: i64::from(count),
            exhausted: state == "budget_exhausted",
        }));
    }
    if !subject.scope_was_frozen {
        // 这张工单出生时没有冻过输入（本表建立之前的工单）：它的失败证明「试过」，不证明
        // 「试的是同一份输入」，所以记进待核实那一列，不进预算——预算会触发停止，而停止
        // 之后要重新可执行需要另一次明确决定；凭一条无法对齐输入的历史失败做到这一步，
        // 等于按 content ID 把一篇作品封掉。返回 `None`：这次上报不归预算管，调用方仍按
        // 既有的「同一工单内」规则收尾。
        sqlx::query(
            "UPDATE collection_execution_input_eligibility \
             SET prior_failures_unverified=prior_failures_unverified+1,last_event_ref=$2, \
                 updated_at=scope_001_now() \
             WHERE eligibility_ref=$1",
        )
        .bind(eligibility_ref)
        .bind(failure_ref)
        .execute(&mut **transaction)
        .await?;
        return Ok(None);
    }
    let next_count = count.saturating_add(1);
    let exhausted = i64::from(next_count) >= DETAIL_PAGE_READ_BUDGET;
    let retry_after_seconds = crate::dispatch::retry_after_seconds_for_failure_count(next_count);
    sqlx::query(
        "UPDATE collection_execution_input_eligibility \
         SET deduplicated_failure_count=$2,last_event_ref=$3, \
             next_retry_at=scope_001_now()+make_interval(secs=>$4), \
             state=CASE WHEN $5 THEN 'budget_exhausted' ELSE state END, \
             reason_code=CASE WHEN $5 THEN $7 ELSE reason_code END, \
             policy_version=$6,updated_at=scope_001_now() \
         WHERE eligibility_ref=$1 AND state <> 'input_blocked'",
    )
    .bind(eligibility_ref)
    .bind(next_count)
    .bind(failure_ref)
    .bind(i32::try_from(retry_after_seconds).unwrap_or(i32::MAX))
    .bind(exhausted)
    .bind(PAGE_READ_BUDGET_POLICY_VERSION)
    .bind(PAGE_READ_BUDGET_EXHAUSTED_REASON)
    .execute(&mut **transaction)
    .await?;
    Ok(Some(PageReadBudget {
        count: i64::from(next_count),
        exhausted,
    }))
}

/// 台账里「此刻说了算」的那一行：非停止行至多一条（部分唯一索引
/// `collection_execution_input_eligibility_current_idx`）。`FOR UPDATE` 让同一范围的两条并发
/// 上报排队数数，而不是各自读到一个旧值再各写一次——那会让预算少记一格。
struct BudgetRow {
    eligibility_ref: Uuid,
    count: i32,
    last_event_ref: Option<Uuid>,
    state: String,
}

async fn current_budget_row_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    domain_scope: &str,
    object_kind: &str,
    object_ref: Uuid,
) -> Result<Option<BudgetRow>, sqlx::Error> {
    let row: Option<(Uuid, i32, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT eligibility_ref,deduplicated_failure_count,last_event_ref,state \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND domain_scope=$2 AND object_kind=$3 AND object_ref=$4 \
           AND capability='content_detail' AND state <> 'input_blocked' \
         FOR UPDATE",
    )
    .bind(target_ref)
    .bind(domain_scope)
    .bind(object_kind)
    .bind(object_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(row.map(
        |(eligibility_ref, count, last_event_ref, state)| BudgetRow {
            eligibility_ref,
            count,
            last_event_ref,
            state,
        },
    ))
}

/// 本表建立之前、没冻过输入的那些失败：记下次数，但不进预算。
///
/// 判据落在**工单**上而不是台账上：一次停止会为某个对象写下 `input_source_status='frozen'`，
/// 于是「这个对象停过」会被读成「这张工单冻过输入」——两件事方向相反。冻结发生在建单那一刻，
/// 所以只有 `collection_work_order.execution_input_frozen_at` 能回答它。
fn unverified_prior_failures_sql(
    target_ref_expression: &str,
    domain_scope: &str,
    object_kind: &str,
    content_external_id_expression: &str,
    cross_industry_ready: bool,
) -> String {
    let scope_join = match (domain_scope, object_kind) {
        ("own_domain", _) => "JOIN collection_work_order_material_target scope \
                              ON scope.work_order_ref=order_row.work_order_ref \
                             AND scope.content_public_ref=$4"
            .to_owned(),
        _ if cross_industry_ready => "JOIN collection_work_order_cross_industry_target scope \
                                       ON scope.work_order_ref=order_row.work_order_ref \
                                      AND scope.sample_ref=$4"
            .to_owned(),
        _ => "JOIN collection_work_order_cross_industry_target scope \
              ON scope.work_order_ref=order_row.work_order_ref AND false"
            .to_owned(),
    };
    format!(
        "(SELECT COUNT(*) \
          FROM collection_work_order_lease_task_dispatch_failure failure \
          JOIN collection_work_order_lease_task task ON task.task_id=failure.task_id \
          JOIN collection_work_order_lease lease ON lease.lease_ref=task.lease_ref \
          JOIN collection_work_order order_row ON order_row.work_order_ref=lease.work_order_ref \
          JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
          {scope_join} \
          WHERE order_row.target_ref={target_ref_expression} \
            AND order_row.execution_input_frozen_at IS NULL \
            AND failure.failure_code='page_read_failed' \
            AND runtime.task_spec #>> '{{capabilitiesRequested,0}}'='content_detail' \
            AND runtime.task_spec #>> '{{target,contentExternalId}}'={content_external_id_expression})",
    )
}

/// 「这个范围的详情材料已经被接纳了吗」。
///
/// 用来挡住「迟到的失败把一件已经做成的事往回写」：一次读失败的报告可能和另一个通道的成功
/// 交付并发到达，而那份材料已经落库。已经拿到的材料不因为一次旧的读失败倒退，也不该再被
/// 算进预算——那个预算问的是「要不要再试」，而这里已经没有要补的东西了。
///
/// 判据与档案完整度用的是同一句：详情材料在库里有一行（本领域侧
/// `linggan_material_content_detail`、跨行业侧 `cross_industry_sample_detail`）。**整包被隔离
/// 时两处都不会有行**，所以「任务说完成、材料没进来」不会被误读成已取得。
pub(crate) async fn detail_material_already_accepted_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let Some(subject) = load_task_execution_subject_in_transaction(transaction, task_id).await?
    else {
        return Ok(false);
    };
    if let Some(content_public_ref) = subject.content_public_ref {
        return sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM linggan_material_content_detail detail \
                            WHERE detail.content_public_ref=$1)",
        )
        .bind(content_public_ref)
        .fetch_one(&mut **transaction)
        .await;
    }
    let Some(sample_ref) = subject.sample_ref else {
        return Ok(false);
    };
    // 控制面证明库里没有跨行业那几张表：「这个环境没有这一侧」是一次能力缺失，不是「没有材料」。
    let ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample_detail') IS NOT NULL",
    )
    .fetch_one(&mut **transaction)
    .await?;
    if !ready {
        return Ok(false);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM cross_industry_sample_detail detail \
                        WHERE detail.sample_ref=$1)",
    )
    .bind(sample_ref)
    .fetch_one(&mut **transaction)
    .await
}
