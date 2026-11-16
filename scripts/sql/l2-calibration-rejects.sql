-- L2 被拒原因分布（只读）：已发布结果与未发布结果中的逐条被拒发现。
SELECT r.source,
       item.value ->> 'rejectCode' AS reject_code,
       count(*) AS findings
FROM (
  SELECT 'published' AS source, d.rejected
  FROM linggan_content_understanding_deep d
  WHERE d.method_version = 'content-understanding.l2.model.v2'
  UNION ALL
  SELECT 'not_published' AS source, o.rejected
  FROM linggan_content_understanding_deep_outcome o
) r
CROSS JOIN LATERAL jsonb_array_elements(COALESCE(r.rejected, '[]'::jsonb)) AS item(value)
GROUP BY 1, 2
ORDER BY 1, 3 DESC, 2
