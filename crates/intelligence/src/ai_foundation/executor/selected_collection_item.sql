 AND EXISTS(SELECT 1 FROM selected_collection
     WHERE b.kind='automatic' AND item.input_ref->>'collectionRunRef'=selected_collection.run_ref::text
     AND (
       (b.task_key='content_understanding' AND item.input_ref->>'kind'='content_understanding'
        AND EXISTS(SELECT 1 FROM linggan_content_understanding_request request
                   WHERE request.item_ref=item.item_ref AND request.batch_ref=b.batch_ref
                     AND request.request_ref::text=item.input_ref->>'requestRef'
                     AND request.domain_ref=selected_collection.domain_ref
                     AND request.work_ref=ANY(selected_collection.work_refs)))
       OR (b.task_key='topic_research' AND item.input_ref->>'kind'='topic_research'
           AND item.input_ref->'stageNumber'='0'::jsonb
           AND EXISTS(SELECT 1 FROM linggan_topic_research_stage stage
                      JOIN linggan_topic_research_window w USING(window_ref)
                      JOIN linggan_topic_research_run research USING(run_ref)
                      WHERE stage.item_ref=item.item_ref AND stage.stage_number=0
                        AND w.window_ref::text=item.input_ref->>'windowRef'
                        AND w.input_refs->>'executionMode'='single_pass'
                        AND w.input_refs->>'inputContract'='topic-map.core-source.v1'
                        AND w.input_refs->'commentStudy'='[]'::jsonb
                        AND research.domain_ref=selected_collection.domain_ref
                        AND w.work_ref=ANY(selected_collection.work_refs)))
     )
 )
