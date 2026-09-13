//! 跨行业样本的两条共用判据：**这个目标看到过这篇**、**这篇的详情已取得**。
//!
//! 这两句话此前在三处各写一遍（检查器的命中作品、列表的命中/详情计数、补详情的待办
//! 判据），而且三处写的都不是同一件事：
//!
//! - 「这个目标的样本」写成 `sample.target_ref=$1`。那一列只在**第一次插入**时写定
//!   （`target_ref = COALESCE(cross_industry_sample.target_ref, EXCLUDED.target_ref)`），
//!   同一篇被另一个关键词先看到，这个目标就永远数不到它、也永远不会给它补详情。
//!   真正回答这个问题的是观察记录：一篇一榜一行，每一行都能顺着包追到工单和目标。
//! - 「详情已取得」从运行任务反推。`0079` 之后它是一条真实的材料事实。
//!
//! 三处共用一份定义，不是为了少打字：**判据分散在三处，就会像现在这样分别错在三个
//! 不同的地方，而每一处看起来都自洽。**
//!
//! 这不是把跨行业与证据侧的查询合成一条。`0044` 的隔离红线仍然成立——两侧不共享查询、
//! 不共享接口，这里定义的只是跨行业**自己那一侧**的判据。

/// 这个目标从某个词的某个榜上看到过这篇样本。
///
/// `$target` 是目标的 SQL 表达式（`"$1"`、`"ANY($1)"` 那样的字面量），比较用 `=`，
/// 所以传 `ANY($1)` 也成立。外层查询里样本表必须别名为 `sample`。
///
/// 只有**带口径的那一轮**会写观察记录（详情面、评论面不写，见 `0073`），所以这条判据
/// 说的正好是「从榜上看到过」，不会把「按已知作品去取详情」也算成一次命中。
macro_rules! sample_observed_by_target_sql {
    ($target:literal) => {
        concat!(
            "EXISTS (SELECT 1 FROM cross_industry_sample_observation seen \
              JOIN linggan_runtime_capture_package seen_package \
                ON seen_package.package_ref=seen.package_ref \
              JOIN collection_work_order_lease_task seen_lease_task \
                ON seen_lease_task.task_id=seen_package.task_id \
              JOIN collection_work_order_lease seen_lease USING(lease_ref) \
              JOIN collection_work_order seen_order USING(work_order_ref) \
             WHERE seen.sample_ref=sample.sample_ref \
               AND seen_order.target_ref=",
            $target,
            ")"
        )
    };
}

/// 这篇样本的详情真的取到过。
///
/// 只看 `0079` 那张表有没有行。**不看运行任务的状态**：整包被隔离时一个字段都没写进样本
/// 行，而租约任务照样是 `completed`；`blocked`／`unavailable` 更是「试过没成功」，与取到了
/// 相反。此前这两种情况都被算成已取得，于是一篇什么都没有的笔记会显示「详情已取得」，
/// 失败过一次的笔记则永久从待补清单里消失。
macro_rules! sample_detail_obtained_sql {
    () => {
        "EXISTS (SELECT 1 FROM cross_industry_sample_detail obtained \
          WHERE obtained.sample_ref=sample.sample_ref)"
    };
}

/// 上面两条判据要读的三张表都在。
///
/// 分开列会漂：判据用到三张表，而调用点此前只探一张（`cross_industry_sample`）。
/// PostgreSQL **在解析阶段**就因表不存在报错，写在 `WHERE` 里的存在性判断根本来不及生效，
/// 所以这道探测必须是独立一句、而且列全。只装了控制面那一段 schema 的证明库正是这种情况。
macro_rules! sample_facts_schema_ready_sql {
    () => {
        "to_regclass('cross_industry_sample') IS NOT NULL \
           AND to_regclass('cross_industry_sample_observation') IS NOT NULL \
           AND to_regclass('cross_industry_sample_detail') IS NOT NULL"
    };
}

pub(crate) use sample_facts_schema_ready_sql;

pub(crate) use sample_detail_obtained_sql;
pub(crate) use sample_observed_by_target_sql;
