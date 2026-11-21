-- The two publications from the L0 shared example. No external task is dispatched.
INSERT INTO observation_domain(domain_ref,name) VALUES(md5('lens-domain')::uuid,'Lens domain');
INSERT INTO linggan_ops_member(member_ref,email_normalized,display_name,role,state)
VALUES(md5('snapshot-member')::uuid,'lens@example.test','Synthetic operator','admin','invited');

INSERT INTO linggan_ops_publication(publication_ref,source_system,source_record_id,platform,publication_url,canonical_url,
    content_id_state,platform_content_id,published_date_state,entry_source,review_status,native_record_state,native_version,
    native_publisher_member_ref,native_registered_by_member_ref,native_business_domain_ref,native_published_on,native_published_precision)
SELECT md5('pub'||n)::uuid,'linggan_native',(md5('pub'||n)::uuid)::text,'xhs','https://example.test/'||n,'https://example.test/'||n,
    'parsed_publication_url',lpad(n::text,24,'0'),'as_recorded','linggan_ops_manual','confirmed','active',1,
    md5('snapshot-member')::uuid,md5('snapshot-member')::uuid,md5('lens-domain')::uuid,date '2026-11-01'+(n-1),'day'
FROM generate_series(1,2) n;

INSERT INTO linggan_ops_native_write_request(request_ref,command_kind,actor_member_ref,payload_sha256,outcome,record_ref,resulting_version,outcome_code)
SELECT md5('title'||n)::uuid,'publication_create',md5('snapshot-member')::uuid,decode(repeat('00',32),'hex'),'accepted',md5('pub'||n)::uuid,1,'saved'
FROM generate_series(1,2) n;

INSERT INTO linggan_ops_publication_field_adoption(adoption_ref,publication_ref,publication_version,field_name,disposition,source_kind,
    source_request_ref,value_text,observed_at)
SELECT gen_random_uuid(),md5('pub'||n)::uuid,1,'title','adopted','native_registration',md5('title'||n)::uuid,
    CASE n WHEN 1 THEN '样例 A!' ELSE '样例A' END,scope_001_now()::text
FROM generate_series(1,2) n;
