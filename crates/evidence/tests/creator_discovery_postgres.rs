#[path = "support/material_fixture.rs"]
mod fixture;
use linggan_contracts::{
    EvidenceQuery,
    creator_discovery::{CreatorScope, creator_key},
};
use linggan_evidence::{
    creator_discovery::{aggregate, load},
    read_work_resources, register_discovered_creator, work_resource_schema_is_ready,
};
use serde_json::json;
use uuid::Uuid;
const D: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
const OTHER: Uuid = Uuid::from_u128(0x00000000000040008000000000000002);

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn batched_media_read_keeps_accepted_ocr_and_current_withdrawal() {
    let db = fixture::proof_database("creator_discovery_batch_media").await;
    let mut refs = Vec::new();
    for (index, text) in [(1, "可用的 OCR 正文"), (2, "已经撤回的 OCR 正文")] {
        let content_id = format!("batch-media-{index}");
        let slot_key = format!("xhs:{content_id}:cover:1");
        let package = fixture::submit_package(
            &db,
            "content_detail",
            json!({"contentExternalId":content_id}),
            json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":content_id},"payload":{"title":format!("作品 {index}"),"authorId":format!("author-{index}")}}),
        ).await;
        let work_ref: Uuid = sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1")
            .bind(&content_id).fetch_one(db.pool()).await.unwrap();
        refs.push(work_ref);
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
            .bind(Uuid::new_v4()).bind(work_ref).bind(D).bind(package).execute(db.pool()).await.unwrap();
        let observation_ref = Uuid::new_v4();
        fixture::submit_package(
            &db,
            "media_slots",
            json!({"contentExternalId":content_id}),
            json!({"kind":"media_slot","slotKey":slot_key,"observationRef":observation_ref,"slot":{"role":"cover","ordinal":1},"observation":{"externalUri":format!("https://media.example/{content_id}.jpg"),"candidateUris":[format!("https://media.example/{content_id}.jpg")],"observedAt":"2026-09-20T10:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":content_id}}),
        ).await;
        let blob = format!("{index:064x}");
        linggan_evidence::admit_media_blob(&db, observation_ref, &blob, "image/jpeg", 4096, &format!("blobs/batch/{index}"))
            .await.unwrap();
        let job_ref: Uuid = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE slot_key=$1 AND processor_kind='image_ocr'")
            .bind(&slot_key).fetch_one(db.pool()).await.unwrap();
        let derivative_ref = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'ocr_text',$3,24,$4)")
            .bind(derivative_ref).bind(job_ref).bind(format!("{:064x}", index + 10)).bind(format!("derivatives/batch/{index}.txt"))
            .execute(db.pool()).await.unwrap();
        let layout_ref = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'paddleocr','batch-proof',1080,1440,$5,20,$6)")
            .bind(layout_ref).bind(derivative_ref).bind(work_ref).bind(&blob).bind(format!("{:064x}", index + 20)).bind(format!("derivatives/batch/{index}.json"))
            .execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_media_ocr_layering_result(layering_ref,layout_ref,layer_version,state,decision_source,image_substantive_text,retained_line_refs,excluded_lines) VALUES($1,$2,'rules-v1','ACCEPTED','rules',$3,'[]'::jsonb,'[]'::jsonb)")
            .bind(Uuid::new_v4()).bind(layout_ref).bind(text).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason,occurred_at) VALUES($1,$2,'succeeded','batch proof',clock_timestamp())")
            .bind(Uuid::new_v4()).bind(job_ref).execute(db.pool()).await.unwrap();
        if index == 2 {
            sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,derivative_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','batch-proof','withdrawal',clock_timestamp())")
                .bind(Uuid::new_v4()).bind(derivative_ref).execute(db.pool()).await.unwrap();
        }
    }
    let scope: CreatorScope = serde_json::from_value(json!({"domain":D})).unwrap();
    let discovery = load(&db, &scope).await.unwrap();
    assert_eq!(discovery.works.len(), 2);
    let allowed = discovery.works.iter().find(|work| work.work_ref == refs[0]).unwrap();
    assert!(allowed.search_text.contains("可用的 OCR 正文"));
    assert!(allowed.fragments.iter().any(|fragment| fragment.field == "ocr" && fragment.text == "可用的 OCR 正文"));
    let withdrawn = discovery.works.iter().find(|work| work.work_ref == refs[1]).unwrap();
    assert!(!withdrawn.search_text.contains("已经撤回的 OCR 正文"));
    assert!(!withdrawn.fragments.iter().any(|fragment| fragment.field == "ocr"));
    let detail = linggan_evidence::read_work_resource(&db, refs[1]).await.unwrap().unwrap();
    assert!(detail.inspector["derivatives"].as_array().unwrap().iter().any(|derivative| derivative["dispositionState"] == "WITHDRAWN_OR_RESTRICTED"));
    // A new job after this read clock is not allowed to leak backward into the projection.
    let first_slot = "xhs:batch-media-1:cover:1";
    let first_blob: String = sqlx::query_scalar("SELECT blob_sha256 FROM linggan_media_processing_job WHERE slot_key=$1 LIMIT 1")
        .bind(first_slot).fetch_one(db.pool()).await.unwrap();
    let asr_job = Uuid::new_v4();
    let asr_derivative = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'asr','batch-asr','full')")
        .bind(asr_job).bind(&first_blob).bind(first_slot).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'asr_text',$3,24,'derivatives/batch/asr.txt')")
        .bind(asr_derivative).bind(asr_job).bind("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text) VALUES($1,$2,'asr_text',$3,$3)")
        .bind(asr_derivative).bind(refs[0]).bind("可用的 ASR 转录").execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,occurred_at) VALUES($1,$2,'succeeded',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(asr_job).execute(db.pool()).await.unwrap();
    let retired_job = Uuid::new_v4();
    let retired_derivative = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'image_ocr','batch-retired','full')")
        .bind(retired_job).bind(&first_blob).bind(first_slot).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'ocr_text',$3,24,'derivatives/batch/retired.txt')")
        .bind(retired_derivative).bind(retired_job).bind("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .execute(db.pool()).await.unwrap();
    let retired_layout = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'paddleocr','batch-proof',1080,1440,$5,20,'derivatives/batch/retired.json')")
        .bind(retired_layout).bind(retired_derivative).bind(refs[0]).bind(&first_blob).bind("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
        .execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_layering_result(layering_ref,layout_ref,layer_version,state,decision_source,image_substantive_text,retained_line_refs,excluded_lines) VALUES($1,$2,'rules-v1','ACCEPTED','rules',$3,'[]'::jsonb,'[]'::jsonb)")
        .bind(Uuid::new_v4()).bind(retired_layout).bind("不能出现的退役 OCR 文本").execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_retirement(retired_job_ref,reason) VALUES($1,'tesseract_replaced_by_paddleocr')")
        .bind(retired_job).execute(db.pool()).await.unwrap();
    let with_asr = load(&db, &scope).await.unwrap();
    let first = with_asr.works.iter().find(|w| w.work_ref == refs[0]).unwrap();
    assert!(first.search_text.contains("可用的 ASR 转录"));
    assert!(first.fragments.iter().any(|fragment| fragment.field == "transcript" && fragment.text == "可用的 ASR 转录"));
    assert!(!first.search_text.contains("不能出现的退役 OCR 文本"));
    let retired_detail = linggan_evidence::read_work_resource(&db, refs[0]).await.unwrap().unwrap();
    assert!(retired_detail.inspector["derivatives"].as_array().unwrap().iter().any(|d| d["jobRef"] == retired_job.to_string() && d["state"] == "RETIRED"));
    let future_job = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope,created_at) VALUES($1,$2,$3,'asr','future-proof','full','2099-01-01')")
        .bind(future_job).bind(&first_blob).bind(first_slot).execute(db.pool()).await.unwrap();
    let unchanged = linggan_evidence::read_work_resource(&db, refs[0]).await.unwrap().unwrap();
    assert!(!unchanged.inspector["derivatives"].as_array().unwrap().iter().any(|d| d["jobRef"] == future_job.to_string()));

    // The same slot can have multiple origin generations; that does not duplicate a job row.
    fixture::submit_package(&db, "media_slots", json!({"contentExternalId":"batch-media-1"}), json!({
        "kind":"media_slot","slotKey":first_slot,"observationRef":Uuid::new_v4(),
        "slot":{"role":"cover","ordinal":1},
        "observation":{"externalUri":"https://media.example/batch-media-1-v2.jpg","candidateUris":["https://media.example/batch-media-1-v2.jpg"],"observedAt":"2026-09-21T10:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":"batch-media-1"}
    })).await;
    let after_reobservation = linggan_evidence::read_work_resource(&db, refs[0]).await.unwrap().unwrap();
    assert_eq!(after_reobservation.inspector["derivativesReceipt"]["total"], unchanged.inspector["derivativesReceipt"]["total"]);

    // 260 nonempty jobs force the exact per-work 256 + 1 page boundary.
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) SELECT gen_random_uuid(),$1,$2,'asr','batch-scale-'||n::text,'full' FROM generate_series(1,260) AS n")
        .bind(&first_blob).bind(first_slot).execute(db.pool()).await.unwrap();
    let page = linggan_evidence::read_work_resource(&db, refs[0]).await.unwrap().unwrap();
    assert_eq!(page.inspector["derivativesReceipt"]["total"], json!(264));
    assert_eq!(page.inspector["derivativesReceipt"]["returned"], json!(256));
    assert_eq!(page.inspector["derivativesReceipt"]["truncated"], true);
    assert!(page.inspector["derivativesReceipt"]["nextCursor"].as_str().is_some_and(|cursor| cursor.starts_with("job:")));
    assert_eq!(page.inspector["derivatives"].as_array().unwrap().len(), 256);
    let scale_order: Vec<Uuid> = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE processor_version LIKE 'batch-scale-%' ORDER BY created_at,job_ref")
        .fetch_all(db.pool()).await.unwrap();
    let scale_set: std::collections::HashSet<_> = scale_order.iter().copied().collect();
    let actual_scale_order: Vec<Uuid> = page.inspector["derivatives"].as_array().unwrap().iter()
        .filter_map(|d| d["jobRef"].as_str().and_then(|value| value.parse::<Uuid>().ok()))
        .filter(|job_ref| scale_set.contains(job_ref)).collect();
    assert!(!actual_scale_order.is_empty());
    assert_eq!(actual_scale_order, scale_order[..actual_scale_order.len()], "same-timestamp jobs use stable job-ref ordering");
    assert_eq!(load(&db, &scope).await.unwrap().works.len(), 2, "pagination within one work must not remove another work");

    // Live disposition is key-wide: derivative, slot, and blob keys can each restrict a read.
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','batch-proof','slot withdrawn',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(first_slot).execute(db.pool()).await.unwrap();
    let slot_restricted = load(&db, &scope).await.unwrap();
    assert!(!slot_restricted.works.iter().find(|w| w.work_ref == refs[0]).unwrap().search_text.contains("可用的 OCR 正文"));
    let second_blob: String = sqlx::query_scalar("SELECT blob_sha256 FROM linggan_media_processing_job WHERE slot_key='xhs:batch-media-2:cover:1' LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,blob_sha256,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','batch-proof','blob withdrawn',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(second_blob).execute(db.pool()).await.unwrap();
    let blob_restricted = linggan_evidence::read_work_resource(&db, refs[1]).await.unwrap().unwrap();
    assert!(blob_restricted.inspector["derivatives"].as_array().unwrap().iter().all(|d| d["dispositionState"] == "WITHDRAWN_OR_RESTRICTED"));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn handbook_fixture_and_same_work_counterexamples() {
    let db = fixture::proof_database("creator_discovery_fixture").await;
    let fixture = [
        (1, 1600, "related", false),
        (1, 50, "related", false),
        (2, 2000, "related", true),
        (2, 30, "related", true),
        (3, 40, "related", false),
        (3, 9000, "unrelated", true),
        (4, 1500, "related", true),
        (5, 1200, "related", false),
        (6, 8000, "unknown", false),
        (7, 100, "related", true),
        (8, 900, "related", true),
        (0, 6000, "related", false),
    ];
    let mut refs = Vec::new();
    for (i, (author, likes, _, _)) in fixture.iter().enumerate() {
        let id = format!("discovery-w{}", i + 1);
        let mut payload = json!({"title":format!("W{} 实践记录",i+1),"bodyText":format!("这是作品{}的具体实践过程",i+1),"likes":likes,"publicCommentCount":0,"collects":0,"shares":0,"authorName":if *author==2||*author==8{"同名作者".to_string()}else{format!("作者{author}")}});
        if *author != 0 {
            payload["authorId"] = json!(format!("A{author}"));
        }
        let package=fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":payload})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        refs.push(work);
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,like_threshold) VALUES($1,'xhs',1000)").bind(D).execute(db.pool()).await.unwrap();
    let mut q: CreatorScope = serde_json::from_value(json!({"domain":D})).unwrap();
    let data = load(&db, &q).await.unwrap();
    assert_eq!(data.works.len(), 12);
    let before_metric_fingerprint=data.works.iter().find(|w|w.work_ref==refs[10]).unwrap().fingerprint.clone();
    fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"discovery-w11"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"discovery-w11"},"payload":{"title":"W11 实践记录","bodyText":"这是作品11的具体实践过程","authorId":"A8","authorName":"同名作者","likes":901,"publicCommentCount":0,"collects":0,"shares":0}})).await;
    let data=load(&db,&q).await.unwrap();
    let after_metric=data.works.iter().find(|w|w.work_ref==refs[10]).unwrap();
    assert_eq!(after_metric.likes,Some(901));
    assert_eq!(after_metric.fingerprint,before_metric_fingerprint,"a like refresh with unchanged text must not queue new semantic analysis");
    for (i, (_, _, relevance, personal)) in fixture.iter().enumerate() {
        let w = data.works.iter().find(|w| w.work_ref == refs[i]).unwrap();
        let ids: Vec<_> = w.fragments.iter().map(|f| &f.fragment_id).take(1).collect();
        let result = json!({"relevance":{"value":relevance,"reason":"合成依据","evidenceFragmentIds":ids},"traits":{"personal_experience":{"value":if *personal{"yes"}else{"no"},"reason":"合成依据","evidenceFragmentIds":ids}},"topicHints":[]});
        sqlx::query("INSERT INTO linggan_creator_discovery_work_analysis(domain_ref,work_public_ref,requested_fingerprint,result_fingerprint,result_json,job_state) VALUES($1,$2,$3,$3,$4,'idle')").bind(D).bind(w.work_ref).bind(&w.fingerprint).bind(result).execute(db.pool()).await.unwrap();
    }
    for author in [1, 2] {
        let key=format!("A{author}");
        let supporting=data.works.iter().find(|w|w.work_ref==refs[if author==1 {0}else{2}]).unwrap().fragments[0].fragment_id.clone();
        sqlx::query("INSERT INTO linggan_creator_discovery_author_analysis(domain_ref,platform,author_external_id,requested_fingerprint,manual_overrides,job_state) VALUES($1,'xhs',$2,'manual',$3,'idle')").bind(D).bind(key).bind(json!({"focus":{"value":{"value":"vertical_tendency"},"supportFragmentIds":[supporting],"actor":"fixture","at":"2026-10-08"}})).execute(db.pool()).await.unwrap();
    }
    for (author, domain, state) in [(1, D, "archived"), (4, OTHER, "dismissed"), (7, D, "paused")] {
        let target = Uuid::new_v4();
        sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,source,lifecycle_state) VALUES($1,'xhs','creator',$2,'manual',$3)").bind(target).bind(format!("A{author}")).bind(state).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,$3)").bind(domain).bind(target).bind(if author==7 {"reference"}else{"primary"}).execute(db.pool()).await.unwrap();
    }
    let a1_target:Uuid=sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE platform='xhs' AND target_kind='creator' AND identity_key='A1'").fetch_one(db.pool()).await.unwrap();
    let source_package:Uuid=sqlx::query_scalar("SELECT package_ref FROM linggan_material_domain_usage WHERE content_public_ref=$1 AND domain_ref=$2").bind(refs[0]).bind(D).fetch_one(db.pool()).await.unwrap();
    for role in ["primary","reference"] {
        let request=Uuid::new_v4();
        sqlx::query("INSERT INTO collection_acquisition_request(request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) VALUES($1,$2,$3,$4,'deep_archive','duplicate usage fixture','person')").bind(request).bind(a1_target).bind(D).bind(role).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,request_ref,package_ref) VALUES($1,$2,$3,$4,'admission_reuse',$5,$6)").bind(Uuid::new_v4()).bind(refs[0]).bind(D).bind(role).bind(request).bind(source_package).execute(db.pool()).await.unwrap();
    }
    let data = load(&db, &q).await.unwrap();
    let a = aggregate(&data, &q);
    assert_eq!(data.works.len(),12,"repeated primary usage cannot duplicate a canonical work");
    let mut reference_scope=q.base();
    reference_scope.usage_role="reference".into();
    let reference=aggregate(&load(&db,&reference_scope).await.unwrap(),&reference_scope);
    assert_eq!(reference["stats"]["works"],1);
    assert_eq!(reference["stats"]["authors"],1);
    assert_eq!(a["stats"]["authors"], 8);
    assert_eq!(a["stats"]["works"], 12);
    assert_eq!(a["stats"]["unknownAuthorWorks"], 1);
    assert_eq!(a["stats"]["relatedAuthors"], 7);
    assert_eq!(a["stats"]["vertical"], 2);
    assert_eq!(a["stats"]["personal"], 4);
    assert_eq!(a["stats"]["outside"], 6);
    assert_eq!(a["stats"]["outsideViral"], 3);
    assert_eq!(a["stats"]["unknownDateWorks"], 12);
    let mut other_only=q.base();
    other_only.observation=Some("other_domains_only".into());
    assert_eq!(aggregate(&data,&other_only)["total"],1);
    assert_eq!(aggregate(&data,&other_only)["items"][0]["authorExternalId"],"A4");
    let mut related_sort=q.base();
    related_sort.sort="related_works".into();
    assert_eq!(aggregate(&data,&related_sort)["items"][0]["authorExternalId"],"A1");
    let mut viral_sort=q.base();
    viral_sort.sort="viral_works".into();
    assert_eq!(aggregate(&data,&viral_sort)["items"][0]["matchedViralWorkCount"],1);
    let mut high_sort=q.base();
    high_sort.sort="high_likes".into();
    let high_rows=aggregate(&data,&high_sort);
    let a1_high=high_rows["items"].as_array().unwrap().iter().find(|row|row["authorExternalId"]=="A1").unwrap();
    assert_eq!(a1_high["representatives"][0]["workRef"],refs[0].to_string(),"high-like sort must lead with that author's highest-like matched work");
    let a3_high=high_rows["items"].as_array().unwrap().iter().find(|row|row["authorExternalId"]=="A3").unwrap();
    assert_eq!(a3_high["candidateHighLikeCount"],40);
    assert_eq!(a3_high["representatives"][0]["workRef"],refs[4].to_string(),"unrelated 9000-like work cannot represent a 40-like high-like candidate");
    let a3_viral=aggregate(&data,&viral_sort)["items"].as_array().unwrap().iter().find(|row|row["authorExternalId"]=="A3").unwrap().clone();
    assert_eq!(a3_viral["matchedViralWorkCount"],0);
    assert_eq!(a3_viral["representatives"][0]["workRef"],refs[4].to_string(),"when no viral candidate exists, relevance fallback must outrank unrelated likes");
    let mut recent_with_high=q.base();
    recent_with_high.high_likes=true;
    recent_with_high.sort="recent".into();
    assert_eq!(aggregate(&data,&recent_with_high)["items"][0]["authorExternalId"],"A8","an explicit recent sort must win over the high-like filter");
    let mut recent_with_viral=q.base();
    recent_with_viral.viral=true;
    recent_with_viral.sort="recent".into();
    assert_eq!(aggregate(&data,&recent_with_viral)["items"][0]["authorExternalId"],"A5","an explicit recent sort must win over the viral filter");
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET result_json=jsonb_set(result_json,'{topicHints}', '[{\"text\":\"具体方向\",\"evidenceFragmentIds\":[]}]'::jsonb),result_at=scope_001_now() WHERE domain_ref=$1 AND work_public_ref=$2").bind(D).bind(refs[2]).execute(db.pool()).await.unwrap();
    let topic_data=load(&db,&q).await.unwrap();
    let mut hint_scope=q.base();
    hint_scope.topic_hint=Some("具体方向".into());
    assert_eq!(aggregate(&topic_data,&hint_scope)["total"],1);
    assert_eq!(aggregate(&topic_data,&hint_scope)["items"][0]["authorExternalId"],"A2");
    assert_eq!(linggan_evidence::creator_discovery::matching_work_refs(&topic_data,&hint_scope),vec![refs[2]]);
    let hint_work=topic_data.works.iter().find(|w|w.work_ref==refs[2]).unwrap();
    assert!(hint_work.analysis["resultAtDisplay"].is_string());
    assert_eq!(hint_work.analysis["supportFragments"][0]["workRef"],refs[2].to_string());
    for (work,hints) in [(refs[0],r#"[{"text":"方向 A"},{"text":"方向 B"}]"#),(refs[1],r#"[{"text":"方向 C"}]"#)] {
        sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET result_json=jsonb_set(result_json,'{topicHints}',$3::jsonb) WHERE domain_ref=$1 AND work_public_ref=$2").bind(D).bind(work).bind(hints).execute(db.pool()).await.unwrap();
    }
    let multi_hint_data=load(&db,&q).await.unwrap();
    let a1_global=aggregate(&multi_hint_data,&q)["items"].as_array().unwrap().iter().find(|row|row["authorExternalId"]=="A1").unwrap().clone();
    assert_eq!(a1_global["topicHints"],json!(["方向 A","方向 B"]));
    let mut third_hint=q.base();
    third_hint.topic_hint=Some("方向 C".into());
    let a1_selected=aggregate(&multi_hint_data,&third_hint);
    assert_eq!(a1_selected["total"],1);
    assert_eq!(a1_selected["items"][0]["topicHints"][0],"方向 C","the exact matched phrase cannot disappear behind the global two-phrase summary");
    assert_eq!(a1_selected["items"][0]["representatives"][0]["workRef"],refs[1].to_string());
    assert_ne!(a["items"][0]["creatorKey"], a["items"][1]["creatorKey"]);
    let same_name = a["items"].as_array().unwrap().iter().filter(|item| item["displayName"] == "同名作者").count();
    assert_eq!(same_name, 2, "same display name cannot merge stable author IDs");
    let explicit: EvidenceQuery = serde_json::from_value(json!({"domainRef":D,"publicRefs":[refs[2],refs[0],refs[2]],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    let hydrated = read_work_resources(&db, &explicit).await.unwrap();
    assert_eq!(hydrated.items.len(), 2, "duplicate public refs hydrate one canonical work");
    assert!(hydrated.items.iter().any(|item| item.identity.public_ref == refs[2]));
    let unrestricted:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"publicRefs":[refs[2]],"restriction":"UNRESTRICTED","scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    assert!(read_work_resources(&db,&unrestricted).await.unwrap().items.is_empty(),"each explicit ref must pass the current restriction filter");
    let foreign: EvidenceQuery = serde_json::from_value(json!({"domainRef":OTHER,"publicRefs":[refs[2]],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    assert!(read_work_resources(&db, &foreign).await.unwrap().items.is_empty(), "explicit refs must retain the domain gate");
    for extra in [json!({"sort":"relevance"}),json!({"window":"last_30_days"}),json!({"text":"实践"})] {
        let mut value=json!({"domainRef":D,"publicRefs":[refs[2]],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"});
        value.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let unsupported:EvidenceQuery=serde_json::from_value(value).unwrap();
        assert!(read_work_resources(&db,&unsupported).await.is_err(),"batch must not silently ignore filters or sort");
    }
    let too_many:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"publicRefs":vec![refs[0];101],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    assert!(read_work_resources(&db,&too_many).await.is_err());
    q.observation = Some("outside".into());
    q.viral = true;
    q.traits = Some("personal_experience".into());
    let matched = aggregate(&data, &q);
    assert_eq!(matched["total"], 2);
    q.creator_key = Some(creator_key("xhs", "A2"));
    let query:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"creatorScope":q,"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    let back = read_work_resources(&db, &query).await.unwrap();
    assert_eq!(back.items.len(), 1);
    assert_eq!(back.items[0].identity.public_ref, refs[2]);
    let mut author_search=q.base();
    author_search.query=Some("同名作者".into());
    let search_query:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"creatorScope":author_search,"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    let search_back=read_work_resources(&db,&search_query).await.unwrap();
    assert_eq!(search_back.items.len(),3,"author search backlink must include only the two stable authors sharing that name");
    assert!(search_back.items.iter().all(|item|[refs[2],refs[3],refs[10]].contains(&item.identity.public_ref)));
    let key = creator_key("xhs", "A2");
    let (one, two) = tokio::join!(
        register_discovered_creator(&db, D, &key,"primary"),
        register_discovered_creator(&db, D, &key,"primary")
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one["targetRef"], two["targetRef"]);
    assert_eq!(one["collectionRequested"], false);
    let paused=register_discovered_creator(&db,D,&creator_key("xhs","A7"),"primary").await.unwrap();
    assert_eq!(paused["lifecycleState"],"paused");
    assert_eq!(paused["created"],false);
    assert_eq!(paused["relationshipAdded"],false);
    let paused_role:String=sqlx::query_scalar("SELECT role FROM observation_domain_target WHERE domain_ref=$1 AND target_ref=$2").bind(D).bind(paused["targetRef"].as_str().unwrap().parse::<Uuid>().unwrap()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(paused_role,"reference","idempotent registration must preserve an existing role");
    q = q.base();
    let after = aggregate(&load(&db, &q).await.unwrap(), &q);
    assert_eq!(after["stats"]["outside"], 5);
    assert_eq!(after["stats"]["outsideViral"], 2);
    assert_eq!(after["stats"]["authors"], 8);
    sqlx::query("UPDATE observation_domain SET status='paused' WHERE domain_ref=$1").bind(OTHER).execute(db.pool()).await.unwrap();
    let a4_before=load(&db,&q).await.unwrap();
    let a4=a4_before.works.iter().find(|w|w.author_external_id.as_deref()==Some("A4")).unwrap();
    assert_eq!(a4.observation["hasOtherPrimaryDomain"],true);
    assert_eq!(a4.observation["lifecycleState"],"dismissed");
    let a4_key=creator_key("xhs","A4");
    let conflict=register_discovered_creator(&db,D,&a4_key,"primary").await.unwrap_err();
    assert_eq!(conflict.to_string(),"active_primary_domain_conflict","a paused other domain still owns the primary relation");
    let reference_receipt=register_discovered_creator(&db,D,&a4_key,"reference").await.unwrap();
    assert_eq!(reference_receipt["usageRole"],"reference");
    assert_eq!(reference_receipt["lifecycleState"],"dismissed");
    assert_eq!(reference_receipt["monitoringEnabled"],false);
    let mut dismissed_scope=q.base();
    dismissed_scope.observation=Some("dismissed".into());
    let dismissed_data=load(&db,&dismissed_scope).await.unwrap();
    assert_eq!(aggregate(&dismissed_data,&dismissed_scope)["total"],1);
    assert_eq!(aggregate(&dismissed_data,&dismissed_scope)["items"][0]["authorExternalId"],"A4");
    let repeated=register_discovered_creator(&db,D,&a4_key,"primary").await.unwrap();
    assert_eq!(repeated["usageRole"],"reference","replay cannot silently promote an existing relation");
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(jobs, 0);
    sqlx::query(
        "UPDATE linggan_creator_discovery_policy SET like_threshold=NULL WHERE domain_ref=$1",
    )
    .bind(D)
    .execute(db.pool())
    .await
    .unwrap();
    let no_rule = load(&db, &q).await.unwrap();
    assert!(aggregate(&no_rule, &q)["stats"]["outsideViral"].is_null());
    let mut unavailable_viral_sort=q.base();
    unavailable_viral_sort.sort="viral_works".into();
    assert_eq!(load(&db,&unavailable_viral_sort).await.unwrap_err().to_string(),"viral_threshold_unset");
    q.high_likes = true;
    q.observation = Some("outside".into());
    q.sort = "high_likes".into();
    let high = aggregate(&no_rule, &q);
    assert_eq!(high["items"][0]["authorExternalId"], "A6");
    assert_eq!(high["items"][0]["candidateHighLikeCount"], 8000);
    // Exact deep links must never silently turn an invalid key into an all-library response.
    q.creator_key = Some("xhs:bad!".into());
    assert!(load(&db, &q).await.is_err());
    sqlx::query("ALTER TABLE collection_observation_target RENAME TO collection_observation_target_unavailable")
        .execute(db.pool()).await.unwrap();
    let partial_scope=q.base();
    let partial=load(&db,&partial_scope).await.unwrap();
    let partial_rows=aggregate(&partial,&partial_scope);
    assert_eq!(partial_rows["observationLookupState"],"unavailable");
    assert!(partial_rows["stats"]["outside"].is_null());
    assert!(partial_rows["stats"]["outsideViral"].is_null());
    assert!(partial_rows["items"][0]["observation"]["inCurrentDomain"].is_null());
    let mut unavailable_filter=partial_scope.clone();
    unavailable_filter.observation=Some("outside".into());
    assert!(load(&db,&unavailable_filter).await.is_err(),"unavailable observation filter cannot produce a false empty result");
    for state in ["other_domains_only","dismissed"] {
        unavailable_filter.observation=Some(state.into());
        assert!(load(&db,&unavailable_filter).await.is_err(),"unknown target relationships cannot be treated as a known state");
    }
    sqlx::query("ALTER TABLE collection_observation_target_unavailable RENAME TO collection_observation_target")
        .execute(db.pool()).await.unwrap();
    sqlx::query("ALTER TABLE collection_monitor_rule_revision RENAME TO collection_monitor_rule_revision_unavailable")
        .execute(db.pool()).await.unwrap();
    let target_count_before:i64=sqlx::query_scalar("SELECT count(*) FROM collection_observation_target").fetch_one(db.pool()).await.unwrap();
    assert!(register_discovered_creator(&db,D,&creator_key("xhs","A5"),"primary").await.is_err());
    let target_count_after:i64=sqlx::query_scalar("SELECT count(*) FROM collection_observation_target").fetch_one(db.pool()).await.unwrap();
    assert_eq!(target_count_before,target_count_after,"unknown observation state must block registration");
    sqlx::query("ALTER TABLE collection_monitor_rule_revision_unavailable RENAME TO collection_monitor_rule_revision")
        .execute(db.pool()).await.unwrap();
    for (id,author,likes,date,epoch) in [("discovery-w1","A1",1600,"2026-01-10T00:00:00Z",1768003200000_i64),("discovery-w2","A1",50,"2026-08-25T00:00:00Z",1787616000000_i64)] {
        fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":format!("{id} 时间反例"),"bodyText":"亲历时间反例","authorId":author,"likes":likes,"publishedAt":epoch,"publishedAtText":date,"publishedAtSourceField":"publishTime","publishedAtSourceKind":"platform_epoch"}})).await;
    }
    sqlx::query("UPDATE linggan_creator_discovery_policy SET like_threshold=1000 WHERE domain_ref=$1").bind(D).execute(db.pool()).await.unwrap();
    let mut recent=q.base();
    recent.published_from=Some("2026-08-24".into());
    recent.published_to=Some("2026-08-27".into());
    recent.viral=true;
    recent.creator_key=Some(creator_key("xhs","A1"));
    let recent_data=load(&db,&recent).await.unwrap();
    assert_eq!(aggregate(&recent_data,&recent)["total"],0,"recent low-like work cannot borrow an older viral work");
    let changed=load(&db,&q.base()).await.unwrap();
    let a1=changed.works.iter().find(|w|w.author_external_id.as_deref()==Some("A1")).unwrap();
    assert_eq!(a1.author_analysis["manual"]["focus"]["value"]["value"],"unknown","changed source text invalidates the old positive support without altering the stored row");
    assert_eq!(a1.author_analysis["manual"]["focus"]["supportUnavailable"],true);
    let stored:serde_json::Value=sqlx::query_scalar("SELECT manual_overrides FROM linggan_creator_discovery_author_analysis WHERE domain_ref=$1 AND author_external_id='A1'").bind(D).fetch_one(db.pool()).await.unwrap();
    assert_eq!(stored["focus"]["value"]["value"],"vertical_tendency","the manual record remains available for review");
    println!(
        "CREATOR-DISCOVERY fixture: 8 authors / 12 works / 1 unknown identity; 2 same-work personal viral; concurrent registration unique; no WorkOrder; null policy verified"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn profile_only_author_uses_the_shared_as_of_owner() {
    let db=fixture::proof_database("creator_discovery_profile_owner").await;
    let id="creator-profile-only-work";
    let package=fixture::submit_package(&db,"profile_discovery",json!({"authorExternalId":"ProfileAuthor"}),json!({"kind":"profile_discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"主页发现的作品"}})).await;
    let work:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1").bind(id).fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    let q:CreatorScope=serde_json::from_value(json!({"domain":D})).unwrap();
    let data=load(&db,&q).await.unwrap();
    let current=data.works.iter().find(|w|w.work_ref==work).unwrap();
    assert_eq!(current.author_external_id.as_deref(),Some("ProfileAuthor"));
    assert_eq!(current.acquisition_kind,"limited_domain_sample", "a profile package without a domain WorkOrder cannot claim broad acquisition");
    let view:(String,String)=sqlx::query_as("SELECT author_external_id,attribution_source FROM linggan_material_content_author WHERE content_public_ref=$1").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(view,("ProfileAuthor".into(),"profile_discovery".into()));
    let direct:(String,String)=sqlx::query_as("SELECT author_external_id,attribution_source FROM linggan_material_content_author_at($1,scope_001_now())").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(direct,view);
    let too_early:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_author_at($1,'2026-01-01'::timestamptz)").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(too_early,0,"the shared owner must not expose future accepted packages in an older read");
    assert!(work_resource_schema_is_ready(&db).await.unwrap());
    sqlx::query("DELETE FROM linggan_local_schema_migration WHERE migration_id='0115_creator_discovery'").execute(db.pool()).await.unwrap();
    assert!(!work_resource_schema_is_ready(&db).await.unwrap(),"shared Work Resource must report unready without its author owner migration");
    sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES('0115_creator_discovery','b935e46868c90819235981749a680ff1a5ae9eb0d5ba69cb9d51e5521c4aed7c')").execute(db.pool()).await.unwrap();
    let detail=fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"详情作者","authorId":"DetailAuthor","authorName":"详情作者"}})).await;
    assert_ne!(detail,package);
    let updated=load(&db,&q).await.unwrap();
    assert_eq!(updated.works.iter().find(|w|w.work_ref==work).unwrap().author_external_id.as_deref(),Some("DetailAuthor"));
    let view:(String,String)=sqlx::query_as("SELECT author_external_id,attribution_source FROM linggan_material_content_author WHERE content_public_ref=$1").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(view,("DetailAuthor".into(),"content_detail".into()));
}
