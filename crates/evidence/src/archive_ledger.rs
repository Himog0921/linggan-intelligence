//! 「这个观察目标有多少篇作品、其中多少篇已经拿到详情」——全系统只在这里定义一次。
//!
//! # 它此前是什么样子
//!
//! 同一组数字有三套取数口径，三套互不相同，而且界面上用的是同一批标签：
//!
//! | 取数处 | 目录作品 | 已有详情 |
//! |---|---|---|
//! | 观察目标列表 | 最新那一个合格目录包 + 巡检包 | **只算当前建档轮次名下的** |
//! | 目标抽屉 | 该目标所有建档工单的发现并集 | 同上范围内的详情 |
//! | 挑下一批采什么 | （不需要） | **全局：这篇到底有没有详情** |
//!
//! 2026-09-08 实测三者同时打架：南瓜哒哒 43 篇作品**全部**已有详情，列表说 1/43 并挂着
//! 「补采缺口」，抽屉说 42/42；木可可真值 10/13，列表说 0/13，抽屉说 9/12；图妈真值
//! 62/201，列表说 33/201，抽屉说 62/201。
//!
//! 根因是列表把详情限定在**当前建档轮次**名下：09-07 那次重建目录产生了新一轮，历史详情
//! 就从计数里消失了。而执行侧挑下一批时用的是全局判断，所以它没有重复采集——**数字错了，
//! 但活没白干**。这恰好说明问题只在取数，不在执行。
//!
//! # 现在的唯一定义
//!
//! - **目录作品**：来自**已证明的主页目录**（当前建档轮次里最新一个通过目录判据的发现包）
//!   加上**合格的巡检发现**，按作品去重。未证明的目录不参与——一轮还在建、或中途被风控
//!   打断的扫描不能拿出来当分母，否则「还差多少篇」会建立在一个自己都不确定的清单上。
//! - **已有详情**：目录作品中，素材库里存在详情行的那些。**不限定建档轮次**——这与执行侧
//!   挑下一批时用的 `NOT EXISTS (SELECT 1 FROM linggan_material_content_detail ...)` 是同一个
//!   判断，因此界面说「还差 N 篇」与系统实际会去采的篇数永远一致。
//!
//! - **已确认失效**：人看过平台页面后确认「这篇已经没了」的那些。作品仍留在目录里（博主
//!   当时确实发过，抹掉分母是改写历史），只是不再计入待补齐。「已有详情」压过「已失效」：
//!   作者若把作品恢复、详情随后采到了，它就该按已有详情算。
//!
//! 归属只认 `package.task_id`：包是哪张任务采回来的，连接条件已经确定了，不需要再拿
//! `coverage.target` 去逐键比对——采样口径是下发的指令而非回执的事实，插件不会把它抄回来。

/// 一个目标的作品目录，每行一篇作品，并标明它有没有详情。
///
/// 展开成一串 CTE，最后一个名为 `directory_work`，列为
/// `(target_ref, content_public_ref, has_detail)`。调用方把它接在自己的 `WITH` 里即可。
///
/// - `$scope`：目标范围条件，可引用别名 `target`，例如 `"target.target_ref=$1"`。
/// - `$as_of`：时点条件，可引用别名 `package`，例如 `"package.accepted_at<=$2::timestamptz"`；
///   不需要时传 `"true"`。
///
/// 用 `EXISTS` 判断有没有详情而不是 join 详情表：一篇作品可能有多条详情记录（重采过），
/// join 会让它在计数里出现多次；这里问的是「有没有」，不是「有几条」。
///
/// 「有没有」问的是**合格详情材料**，判据来自 `qualified_detail.rs`——与目标抽屉、关键词
/// 补齐、缺口计数消费的是同一句，不在这里另写一遍。
macro_rules! directory_works_sql {
    ($scope:literal, $as_of:literal) => {
        concat!(
            "ledger_roots AS ( \
                 SELECT target.target_ref,work_order.work_order_ref AS root_work_order_ref, \
                        row_number() OVER (PARTITION BY target.target_ref \
                                           ORDER BY work_order.created_at DESC, \
                                                    work_order.work_order_ref DESC) AS root_rank \
                 FROM collection_observation_target target \
                 JOIN collection_work_order work_order USING(target_ref) \
                 WHERE work_order.lane='deep_archive' \
                   AND work_order.stop_conditions #>> '{progressiveArchive,version}'='1' \
                   AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}' \
                       =work_order.work_order_ref::text \
                   AND ", $scope, " \
             ), ledger_active_roots AS ( \
                 SELECT target_ref,root_work_order_ref FROM ledger_roots WHERE root_rank=1 \
             ), ledger_root_orders AS ( \
                 SELECT roots.target_ref,work_order.work_order_ref \
                 FROM ledger_active_roots roots \
                 JOIN collection_work_order work_order \
                   ON work_order.target_ref=roots.target_ref \
                 WHERE work_order.lane='deep_archive' \
                   AND (work_order.work_order_ref=roots.root_work_order_ref \
                        OR work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}' \
                           =roots.root_work_order_ref::text) \
             ), ledger_proven_directory AS ( \
                 SELECT scoped.target_ref,package.package_ref, \
                        row_number() OVER (PARTITION BY scoped.target_ref \
                                           ORDER BY package.accepted_at DESC, \
                                                    package.package_ref DESC) AS package_rank \
                 FROM ledger_root_orders scoped \
                 JOIN collection_work_order_lease lease USING(work_order_ref) \
                 JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
                 JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
                 JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
                 JOIN linggan_runtime_submission_receipt receipt \
                   ON receipt.package_ref=package.package_ref \
                 CROSS JOIN LATERAL jsonb_array_elements( \
                   CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                        THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
                 WHERE package.package_kind='profile_discovery' \
                   AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
                   AND receipt.material_admission='ACCEPTED' \
                   AND layer->>'capability'='profile_discovery' \
                   AND ", crate::directory_boundary::directory_proven_sql!(), " \
                   AND ", $as_of, " \
                   AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                                   WHERE disposition.package_ref=package.package_ref \
                                     AND disposition.disposition='quarantined') \
                   -- 回执说采了几条，落库就得有几条。对不上说明这一包只落了一半，
                   -- 它的清单不能拿来当分母。
                   AND (SELECT count(*) FROM linggan_runtime_record_disposition disposition \
                        WHERE disposition.package_ref=package.package_ref \
                          AND disposition.disposition='accepted_for_library_discovery') \
                       =COALESCE((layer->>'acquired')::integer,-1) \
             ), ledger_patrol_packages AS ( \
                 SELECT DISTINCT target.target_ref,package.package_ref \
                 FROM collection_observation_target target \
                 JOIN collection_work_order work_order USING(target_ref) \
                 JOIN collection_work_order_lease lease USING(work_order_ref) \
                 JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
                 JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
                 JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
                 JOIN linggan_runtime_submission_receipt receipt \
                   ON receipt.package_ref=package.package_ref \
                 CROSS JOIN LATERAL jsonb_array_elements( \
                   CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                        THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
                 WHERE work_order.lane='patrol' \
                   AND package.package_kind='profile_discovery' \
                   AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
                   AND receipt.material_admission='ACCEPTED' \
                   AND layer->>'capability'='profile_discovery' \
                   AND ", crate::directory_boundary::surface_scan_complete_sql!(), " \
                   AND ", $as_of, " \
                   AND ", $scope, " \
                   AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                                   WHERE disposition.package_ref=package.package_ref \
                                     AND disposition.disposition='quarantined') \
                   -- 回执说采了几条，落库就得有几条。对不上说明这一包只落了一半，
                   -- 它的清单不能拿来当分母。
                   AND (SELECT count(*) FROM linggan_runtime_record_disposition disposition \
                        WHERE disposition.package_ref=package.package_ref \
                          AND disposition.disposition='accepted_for_library_discovery') \
                       =COALESCE((layer->>'acquired')::integer,-1) \
             ), ledger_directory_packages AS ( \
                 SELECT target_ref,package_ref FROM ledger_proven_directory WHERE package_rank=1 \
                 UNION \
                 SELECT target_ref,package_ref FROM ledger_patrol_packages \
             ), directory_work AS ( \
                 SELECT DISTINCT packages.target_ref,finding.content_public_ref, \
                        ", crate::qualified_detail::qualified_detail_exists_sql!(
            "finding.content_public_ref"
        ), " \
                            AS has_detail, \
                        -- 人确认过「这篇在平台上已经没了」。它仍然留在目录里——博主当时
                        -- 确实发过——只是不再计入待补齐。
                        EXISTS (SELECT 1 FROM collection_material_retirement retired \
                                WHERE retired.target_ref=packages.target_ref \
                                  AND retired.content_public_ref=finding.content_public_ref) \
                            AS is_retired \
                 FROM ledger_directory_packages packages \
                 JOIN linggan_material_discovery_finding finding \
                   ON finding.package_ref=packages.package_ref \
                 JOIN linggan_runtime_record_disposition disposition \
                   ON disposition.package_ref=finding.package_ref \
                  AND disposition.record_ordinal=finding.record_ordinal \
                 WHERE disposition.disposition<>'quarantined' \
             )"
        )
    };
}

pub(crate) use directory_works_sql;
