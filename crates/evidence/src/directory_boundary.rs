//! 「这一步到底做成了没有」的判据，全系统只在这里定义一次。
//!
//! # 它此前是什么样子
//!
//! 同一个判断被抄成了六份，散在深度建档状态机、租约收尾、创作者基线、创作者生命周期、
//! 档案完整度、观察目标检查器里，而且六份彼此还不完全一样。它们回答的是同一个问题，却
//! 各写各的——「页面说可以补采详情、按钮点了没反应」正是这类分裂的典型症状。
//!
//! 更糟的是：**六份全都永远为假**。2026-09-08 拿全库实测：
//!
//! | 判据要求 | 插件实际报的 | 命中率 |
//! |---|---|---|
//! | `unknown = 0` | 主页发现恒为 4，作者资料恒为 3 | 30 次里 1 次 |
//! | 停止原因 ∈ {`surface_ended`, `maximum_quota`} | `surface_read_complete` / `profile_read_complete` | 30 次里 1 次 |
//! | 按配额停时数量正好等于配额 | 图妈采回 201 篇（配额 200） | 不成立 |
//!
//! 后果是可见的：深度建档在「采目录」这一步无限循环，详情永远采不到；巡检天天在跑，
//! 却从 2026-09-03 起一次都没被记成「成功」，界面上「上次巡查」永远停在四天前。
//!
//! # 为什么原判据错
//!
//! 它把「拿到了全部」当成了「该拿的都拿到了」，并且用**数量**去证明**完整性**。
//!
//! - `unknown` 是插件的诚实表达，不是缺陷。它的注释写着「一个可见的页面本来就不等于完整
//!   的结果集，`unknown` 就是在明说这件事」。要求它为 0，等于要求插件担保一件它担保不了
//!   的事——两边都对，但永远合不到一起。
//! - `coverage.stoppedReason` 里那个词是**写死的常量**，不携带任何信息，而且不在合同的
//!   停止原因词表里。真话在旁边：`checkpoint.surfaceReceipt.stopReason`。
//! - 数量等于上限证明不了翻到底了，数量小于上限也不证明没翻到底（这个博主可能就只有 8 篇）。
//!
//! # 真话在哪里
//!
//! 插件的滚动循环一直在区分五种收尾，只是没报进 `coverage`：
//!
//! - `bottom_confirmed`：滚到底部且连续几轮不再出现新内容——**他就这么多作品**。
//! - `target_reached`：拿满了本次下发的配额——**够了，产品规则本就是 200 篇封顶**。
//! - `max_rounds_reached` / `no_progress` / `risk_control`：**没到底也没拿满就停了**。
//!   这才是「明明有 200 篇却只给了 50 篇」，判据为假，界面显示「处理异常」交给人决定。
//!
//! 2026-09-08 实测这个信号有区分度：木可可 `bottom_confirmed` 8 篇（他真的只有 8 篇），
//! 图妈 `target_reached` 201 篇（撞上限停的），两者都成立；而中途失败的那几种一个都混不进来。

/// 一次**发现面扫描**（主页作品目录 / 关键词搜索）是否把该拿的都拿到了。
///
/// 调用方必须让这三个名字在作用域内可解析且不带歧义：
/// - `layer`：`coverage->'layers'` 展开出的那一层
/// - `task_spec`：这次任务的规格，用来读本次下发的**预期份数**
/// - `checkpoint`：采集包的执行回执，插件真正的滚动收尾在这里
///
/// 判据**不含**包类型、能力名与回执有效性：那些是各调用方自己的连接条件，形态不同。
///
/// 用宏而不是常量：Rust 的 `concat!` 只接受字面量，只有展开成字面量的宏才能嵌进调用方
/// 那些编译期拼好的 SQL 常量里。这是「只有一份定义」在这个语言里的代价，值得付。
///
/// **`target_reached` 那一支优先读 `expectedCount`，不是 `maximumQuota`。** 两者是两件事：
/// `maximumQuota` 是授权上限、也是搜索结果页的加载预算；`expectedCount` 才是这一轮该拿回
/// 多少。关键词巡查上它们不是同一个数——规则说「取赞前 20」（adhd = 20），授权给的加载预算
/// 是 200。它此前读 `maximumQuota`，于是那一轮采回 20 篇、按规则停得完全正确，判据却要求
/// `20 >= 200`，永远不成立：「巡查成功」的时间戳一次也没写上，界面上同一行同时显示「最近
/// 新增 +15」与「上次巡查 尚未巡查 / 尚未取得成功结果」。一个事实只有一个家，判据也只许
/// 读那一个家——判据读的这个数由派发侧算一次冻进任务说明书（见 `build_task_spec`）。旧任务
/// 在该字段出现前已冻结，只有在**字段缺失**时才回退到同一任务冻结的 `maximumQuota`；这不是
/// 对新巡查规则的默认值，避免已按当时配额真实完成的历史建档永久失去完成资格。
macro_rules! surface_scan_complete_sql {
    () => {
        "COALESCE((layer->>'failed')::integer,0)=0 \
         AND COALESCE((layer->>'notAttempted')::integer,0)=0 \
         AND ( \
           checkpoint #>> '{surfaceReceipt,stopReason}'='bottom_confirmed' \
           OR (checkpoint #>> '{surfaceReceipt,stopReason}'='target_reached' \
               AND COALESCE((layer->>'acquired')::integer,0) \
                   >= COALESCE((task_spec->>'expectedCount')::integer, \
                               (task_spec->>'maximumQuota')::integer,2147483647)))"
    };
}

/// 一次扫描是否**按建档标准**发出并完成，也就是「主页目录边界已建立」。
///
/// 比上面多两组条件：
///
/// 1. 必须真的拿到了东西（`observed`/`attempted`/`acquired` 都大于 0）。**这一条只属于
///    目录，不属于巡检**：巡检采回 0 条是完全正常的成功——博主这几天没发新内容；而一份
///    空目录不能算建成。2026-09-08 把这条错误地加到巡检上，立刻被
///    `patrol_success_and_latest_new_ring_share_one_qualified_target_level_round` 拦下。
/// 2. 本次下发的配额必须是建档上限 200。这不是在证明完整性，而是在证明**这次扫描是按
///    建档标准发出的**——巡检那种 20 篇的小扫描同样会 `bottom_confirmed`，但它没资格
///    当作目录边界。**这一条仍然读 `maximumQuota`**：它问的是「这一单是多少篇的档案」，
///    不是「采到多少算完成」，所以它读的是授权上限本身，不是预期份数。
macro_rules! directory_proven_sql {
    () => {
        concat!(
            crate::directory_boundary::surface_scan_complete_sql!(),
            " AND COALESCE((layer->>'observed')::integer,0)>0 \
             AND COALESCE((layer->>'attempted')::integer,0)>0 \
             AND COALESCE((layer->>'acquired')::integer,0)>0 \
             AND COALESCE((task_spec->>'maximumQuota')::integer,-1)=200"
        )
    };
}

/// 一次**作者资料读取**是否完成。
///
/// 作者资料没有滚动，也就没有 `surfaceReceipt`；它只有一条记录，读到了就是读到了。
/// 插件在这里报的完成词是 `profile_read_complete`，`unknown` 恒为 3（三项它拿不到的
/// 资料字段）——同样不该被当成缺陷。
macro_rules! profile_read_complete_sql {
    () => {
        "COALESCE((layer->>'observed')::integer,0)>0 \
         AND COALESCE((layer->>'attempted')::integer,0)>0 \
         AND COALESCE((layer->>'acquired')::integer,0)>0 \
         AND COALESCE((layer->>'failed')::integer,0)=0 \
         AND COALESCE((layer->>'notAttempted')::integer,0)=0 \
         AND layer->>'stoppedReason' \
             IN ('profile_read_complete','surface_ended','maximum_quota')"
    };
}

pub(crate) use {directory_proven_sql, profile_read_complete_sql, surface_scan_complete_sql};
