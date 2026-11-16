-- L2 调用的 token 实测（只读）：按 Skill 版本与研究模式汇总。
SELECT sv.directory_name AS skill,
       sv.version_no AS skill_version,
       q.research_mode,
       count(*) AS calls,
       count(*) FILTER (WHERE i.state = 'succeeded') AS succeeded,
       count(*) FILTER (WHERE i.state = 'failed') AS failed,
       round(avg(i.input_tokens)) AS avg_input_tokens,
       round(avg(i.cached_input_tokens)) AS avg_cached_input_tokens,
       round(avg(i.output_tokens)) AS avg_output_tokens,
       round(avg(i.reasoning_tokens)) AS avg_reasoning_tokens,
       max(i.input_tokens) AS max_input_tokens
FROM linggan_model_invocation i
JOIN linggan_ai_execution_attempt a USING (invocation_ref)
LEFT JOIN linggan_skill_version sv ON sv.version_ref = a.skill_version_ref
LEFT JOIN linggan_content_understanding_deep_request q ON q.item_ref = a.item_ref
WHERE i.task_key = 'content_understanding_deep'
GROUP BY 1, 2, 3
ORDER BY 1, 2, 3
