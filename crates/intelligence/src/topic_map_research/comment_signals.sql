-- Frozen semantic head at the topic cutoff. Rust requalifies the exact dossier and parent.
SELECT DISTINCT ON (selected.content_public_ref,selected.comment_external_id)
 selected.content_public_ref,selected.comment_external_id,request.request_ref,result.result
FROM linggan_material_comment selected
JOIN linggan_material_comment studied ON studied.content_public_ref=selected.content_public_ref AND studied.comment_external_id=selected.comment_external_id
JOIN linggan_comment_research_request request ON request.source_ref=studied.material_ref
JOIN linggan_comment_research_result result USING(request_ref)
JOIN linggan_ai_batch_item item ON item.batch_ref=request.batch_ref
JOIN linggan_ai_batch_item_control control USING(item_ref)
WHERE request.domain_ref=$1 AND selected.material_ref=ANY($2)
 AND request.created_at<=$3::timestamptz AND result.created_at<=$3::timestamptz
 AND control.state='succeeded' AND result.result IS NOT NULL
 AND selected.body_state='KNOWN' AND studied.body_state='KNOWN' AND selected.body_text=studied.body_text
ORDER BY selected.content_public_ref,selected.comment_external_id,request.created_at DESC,request.request_ref DESC;
