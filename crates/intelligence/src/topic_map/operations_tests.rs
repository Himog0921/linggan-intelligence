use super::*;
#[allow(dead_code)]
#[path = "../../../evidence/tests/support/domain_fixture.rs"]
mod domain_fixture;
#[allow(dead_code)]
#[path = "../topic_map_research/tests/saved_source_fixture.rs"]
mod support;

#[tokio::test]
#[ignore = "isolated PostgreSQL; synthetic exact publication mappings, no model"]
async fn operations_publications_drive_own_coverage() {
    let (db, _) = support::setup(&format!("tm_ops_{}", Uuid::new_v4().simple())).await;
    domain_fixture::submit_package_for_domain(
        &db,
        &support::D.to_string(),
        "synthetic-ops",
        "deep_archive",
        json!({"query":"synthetic-ops"}),
        1,
        "discovery_search",
        domain_fixture::search_coverage("synthetic-ops", 1),
        json!({"surfaceReceipt":{"stopReason":"bottom_confirmed"}}),
        vec![domain_fixture::discovery_card(
            "synthetic-ops",
            "synthetic",
            "10",
            "https://www.xiaohongshu.com/explore/synthetic-ops",
        )],
    )
    .await;
    let work = support::work(&db, "synthetic-ops", "合成运营发布内容").await;
    let member = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ops_member(member_ref,email_normalized,display_name,role,state) VALUES($1,$2,'SYNTHETIC','admin','invited')")
        .bind(member).bind(format!("{member}@example.test")).execute(db.pool()).await.unwrap();
    let mut publications = Vec::new();
    for n in 0..3 {
        let publication = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_ops_publication(publication_ref,source_system,source_record_id,platform,publication_url,canonical_url,content_id_state,platform_content_id,published_date_state,entry_source,review_status,native_record_state,native_version,native_publisher_member_ref,native_registered_by_member_ref,native_business_domain_ref,native_published_on,native_published_precision) VALUES($1,'linggan_native',$1::text,'xhs',$2,$2,'parsed_publication_url',$3,'as_recorded','linggan_ops_manual','confirmed','active',1,$4,$4,$5,'2026-11-11','day')")
            .bind(publication).bind(format!("https://www.xiaohongshu.com/explore/{n:024x}")).bind(format!("{n:024x}")).bind(member).bind(support::D).execute(db.pool()).await.unwrap();
        if n < 2 {
            // Each detail observation has an immutable receipt; distinct mappings retain both publications.
            support::work(&db, "synthetic-ops", &format!("合成运营发布内容{n}")).await;
            sqlx::query("INSERT INTO linggan_ops_publication_material_projection(projection_ref,publication_ref,publication_version,request_ref,work_order_ref,task_id,attempt_id,package_ref,receipt_ref,record_ordinal,content_public_ref,observed_at) SELECT gen_random_uuid(),$1,1,(SELECT request_ref FROM collection_acquisition_request LIMIT 1),(SELECT work_order_ref FROM collection_work_order LIMIT 1),receipt.task_id,receipt.attempt_id,detail.package_ref,receipt.receipt_ref,detail.record_ordinal,$2,detail.observed_at FROM linggan_material_content_detail detail JOIN linggan_runtime_submission_receipt receipt USING(package_ref) WHERE detail.content_public_ref=$2 AND NOT EXISTS(SELECT 1 FROM linggan_ops_publication_material_projection old WHERE old.receipt_ref=receipt.receipt_ref) ORDER BY detail.created_at DESC LIMIT 1")
                .bind(publication).bind(work).execute(db.pool()).await.unwrap();
        }
        publications.push(publication);
    }
    let report = |publication, count| {
        let db = &db;
        async move {
            let request = Uuid::new_v4();
            sqlx::query("INSERT INTO linggan_ops_native_write_request(request_ref,command_kind,actor_member_ref,payload_sha256,outcome,record_ref,outcome_code) VALUES($1,'view_report_record',$2,decode(repeat('a',64),'hex'),'accepted',$3,'synthetic')").bind(request).bind(member).bind(publication).execute(db.pool()).await.unwrap();
            sqlx::query("INSERT INTO linggan_ops_view_report(report_ref,publication_ref,report_kind,view_count,source_kind,reported_by_member_ref,request_ref,as_of) VALUES(gen_random_uuid(),$1,CASE WHEN $2::bigint IS NULL THEN 'cleared' ELSE 'value' END,$2,'linggan_ops_manual',$3,$4,scope_001_now())")
            .bind(publication).bind(count).bind(member).bind(request).execute(db.pool()).await.unwrap();
        }
    };
    report(publications[0], Some(6000_i64)).await;
    let facts = read(&db, &[work], Some(support::D)).await.unwrap();
    assert_eq!(facts[&work].len(), 2);
    assert!(facts[&work].iter().any(|p| p["hit"] == true));
    super::super::save_topic_map_command(
        &db,
        &super::super::TopicMapCommand::Breakout {
            idempotency_key: Uuid::new_v4().to_string(),
            domain_ref: support::D,
            work_ref: work,
            marked: true,
        },
    )
    .await
    .unwrap();
    let snapshot = super::super::read_topic_map(
        &db,
        &super::super::TopicMapQuery {
            domain_ref: Some(support::D),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        snapshot
            .works
            .iter()
            .find(|w| w.work_ref == work)
            .unwrap()
            .own
    );
    assert!(
        snapshot
            .works
            .iter()
            .find(|w| w.work_ref == work)
            .unwrap()
            .own_breakout
    );
    assert_eq!(snapshot.scope["operations"]["publishedWorkCount"], 1);
    assert_eq!(snapshot.scope["operations"]["publicationCount"], 2);
    report(publications[0], Some(4000_i64)).await;
    assert!(
        read(&db, &[work], Some(support::D)).await.unwrap()[&work]
            .iter()
            .all(|p| p["hit"] == false)
    );
    report(publications[0], None).await;
    let facts = read(&db, &[work], Some(support::D)).await.unwrap();
    assert!(facts[&work].iter().all(|p| p["views"].is_null()));
    sqlx::query("UPDATE linggan_ops_publication SET native_version=2 WHERE publication_ref=$1")
        .bind(publications[0])
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        read(&db, &[work], Some(support::D)).await.unwrap()[&work].len(),
        1
    );
    sqlx::query("UPDATE linggan_ops_publication SET native_record_state='revoked',native_revoked_at=scope_001_now(),native_revoked_by_member_ref=$2,native_revocation_reason='synthetic withdrawal' WHERE publication_ref=$1").bind(publications[1]).bind(member).execute(db.pool()).await.unwrap();
    assert!(
        read(&db, &[work], Some(support::D))
            .await
            .unwrap()
            .is_empty()
    );
}
