-- COMMENT-RESEARCH-VOICES-READ-001
--
-- 用户原声按 current derivation 逐行读取最后一次 RunItem 状态。现有 queue / lease 索引
-- 都以 run_ref 或 state 开头，不能为该 derivation_ref LATERAL lookup 提供有界的顺序访问。
-- 本仓库的 migration runner 将每个文件置于 BEGIN/COMMIT 中，因此不能使用
-- CREATE INDEX CONCURRENTLY；此处保持一条最小、常规事务内可应用的索引。

CREATE INDEX linggan_comment_research_run_item_derivation_latest_idx
  ON linggan_comment_research_run_item(derivation_ref, updated_at DESC, run_ref DESC);

COMMENT ON INDEX linggan_comment_research_run_item_derivation_latest_idx IS
  'Comment Research Voices: most recent RunItem state for one current canonical derivation.';
