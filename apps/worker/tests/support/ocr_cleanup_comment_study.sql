-- Synthetic batch-only OCR dependency; no provider calls. All guards stay enabled.
DO $fixture$
DECLARE
 work uuid; source uuid; old_reading uuid; config uuid; model uuid; version uuid;
 policy constant uuid := 'b1680000-0000-4000-8000-000000000001';
 dirty_run constant uuid := 'b1680000-0000-4000-8000-000000000002';
 clean_run constant uuid := 'b1680000-0000-4000-8000-000000000003';
 dirty_batch constant uuid := 'b1680000-0000-4000-8000-000000000004';
 clean_batch constant uuid := 'b1680000-0000-4000-8000-000000000005';
 dirty_inv constant uuid := 'b1680000-0000-4000-8000-000000000006';
 clean_inv constant uuid := 'b1680000-0000-4000-8000-000000000007';
 dirty_start constant uuid := 'b1680000-0000-4000-8000-000000000008';
 clean_start constant uuid := 'b1680000-0000-4000-8000-000000000009';
 domain constant uuid := '00000000-0000-4000-8000-000000000001';
 method jsonb; run_id uuid; batch_id uuid; inv_id uuid; start_id uuid; dirty boolean;
BEGIN
 SELECT public_ref INTO STRICT work FROM linggan_material_content WHERE content_external_id='dossier-note';
 SELECT reading_ref INTO STRICT old_reading FROM linggan_material_reading WHERE content_public_ref=work AND producer_kind='cloud_vision' LIMIT 1;
 SELECT c.config_ref,m.model_ref,m.connection_version_ref INTO STRICT config,model,version
 FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE m.model_id='synthetic-preview' LIMIT 1;
 SELECT material_ref INTO source FROM linggan_material_comment WHERE content_public_ref=work LIMIT 1;
 -- Source identity is a JSON selection marker, no Target row/FK is required for this
 -- batch-only counterexample. A seeded real raw comment is preferable and must survive.
 source := COALESCE(source,'b1680000-0000-4000-8000-000000000010'::uuid);
 method := jsonb_build_object('contract','comment-study.method.v1','modelConfigRef',config,
  'builderRevision','synthetic-fixture.v1','stages',jsonb_build_object(
   'semantic',jsonb_build_object('systemInstruction','synthetic only','outputSchema','{}'::jsonb,'stageHash',repeat('a',64)),
   'resolution',jsonb_build_object('systemInstruction','synthetic only','outputSchema','{}'::jsonb,'stageHash',repeat('b',64)),
   'pair',jsonb_build_object('systemInstruction','synthetic only','outputSchema','{}'::jsonb,'stageHash',repeat('c',64))));
 INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,model_config_ref,contract,comment_budget,method_name,method_manifest,method_hash)
 VALUES(policy,domain,config,'comment-study.v1',1,'SYNTHETIC PURGE',method,repeat('d',64));
 FOR dirty IN SELECT unnest(ARRAY[true,false]) LOOP
  run_id := CASE WHEN dirty THEN dirty_run ELSE clean_run END;
  batch_id := CASE WHEN dirty THEN dirty_batch ELSE clean_batch END;
  inv_id := CASE WHEN dirty THEN dirty_inv ELSE clean_inv END;
  start_id := CASE WHEN dirty THEN dirty_start ELSE clean_start END;
  INSERT INTO linggan_comment_study_run(run_ref,policy_ref,as_of,state,selection_manifest,selection_hash,comment_budget,token_limit,execution_manifest,dispatch_state,dispatch_reason)
  VALUES(run_id,policy,scope_001_now(),'queued',jsonb_build_object('contract','comment-study.run-selection.v2',
    'commentBudget',1,'tokenLimit',1024,'requestedWorkRefs',jsonb_build_array(work),'coveredWorkRefs',jsonb_build_array(work),'targetSourceRefs',jsonb_build_array(source)),
    repeat('e',64),1,1024,jsonb_build_object('contract','comment-study.execution.v1','methodHash',repeat('d',64),
    'builderRevision','synthetic-fixture.v1','engineRevision',repeat('f',40),'selectionOrder','work_round_robin_oldest_first.v1','maxTargetsPerBatch',12),
    'enabled',NULL);
  -- Neither work manifest contains any OCR ID. This is the missing-UNION counterexample.
  INSERT INTO linggan_comment_study_work(run_ref,content_public_ref,domain_ref,observation_role,selection_reason,context_state,context_manifest,context_hash)
  VALUES(run_id,work,domain,'primary','user_selected','ready',jsonb_build_object('workRef',work,'sources','[]'::jsonb),repeat('1',64));
  INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,
    input_tokens,output_tokens,result,finished_at,task_key,quota_bucket,budget_domain_ref,budget_run_ref,dispatch_state)
  VALUES(inv_id,version,model,config,'analyze',repeat('2',64),'succeeded',0,0,0,0,
    jsonb_build_object('callStarted',false,'syntheticText',CASE WHEN dirty THEN 'SYNTHETIC-OLD-OCR-COPIED' ELSE 'KEEP-UNRELATED-COMMENT-RESULT' END),
    scope_001_now(),'comment_study','run',domain,run_id,'not_dispatched');
  INSERT INTO linggan_comment_study_batch(batch_ref,run_ref,content_public_ref,state,input_manifest,input_hash,output_manifest,model_invocation_ref,finished_at)
  VALUES(batch_id,run_id,work,'accepted',CASE WHEN dirty THEN jsonb_build_object('readingRef',old_reading,'text','SYNTHETIC-OLD-OCR-COPIED') ELSE jsonb_build_object('syntheticText','KEEP-UNRELATED-COMMENT-INPUT') END,
    repeat('3',64),jsonb_build_object('syntheticOutput','fixture'),inv_id,scope_001_now());
  INSERT INTO linggan_comment_study_model_request(invocation_ref,run_ref,policy_ref,stage,batch_ref,attempt_ordinal,input_context_hash,request_manifest,request_hash,deadline_at)
  VALUES(inv_id,run_id,policy,'semantic',batch_id,1,repeat('1',64),jsonb_build_object('contract','comment-study.model-request.v1'),repeat('2',64),scope_001_now()+interval '1 minute');
  INSERT INTO linggan_comment_study_start_request(request_ref,domain_ref,policy_ref,request_hash,origin,command_manifest,outcome,run_ref,result_manifest)
  VALUES(start_id,domain,policy,repeat('4',64),'manual','{}','created',run_id,'{}');
 END LOOP;
END $fixture$;
