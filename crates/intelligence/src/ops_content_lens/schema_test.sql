-- Synthetic foundation receipts only. No model is invoked for the schema proof.
SET LOCAL session_replication_role = replica;
INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref)
SELECT md5('batch'||n)::uuid,task_key,current_config_ref,'interactive',repeat('a',64),1,gen_random_uuid()
FROM generate_series(1,2) n, linggan_ai_task_setting WHERE task_key='ops_title_label';
INSERT INTO linggan_ai_batch_control(batch_ref,state,finished_at)
SELECT md5('batch'||n)::uuid,'completed',scope_001_now() FROM generate_series(1,2) n;
INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,
    reserved_tokens,charged_tokens,task_key,task_config_ref,budget_day,usage_state,dispatch_state,finished_at)
SELECT md5('invocation')::uuid,gen_random_uuid(),gen_random_uuid(),'analyze',repeat('a',64),'succeeded',
    1,1,task_key,current_config_ref,CURRENT_DATE,'unknown_reserved','started',scope_001_now()
FROM linggan_ai_task_setting WHERE task_key='ops_title_label';
SET LOCAL session_replication_role = origin;

INSERT INTO linggan_ops_lens_label_run(run_ref,run_kind,taxonomy_ref,metric_ref,primary_run_ref,
    task_config_ref,prompt_version_ref,input_hash,run_key,foundation_batch_ref,frozen_input,title_count,sample_seed,blind)
SELECT md5('run'||n)::uuid,CASE n WHEN 1 THEN 'primary' ELSE 'audit' END,
    taxonomy.definition_ref,metric.definition_ref,CASE WHEN n=2 THEN md5('run1')::uuid END,
    cfg.task_config_ref,cfg.prompt_version_ref,repeat('b',64),'example-run-'||n,md5('batch'||n)::uuid,
    jsonb_build_object('shards',jsonb_build_array(jsonb_build_array(linggan_ops_lens_title_key('样例 A!')))),
    CASE WHEN n=1 THEN 2 ELSE 1 END,CASE WHEN n=2 THEN 43 END,true
FROM generate_series(1,2) n, linggan_ai_task_setting ts
JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=ts.current_config_ref
CROSS JOIN linggan_ops_lens_definition_current taxonomy
CROSS JOIN linggan_ops_lens_definition_current metric
WHERE ts.task_key='ops_title_label' AND taxonomy.kind='taxonomy' AND metric.kind='metric' ORDER BY n;

INSERT INTO linggan_ops_lens_label(run_ref,title_key,situation,moment,offer,audience,confidence,invocation_ref)
SELECT md5('run'||n)::uuid,linggan_ops_lens_title_key('样例 A!'),'other','other','other','other','medium',md5('invocation')::uuid
FROM generate_series(1,2) n;

DO $$ BEGIN
    IF EXISTS(SELECT FROM linggan_ops_lens_effective_label) THEN
        RAISE EXCEPTION 'unaudited labels visible';
    END IF;
END $$;
INSERT INTO linggan_ops_lens_agreement(primary_run_ref,audit_run_ref,dimension,sample_count,excluded_example_count,agreement,kappa,reason)
SELECT md5('run1')::uuid,md5('run2')::uuid,dimension,1,0,1,NULL,'kappa_undefined'
FROM unnest(ARRAY['situation','moment','offer','audience']) dimension;
INSERT INTO linggan_ops_member(member_ref,email_normalized,display_name,role,state)
VALUES(md5('member')::uuid,'tester@example.test','Test','admin','invited');
INSERT INTO linggan_ops_lens_adjudication(adjudication_ref,title_key,taxonomy_ref,dimension,value,member_ref)
SELECT gen_random_uuid(),linggan_ops_lens_title_key('样例A'),definition_ref,'moment','unclear',md5('member')::uuid
FROM linggan_ops_lens_definition_current WHERE kind='taxonomy';

DO $$ BEGIN
    IF (SELECT count(*) FROM linggan_ops_lens_effective_label) <> 4
       OR (SELECT count(*) FROM linggan_ops_lens_effective_label WHERE dimension='moment' AND value='unclear'
            AND adjudication_ref IS NOT NULL AND run_ref=md5('run1')::uuid) <> 1
       OR (SELECT count(*) FROM linggan_ops_lens_effective_label WHERE value='other' AND adjudication_ref IS NULL) <> 3 THEN
        RAISE EXCEPTION 'effective dimension or provenance mismatch';
    END IF;
END $$;
