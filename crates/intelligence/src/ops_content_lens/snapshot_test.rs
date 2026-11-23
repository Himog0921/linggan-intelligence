use super::*;
#[path = "read_tests.rs"]
mod read_tests;

#[tokio::test]
#[ignore = "requires the disposable ops ledger proof database"]
async fn snapshot_generation_reuses_input_and_preserves_frozen_members() {
    use crate::ops_content_lens::{
        LabelFilter, SampleMetric, SnapshotScope, Subset, select_publications,
        snapshot::build_for_task,
    };
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = isolated_proof_schema(&url, "ops_content_lens_snapshot", fixture::FULL_MIGRATIONS)
        .await
        .unwrap();
    load_taxonomy(&db, &serde_json::to_vec(&synthetic_taxonomy()).unwrap())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("snapshot_test.sql"))
        .execute(db.pool())
        .await
        .unwrap();
    let first_pub: Uuid = sqlx::query_scalar("SELECT md5('pub1')::uuid")
        .fetch_one(db.pool())
        .await
        .unwrap();
    record_views(&db, first_pub, 1, Some(900)).await;
    record_views(&db, first_pub, 2, Some(1200)).await;
    let generate = || build_for_task(&db, SnapshotScope::default(), "ops_title_label");
    assert!(matches!(
        generate().await,
        Err(LensDefinitionError::MissingAgreement)
    ));
    sqlx::raw_sql(include_str!("schema_test.sql"))
        .execute(db.pool())
        .await
        .unwrap();

    let first = generate().await.unwrap();
    assert!(!first.reused);
    let repeated = generate().await.unwrap();
    assert!(repeated.reused);
    assert_eq!(first.snapshot_ref, repeated.snapshot_ref);
    let (_, metric) = metric_definition(&db, 1).await.unwrap();
    let frozen = |id| {
        sqlx::query_as::<_, (sqlx::types::Json<AnalysisInput>, Value)>(
            "SELECT analysis_input,body FROM linggan_ops_lens_snapshot WHERE snapshot_ref=$1",
        )
        .bind(id)
        .fetch_one(db.pool())
    };
    let (initial, body) = frozen(first.snapshot_ref).await.unwrap();
    let stored_hash: String = sqlx::query_scalar(
        "SELECT input_hash FROM linggan_ops_lens_snapshot WHERE snapshot_ref=$1",
    )
    .bind(first.snapshot_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(initial.input_hash().unwrap(), stored_hash);
    assert_eq!(initial.agreement_sources.len(), 1);
    assert_eq!(body["quality"]["titleKeys"], 1);
    assert_eq!(body["quality"]["publications"], 2);
    assert_eq!(body["quality"]["withViews"], 1);
    assert_eq!(
        body["effect"]["byDimension"]["offer"]["unavailableReason"],
        "agreement_below_threshold"
    );
    let selected = |input: &AnalysisInput, metric_kind, subset| {
        select_publications(input, &LabelFilter::default(), metric_kind, subset, &metric)
    };
    assert_eq!(
        selected(&initial, SampleMetric::Publications, Subset::All).len(),
        2
    );
    let known = selected(&initial, SampleMetric::OverThousand, Subset::Denominator);
    assert_eq!(known.len(), 1);
    assert_eq!(
        known,
        selected(&initial, SampleMetric::OverThousand, Subset::Numerator)
    );
    assert_eq!(initial.publications[&known[0]].views.value, Some(1200));
    let saved_body = serde_json::to_string(&body).unwrap();
    assert!(!saved_body.contains("样例 A!") && !saved_body.contains("样例A"));
    // The vocabulary display names are synthetic; raw titles never enter the saved input.
    assert!(!serde_json::to_string(&initial.0).unwrap().contains("样例"));
    let second_pub = *initial
        .publications
        .keys()
        .find(|id| **id != known[0])
        .unwrap();

    for (n, count, expected) in [(1, Some(5000_i64), 2), (2, None, 1)] {
        // Each update uses the ledger's real, append-only receipt and matching actor request.
        record_views(&db, second_pub, n, count).await;
        let (a, b) = tokio::join!(generate(), generate());
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_eq!(a.snapshot_ref, b.snapshot_ref);
        assert_ne!(a.reused, b.reused);
        assert_ne!(first.snapshot_ref, a.snapshot_ref);
        let (after, _) = frozen(a.snapshot_ref).await.unwrap();
        assert_eq!(
            selected(&after, SampleMetric::OverThousand, Subset::Denominator).len(),
            expected
        );
        assert!(after.publications[&second_pub].views.source.is_some());
        assert_eq!(
            selected(
                &frozen(first.snapshot_ref).await.unwrap().0,
                SampleMetric::OverThousand,
                Subset::Denominator
            ),
            known
        );
    }
    let before = generate().await.unwrap().snapshot_ref;
    let member: Uuid = sqlx::query_scalar("SELECT md5('member')::uuid")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let command = Uuid::new_v4();
    let decide = |id| {
        crate::ops_content_lens::adjudicate(
            &db,
            member,
            crate::ops_content_lens::Adjudication {
                command_ref: id,
                snapshot_ref: before,
                title_key: initial.publications[&first_pub].title_key.clone(),
                dimension: crate::ops_content_lens::LabelDimension::Offer,
                value: "actionable_method".into(),
                reason: None,
            },
        )
    };
    let after = decide(command).await.unwrap().snapshot_ref;
    assert_eq!(decide(command).await.unwrap().snapshot_ref, after);
    assert!(matches!(
        decide(Uuid::new_v4()).await,
        Err(LensDefinitionError::AdjudicationConflict)
    ));
    assert_eq!(generate().await.unwrap().snapshot_ref, after);
    assert_ne!(after, before);
    let (input, _) = frozen(after).await.unwrap();
    assert_eq!(
        select_publications(
            &input,
            &LabelFilter {
                offer: Some("actionable_method".into()),
                ..Default::default()
            },
            SampleMetric::Publications,
            Subset::All,
            &metric
        )
        .len(),
        2
    );
    assert_eq!(
        frozen(before).await.unwrap().0.publications[&second_pub].labels
            [&crate::ops_content_lens::LabelDimension::Offer]
            .value,
        "other"
    );
    let filtered = build_for_task(
        &db,
        SnapshotScope {
            from: Some("2026-11-02".into()),
            ..Default::default()
        },
        "ops_title_label",
    )
    .await
    .unwrap();
    assert_eq!(
        frozen(filtered.snapshot_ref)
            .await
            .unwrap()
            .0
            .total_publications,
        1
    );
    read_tests::verify_reads(&db, first.snapshot_ref, after, filtered.snapshot_ref).await;
}

async fn record_views(db: &Database, publication: Uuid, seq: i64, count: Option<i64>) {
    let request = Uuid::new_v4();
    let mut tx = db.pool().begin().await.unwrap();
    let report = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ops_native_write_request(request_ref,command_kind,actor_member_ref,payload_sha256,outcome,record_ref,resulting_version,outcome_code)
        VALUES($1,'view_report_record',md5('snapshot-member')::uuid,decode(repeat('00',32),'hex'),'accepted',$2,$3,'saved')")
        .bind(request).bind(publication).bind(seq).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ops_view_report(report_ref,publication_ref,report_kind,view_count,source_kind,reported_by_member_ref,request_ref,as_of)
        VALUES($1,$2,CASE WHEN $3::bigint IS NULL THEN 'cleared' ELSE 'value' END,$3,'linggan_ops_manual',md5('snapshot-member')::uuid,$4,scope_001_now())")
        .bind(report).bind(publication).bind(count).bind(request).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}
