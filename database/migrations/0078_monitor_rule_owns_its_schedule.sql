-- 规则的东西全部归规则。目标行上不再留一份副本。
--
-- `0076` 把巡检规则拆成了两级，但把四样属于规则的东西留在了目标行上：当前版本指针、
-- 排期状态、巡检周期、以及**按目标唯一的版本号**。同一件事有两个存放处，只能靠同步维持
-- 一致——而同步必然会漏。它已经漏了两次，都是必现：
--
-- 1. 一个目标配到第三条规则时必然失败。版本号按目标全局递增，而「当前版本」读的是首要
--    规则那一条：加完第二条之后全局走到 2、指针仍停在 1，第三条必然被判 `stale_revision`，
--    改任何一条都会撞 `UNIQUE (target_ref, revision)`——不是被拒绝，是数据库直接报错。
-- 2. 调度推进排期只写规则行，而列表读目标行。下一次巡检真的排出去之后两处分叉，列表上
--    「下次巡查」会永远停在一个越来越旧的过去时间，博主也一样。
--
-- 这一支不是去同步那两处，是把目标行上那一份删掉。
--
-- **`is_primary` 一并取消。** 它只为迁移期保住下面那两条 CHECK 而存在，不是领域概念——
-- 一个关键词的三条规则之间没有主次。取消之后「加规则顶掉首要位」「停用首要要交接」这两类
-- 问题连同它们的代码一起消失。
--
-- **那两条 CHECK 也一并删除，这不是放松约束。** 它们守的是「监控中的目标必须有规则，否则
-- 调度读不到东西可跑」。调度现在遍历的是规则：一个没有规则的目标自然产不出任何到期项，
-- 它们守的失效模式已经不存在了。`monitoring_enabled` 留着——「这个目标在不在被观察」是
-- 真实的目标级事实，调度同时要求它为真且有一条到期规则。

-- 版本号改成按规则计。同一条规则改过几次是它自己的历史，与同目标的别条规则无关。
--
-- **旧约束必须先拆。** 重排会让同一个目标下的两条规则都拿到第 1 版——这正是新口径想要的
-- 结果，却当场撞上还没删的 `UNIQUE (target_ref, revision)`：只要库里有任何一个目标配了
-- 两条规则（`0076` 上线后正是这么用的），迁移会直接报错回滚。顺序写反在空库上永远看不出来。
ALTER TABLE collection_monitor_rule_revision
    DROP CONSTRAINT collection_monitor_rule_revision_target_ref_revision_key;

UPDATE collection_monitor_rule_revision revision
   SET revision = renumbered.rank
  FROM (SELECT rule_revision_ref,
               row_number() OVER (PARTITION BY rule_ref ORDER BY created_at, rule_revision_ref)
                 AS rank
          FROM collection_monitor_rule_revision) renumbered
 WHERE renumbered.rule_revision_ref = revision.rule_revision_ref;

ALTER TABLE collection_monitor_rule_revision
    ADD CONSTRAINT collection_monitor_rule_revision_rule_revision_key
        UNIQUE (rule_ref, revision);

-- 目标行上那份副本。
ALTER TABLE collection_observation_target
    DROP CONSTRAINT collection_observation_target_active_rule_fk,
    DROP CONSTRAINT collection_observation_target_monitoring_lifecycle_requires_rul,
    DROP CONSTRAINT collection_observation_target_monitoring_requires_rule,
    DROP CONSTRAINT collection_observation_target_monitor_missed_run_count_check,
    DROP CONSTRAINT collection_observation_targe_monitor_schedule_slot_second_check,
    DROP COLUMN active_monitor_rule_revision_ref,
    DROP COLUMN monitor_next_run_at,
    DROP COLUMN monitor_missed_run_count,
    DROP COLUMN last_patrol_dispatched_at,
    DROP COLUMN patrol_interval_seconds,
    -- 这两列只被写、从来没有被读去算过东西：错峰偏移是由一个纯函数当场算出来的。
    -- 而错峰本来就该**按规则**算——一个关键词的三条规则若共用目标级偏移，会一起开跑。
    DROP COLUMN monitor_schedule_anchor_at,
    DROP COLUMN monitor_schedule_slot_seconds;

DROP INDEX collection_monitor_rule_primary_idx;
ALTER TABLE collection_monitor_rule DROP COLUMN is_primary;

COMMENT ON COLUMN collection_monitor_rule_revision.revision IS
  '这条规则改过第几次。按规则计，不按目标——同目标的另一条规则有它自己的一串版本号';

-- 「上次巡查成功」两级都留着，但它们答的不是同一个问题，所以不是副本：
-- 目标那一列包含手动的一次性观察（那种工单不冻规则版本，没有规则可记）；
-- 规则那一列只记这条口径自己的成功。`0076` 建出规则那一列之后一直没有人写，
-- 界面上是一个停在回填那一刻、看不出错的旧时间；本支同时补上写入。
COMMENT ON COLUMN collection_observation_target.last_patrol_succeeded_at IS
  '这个目标最近一次巡查成功，含人工发起的一次性观察';
COMMENT ON COLUMN collection_monitor_rule.last_patrol_succeeded_at IS
  '这条口径最近一次巡查成功。只有冻了本规则版本的工单才算，手动观察不计入';
