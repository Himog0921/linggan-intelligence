//! 「详情已取得」的唯一领域判据：这份内容在**当前允许用途**下，有一份合格详情材料及其来源。
//!
//! 这句话此前在七处各写一遍，其中一处写的是另一件事：观察目标的目录把「详情标题非空」当成
//! 已取得。于是一篇**详情已经落库、只是平台没给标题**的作品，在列表与抽屉里显示成还欠一篇
//! 详情，在补齐资格里又被挑中重采，而在覆盖统计里它已经算取得了——同一条材料事实，四个面
//! 读出四种答案。
//!
//! 判据由三段事实合成，缺一不可：
//!
//! - **材料本体**：`linggan_material_content_detail` 有一行（这份内容的详情确实接过来了）；
//! - **来源**：承载它的包有一张**已接纳**的回执（`linggan_runtime_submission_receipt`）；
//! - **用途**：那条记录当时的处置不是 `quarantined`（`linggan_runtime_record_disposition`）。
//!
//! 标题不在其中。`title` 是**字段覆盖度**，不是完成度：`0015` 用
//! `CHECK ((title_state='KNOWN') = (title IS NOT NULL))` 把这件事单独记着，缺标题时页面该说
//! 「标题未收录」，而不是把整篇材料说成没取到，更不回写一个编出来的标题。
//!
//! **今天这条判据与裸的 `EXISTS (… FROM linggan_material_content_detail …)` 等价**，而且这个
//! 等价是结构保证的，不是巧合：详情行只由 `material_admission::insert_typed_materials` 写，
//! 它按 `disposition <> 'quarantined'` 过滤记录序号，并和那张 `ACCEPTED` 回执落在同一个事务
//! 里；处置行是 append-only（`0004` 的触发器禁止改删），所以那一刻的事实不会被后来的写入
//! 翻转。写严的那两段守的是**下一位写库的人**：没有它们，将来任何一条绕过准入的写入都会让
//! 「已取得」凭空变真，而四个面会一起错。
//!
//! 连接走 `package_ref`（包的主键）与 `attempt_id`（包与回执在这一列上各自唯一，`0004`），
//! 不走 `receipt.package_ref`——那一列没有索引，按它关联会让每一行都扫一遍回执表。
//!
//! **不合并两个领域。** 跨行业样本的详情住在 `cross_industry_sample_detail`，判据在
//! `cross_industry_sample_facts.rs`，本模块只管证据侧。`0044` 的隔离要求两侧不共享查询、
//! 不共享接口——合成一条，隔离就变成「要记得加条件」。
//!
//! **行级孪生。** `target_catalog::read_catalog` 的 LATERAL 取的是**那一行**的字段（标题、
//! 时间），不能只问存在性，所以它的连接条件另行写在这里的同一组事实上。两处必须同进同出：
//! 改这里的判据时，那处也要看一遍（它有 T19 的用例守着）。
//!
//! **写路径的守卫有意不在这里。** 「至少得有一行材料」这类判断（例如
//! `archive_completeness::retire_materials` 决定这篇能不能被确认失效）问的是**有没有任何一行
//! 材料**，方向是把关更严。把展示判据用在那里，会让一份被隔离的材料把退役入口打开。

/// 这份内容有一份合格详情材料。
///
/// `$content_public_ref` 是 `linggan_material_content_detail.content_public_ref` 要比对的
/// SQL 表达式（`"$1"` 或外层查询里的列引用，都是字面量），比较用 `=`。
macro_rules! qualified_detail_exists_sql {
    ($content_public_ref:literal) => {
        concat!(
            "EXISTS (SELECT 1 FROM linggan_material_content_detail qualified_detail \
              JOIN linggan_runtime_capture_package qualified_package \
                ON qualified_package.package_ref=qualified_detail.package_ref \
              JOIN linggan_runtime_submission_receipt qualified_receipt \
                ON qualified_receipt.attempt_id=qualified_package.attempt_id \
              JOIN linggan_runtime_record_disposition qualified_disposition \
                ON qualified_disposition.package_ref=qualified_detail.package_ref \
               AND qualified_disposition.record_ordinal=qualified_detail.record_ordinal \
             WHERE qualified_detail.content_public_ref=",
            $content_public_ref,
            " AND qualified_receipt.material_admission='ACCEPTED' \
                AND qualified_disposition.disposition<>'quarantined')"
        )
    };
}

/// 这份内容**还欠**详情。挑选、批量与计数共用它，免得「欠着」与「有了」两句话各写一遍。
///
/// 里面那句用 `$crate::` 指名调用：`macro_rules!` 的展开在**调用点**解析名字，写短名会要求
/// 每个调用方把两条宏都 import 进来——那正是「判据分散」的另一种形态。
macro_rules! qualified_detail_missing_sql {
    ($content_public_ref:literal) => {
        concat!(
            "NOT ",
            $crate::qualified_detail::qualified_detail_exists_sql!($content_public_ref)
        )
    };
}

pub(crate) use qualified_detail_exists_sql;
pub(crate) use qualified_detail_missing_sql;
