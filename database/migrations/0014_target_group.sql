-- COLLECTION-001 · 观察目标的分组
--
-- 取自内容工作台监控来源的「分组」：来源多起来之后，人需要按自己的分类去看，而不是
-- 在一张八十行的表里滚动找。那边的批量管理对话框写着「可以批量加入／暂停监控，也可以
-- 批量改分组、打标签、写备注」——分组是其中最先需要的一个。
--
-- 只做分组，不做标签与备注：标签是多对多、备注是长文本，各自需要自己的表；一个字段
-- 装三样东西，最后三样都用不好。

ALTER TABLE collection_observation_target
    -- 人给的分组名。为空表示未分组——那是一个正常状态，不是缺失。
    ADD COLUMN group_name text CHECK (group_name IS NULL OR length(btrim(group_name)) > 0);

CREATE INDEX collection_observation_target_group_idx
    ON collection_observation_target (group_name) WHERE group_name IS NOT NULL;
