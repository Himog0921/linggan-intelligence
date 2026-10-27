//! Disposable, synthetic proof; no providers, production tables, or source text fixtures.
#[path = "../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod full_schema_fixture;
use linggan_intelligence::{
    TopicMaterialMemberImport, TopicMaterialRole, TopicWorkspaceImport, import_topic_workspace,
    topic_map::*,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
fn domain() -> Uuid {
    "00000000-0000-4000-8000-000000000001".parse().unwrap()
}
async fn fixture_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("disposable proof URL");
    let database = isolated_proof_schema(&url, schema, full_schema_fixture::FULL_MIGRATIONS)
        .await
        .unwrap();
    let installed: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_receipt')IS NOT NULL")
            .fetch_one(database.pool())
            .await
            .unwrap();
    if !installed {
        sqlx::raw_sql(include_str!(
            "../../../database/migrations/0116_topic_map.sql"
        ))
        .execute(database.pool())
        .await
        .unwrap();
    }
    database
}
async fn work(
    database: &Database,
    index: usize,
    platform: &str,
    likes: Option<i64>,
    days: i32,
) -> Uuid {
    work_with_text(database, index, platform, likes, days, true).await
}

/// Match ordinary content admission: Dossier v2 owns a canonical body reading,
/// so Topic Map evidence must not read a raw detail row as its source text.
async fn publish_current_body(database: &Database, work: Uuid) {
    let mut transaction = database.pool().begin().await.unwrap();
    linggan_evidence::research_dossier::native::publish_current_body_in(&mut transaction, work)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}

async fn work_with_text(
    database: &Database,
    index: usize,
    platform: &str,
    likes: Option<i64>,
    days: i32,
    has_text: bool,
) -> Uuid {
    let task = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let package = Uuid::new_v4();
    let reference = Uuid::new_v4();
    let hash =
        linggan_evidence::creator_discovery::hash(&format!("synthetic-topic-map-{reference}"));
    sqlx::query("INSERT INTO linggan_runtime_task(task_id,task_spec_hash,task_spec,source,platform,page_type)VALUES($1,$2,'{}','manual',$3,'topic_map_proof')").bind(task).bind(&hash).bind(platform).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_runtime_attempt(attempt_id,task_id,producer_instance_id)VALUES($1,$2,$3)").bind(attempt).bind(task).bind(Uuid::new_v4()).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_runtime_capture_package(package_ref,attempt_id,task_id,producer_instance_id,package_kind,platform,package_hash,observed_at,captured_at,coverage,payload)SELECT $1,$2,$3,producer_instance_id,'content_detail',$4,$5,to_char(scope_001_now(),'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),to_char(scope_001_now(),'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),'{}','{}' FROM linggan_runtime_attempt WHERE attempt_id=$2").bind(package).bind(attempt).bind(task).bind(platform).bind(&hash).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_content(platform,content_external_id,public_ref,first_package_ref)VALUES($1,$2,$3,$4)").bind(platform).bind(format!("synthetic-topic-map-{index}")).bind(reference).bind(package).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason)VALUES($1,0,'accepted_for_library_content','SYNTHETIC_TOPIC_MAP_PROOF')").bind(package).execute(database.pool()).await.unwrap();
    let titles = [
        "合成样本·屏幕时间与即时刺激",
        "合成样本·理解孩子的困难",
        "合成样本·寻找支持",
        "合成样本·实践记录",
        "合成样本·长期调整",
        "合成样本·阶段不明",
        "合成样本·成人环境转换",
    ];
    sqlx::query("INSERT INTO linggan_material_content_detail(material_ref,content_public_ref,package_ref,record_ordinal,observed_at,title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,published_at_source_text,published_at_source_text_state,searchable_text,author_external_id,like_count,like_count_state,published_at,published_at_source_kind,published_at_source_field,published_at_precision,published_at_parser_version)VALUES($1,$2,$3,0,scope_001_now()::text,$4,'KNOWN',$5,'KNOWN',$6,'KNOWN','合成明确发布记录','KNOWN',$5,$7,$8,$9,scope_001_now()-($10*interval '1 day'),'platform_epoch','publishedAt','second','synthetic-proof.v1')").bind(Uuid::new_v4()).bind(reference).bind(package).bind(if has_text{titles[index%titles.len()]}else{""}).bind(if has_text{format!("SYNTHETIC ONLY 合成边界测试，第{index}篇：记录明确行动、尝试与限制，不表示真实研究。")}else{String::new()}).bind(if index==0{"合成我方账号"}else{"合成外部作者"}).bind(if index==0{"synthetic-own"}else{"synthetic-peer"}).bind(likes).bind(if likes.is_some(){"KNOWN"}else{"UNKNOWN"}).bind(days).execute(database.pool()).await.unwrap();
    publish_current_body(database, reference).await;
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref)VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(reference).bind(domain()).bind(package).execute(database.pool()).await.unwrap();
    reference
}
async fn topic(
    database: &Database,
    key: &str,
    label: &str,
    parent: Option<Uuid>,
    refs: &[Uuid],
) -> (Uuid, Uuid) {
    let request = TopicWorkspaceImport {
        idempotency_key: format!("topic-map-proof:{key}"),
        domain_key: "adhd-family".into(),
        canonical_key: key.into(),
        display_name: label.into(),
        definition_text: format!("合成定义：{label}，不表示市场实际范围。"),
        expected_version: None,
        adjudication_note: "合成fixture人工分类".into(),
        source_boundary: "合成验证，非实际研究".into(),
        members: refs
            .iter()
            .enumerate()
            .map(|(i, r)| TopicMaterialMemberImport {
                work_public_ref: *r,
                role: if i == 0 {
                    TopicMaterialRole::Support
                } else {
                    TopicMaterialRole::Boundary
                },
                rationale: "合成测试引用".into(),
            })
            .collect(),
    };
    let r = import_topic_workspace(database, &request).await.unwrap();
    save_topic_map_command(
        database,
        &TopicMapCommand::BindTopic {
            idempotency_key: format!("bind-proof:{key}"),
            domain_ref: domain(),
            topic_ref: r.topic_ref,
            parent_topic_ref: parent,
            expected_version: None,
        },
    )
    .await
    .unwrap();
    (r.topic_ref, r.definition_ref)
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL pgvector proof harness"]
async fn complete_canonical_map_dedup_unknown_history_and_idempotent_alternative() {
    let database = fixture_database("topic_map_browser_proof").await;
    let mut refs = Vec::new();
    for i in 0..7 {
        refs.push(
            work(
                &database,
                i,
                if i == 6 { "douyin" } else { "xhs" },
                [
                    Some(1200),
                    Some(500),
                    Some(0),
                    None,
                    Some(100),
                    Some(50),
                    Some(5000),
                ][i],
                if i == 0 { 100 } else { 1 },
            )
            .await,
        )
    }
    let (parent, _) = topic(
        &database,
        "synthetic-screen-time",
        "屏幕时间与即时刺激（合成）",
        None,
        &refs[..2],
    )
    .await;
    let (child, definition) = topic(
        &database,
        "synthetic-practice",
        "实践中的具体困难（合成）",
        Some(parent),
        &refs[1..4],
    )
    .await;
    let own = TopicMapCommand::OwnCreator {
        idempotency_key: "own-proof:0001".into(),
        domain_ref: domain(),
        platform: "xhs".into(),
        author_external_id: "synthetic-own".into(),
        active: true,
    };
    let r = save_topic_map_command(&database, &own).await.unwrap();
    assert_eq!(
        r.receipt_ref,
        save_topic_map_command(&database, &own)
            .await
            .unwrap()
            .receipt_ref
    );
    save_topic_map_command(
        &database,
        &TopicMapCommand::Breakout {
            idempotency_key: "breakout-proof:0001".into(),
            domain_ref: domain(),
            work_ref: refs[0],
            marked: true,
        },
    )
    .await
    .unwrap();
    let data = linggan_evidence::creator_discovery::load(
        &database,
        &serde_json::from_value(json!({"domain":domain()})).unwrap(),
    )
    .await
    .unwrap();
    for (i, reference) in refs.iter().take(6).enumerate() {
        let source = data
            .works
            .iter()
            .find(|w| w.work_ref == *reference)
            .unwrap();
        let f = source.fragments.iter().find(|f| f.field == "body").unwrap();
        append_work_annotation(
            &database,
            &TopicMapAnnotationRequest {
                idempotency_key: format!("annotation-proof:{i:04}"),
                domain_ref: domain(),
                work_public_ref: *reference,
                topic_ref: if i == 1 { Some(child) } else { None },
                definition_ref: if i == 1 { Some(definition) } else { None },
                method_version: "synthetic-human-proof.v1".into(),
                main_stage: if i < 5 {
                    MAIN_STAGES[i].into()
                } else {
                    "unclear".into()
                },
                involved_stages: if i < 5 {
                    vec![MAIN_STAGES[i].into(), MAIN_STAGES[(i + 1) % 5].into()]
                } else {
                    vec![]
                },
                overlays: if i == 3 {
                    vec![OVERLAYS[0].into()]
                } else {
                    vec![]
                },
                path: "family".into(),
                rationale: "合成引用验证".into(),
                evidence_citations: vec![TopicMapCitation {
                    fragment_id: f.fragment_id.clone(),
                    source_ref: f.source_ref,
                    field: f.field.clone(),
                }],
            },
        )
        .await
        .unwrap();
    }
    let map = read_topic_map(
        &database,
        &TopicMapQuery {
            domain_ref: Some(domain()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(map.statistics["workCount"], 7);
    assert_eq!(
        map.topics
            .iter()
            .find(|t| t.topic_ref == parent)
            .unwrap()
            .work_refs
            .len(),
        4
    );
    assert_eq!(map.statistics["platforms"][0]["unknownLikeCount"], 1);
    assert_eq!(
        map.statistics["platforms"][0]["highPerformanceThreshold"],
        serde_json::Value::Null
    );
    assert_eq!(map.journey["denominator"], 7);
    assert_eq!(
        map.journey["main"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["count"].as_u64().unwrap())
            .sum::<u64>(),
        7
    );
    let filtered = read_topic_map(
        &database,
        &TopicMapQuery {
            domain_ref: Some(domain()),
            window_days: Some(30),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(filtered.scope["ownHistoryPublishedCount"], 1);
    assert_eq!(filtered.statistics["ownPublishedCount"], 0);
    assert_eq!(filtered.statistics["workCount"], 6);
    let alternative = TopicMapCommand::SaveAlternative {
        idempotency_key: "alternative-proof:0001".into(),
        domain_ref: domain(),
        topic_ref: Some(child),
        definition_ref: Some(definition),
        title: "合成备选：第一步怎样更容易".into(),
        angle: "回答具体实践困难".into(),
        rationale: "仅合成范围内引用，不发起监控".into(),
        evidence_work_refs: vec![refs[1]],
        method_version: "synthetic-human-proof.v1".into(),
        research_result_ref: None,
        research_angle_index: None,
        research_opportunity_index: None,
    };
    let saved = save_topic_map_command(&database, &alternative)
        .await
        .unwrap();
    assert_eq!(
        saved.receipt_ref,
        save_topic_map_command(&database, &alternative)
            .await
            .unwrap()
            .receipt_ref
    );
    let restored = read_topic_map(
        &database,
        &TopicMapQuery {
            domain_ref: Some(domain()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(restored.alternatives.len(), 1);
    assert_eq!(restored.alternatives[0]["trackingEnabled"], false);
    let bad = TopicMapCommand::BindTopic {
        idempotency_key: "cycle-proof:0001".into(),
        domain_ref: domain(),
        topic_ref: parent,
        parent_topic_ref: Some(child),
        expected_version: Some(1),
    };
    assert!(matches!(
        save_topic_map_command(&database, &bad).await,
        Err(TopicMapError::Invalid(_))
    ));
    // Fresh annotation with forged source must not create a receipt or derived result.
    let mut forged = TopicMapAnnotationRequest {
        idempotency_key: "forged-proof:0001".into(),
        domain_ref: domain(),
        work_public_ref: refs[1],
        topic_ref: None,
        definition_ref: None,
        method_version: "synthetic-human-proof.v1".into(),
        main_stage: "begin_practice".into(),
        involved_stages: vec![],
        overlays: vec![],
        path: "family".into(),
        rationale: "合成伪引用反例".into(),
        evidence_citations: vec![TopicMapCitation {
            fragment_id: "forged".into(),
            source_ref: Uuid::new_v4(),
            field: "body".into(),
        }],
    };
    assert!(matches!(
        append_work_annotation(&database, &forged).await,
        Err(TopicMapError::Invalid(_))
    ));
    forged.idempotency_key = "forged-proof:0002".into();
    assert!(matches!(
        append_work_annotation(&database, &forged).await,
        Err(TopicMapError::Invalid(_))
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_alternative")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let shape=sqlx::query("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='linggan_topic_map_alternative'").fetch_all(database.pool()).await.unwrap();
    assert!(
        !shape
            .iter()
            .any(|r| r.get::<String, _>("column_name").contains("body"))
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL pgvector proof harness"]
async fn structural_preview_freezes_versions_and_reclassifies_without_inheriting_old_judgments() {
    use linggan_intelligence::topic_map_structure::*;
    let database = fixture_database("topic_map_structure_proof").await;
    let first = work(&database, 0, "xhs", Some(1000), 60).await;
    let second = work(&database, 1, "xhs", Some(20), 1).await;
    let third = work(&database, 2, "xhs", None, 2).await;
    let fourth = work(&database, 3, "xhs", Some(400), 3).await;
    let (a, ad) = topic(
        &database,
        "structure-a",
        "合成方向A",
        None,
        &[first, second],
    )
    .await;
    let (b, bd) = topic(
        &database,
        "structure-b",
        "合成方向B",
        None,
        &[third, fourth],
    )
    .await;
    let plan = StructurePlan {
        request_ref: Uuid::new_v4(),
        domain_ref: domain(),
        kind: "merge".into(),
        sources: vec![
            StructureSource {
                topic_ref: a,
                definition_ref: ad,
            },
            StructureSource {
                topic_ref: b,
                definition_ref: bd,
            },
        ],
        destinations: vec![StructureDestination {
            display_name: "合成合并方向".into(),
            definition_text: "显式选择的合成方向，未继承旧判断。".into(),
            parent_topic_ref: None,
            members: vec![TopicMaterialMemberImport {
                work_public_ref: first,
                role: TopicMaterialRole::Support,
                rationale: "用户明确重新分配的依据".into(),
            }],
        }],
        rationale: "合成结构更正验证".into(),
    };
    let preview = preview_structure(&database, &plan).await.unwrap();
    assert_eq!(
        preview["impact"]["unassignedWorkRefs"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(preview["impact"]["claimsInherited"], false);
    let bad = ApplyStructure {
        plan: plan.clone(),
        preview_hash: "a".repeat(64),
    };
    assert!(matches!(
        apply_structure(&database, &bad).await,
        Err(TopicMapError::Conflict)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_topic_workspace")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 2,
        "rejected preview hash must not partially create destinations"
    );
    let request = ApplyStructure {
        plan: plan.clone(),
        preview_hash: preview["previewHash"].as_str().unwrap().into(),
    };
    let receipt = apply_structure(&database, &request).await.unwrap();
    assert_eq!(receipt["state"], "persisted");
    assert_eq!(apply_structure(&database, &request).await.unwrap(), receipt);
    let new_topic: Uuid = receipt["destinations"][0]["topicRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let snapshot = read_topic_map(
        &database,
        &TopicMapQuery {
            domain_ref: Some(domain()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot
            .topics
            .iter()
            .find(|t| t.topic_ref == new_topic)
            .unwrap()
            .direct_work_refs,
        vec![first]
    );
    let old_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_topic_definition WHERE definition_ref=ANY($1)",
    )
    .bind(vec![ad, bd])
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        old_count, 2,
        "old definitions remain exact historical references"
    );
    let mut foreign = plan.clone();
    foreign.request_ref = Uuid::new_v4();
    foreign.destinations[0].members[0].work_public_ref = Uuid::new_v4();
    assert!(preview_structure(&database, &foreign).await.is_err());
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof harness"]
async fn saved_alternative_freezes_source_version_and_concurrent_saves_do_not_starve_pool() {
    let database = fixture_database("topic_map_alternative_versions").await;
    let refs = vec![
        work(&database, 0, "xhs", Some(100), 1).await,
        work(&database, 1, "xhs", Some(200), 1).await,
    ];
    let (topic_ref, definition_ref) = topic(
        &database,
        "synthetic-backup-source",
        "合成版本边界",
        None,
        &refs,
    )
    .await;
    let mut handles = Vec::new();
    // Database has four pooled connections. Eight simultaneous writers used to fill it with
    // advisory-lock waiters while the lock holder tried to acquire a fifth reader connection.
    for i in 0..8 {
        let db = database.clone();
        let evidence = refs.clone();
        handles.push(tokio::spawn(async move {
            save_topic_map_command(
                &db,
                &TopicMapCommand::SaveAlternative {
                    idempotency_key: format!("backup-concurrent:{i}"),
                    domain_ref: domain(),
                    topic_ref: Some(topic_ref),
                    definition_ref: Some(definition_ref),
                    title: "合成保存的角度".into(),
                    angle: "原来源版本支持的合成推论".into(),
                    rationale: "合成验证".into(),
                    evidence_work_refs: evidence,
                    method_version: "synthetic.v1".into(),
                    research_result_ref: None,
                    research_angle_index: None,
                    research_opportunity_index: None,
                },
            )
            .await
        }));
    }
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
    })
    .await
    .expect("concurrent saves must complete without nested pool acquisition");
    let query = TopicMapQuery {
        domain_ref: Some(domain()),
        ..Default::default()
    };
    let saved = read_topic_map(&database, &query).await.unwrap();
    assert_eq!(saved.alternatives.len(), 8);
    assert!(
        saved
            .alternatives
            .iter()
            .all(|a| a["sourceState"] == "available" && a["angle"].is_string())
    );
    let package: Uuid = sqlx::query_scalar(
        "SELECT first_package_ref FROM linggan_material_content WHERE public_ref=$1",
    )
    .bind(refs[0])
    .fetch_one(database.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason)VALUES($1,1,'accepted_for_library_content','SYNTHETIC_NEW_SOURCE_VERSION')").bind(package).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_content_detail(material_ref,content_public_ref,package_ref,record_ordinal,observed_at,title,title_state,body_text,body_state,searchable_text,creator_display_name,creator_display_name_state,published_at_source_text_state) VALUES($1,$2,$3,1,scope_001_now()::text,'合成新来源版本','KNOWN','SYNTHETIC CHANGED BODY','KNOWN','SYNTHETIC CHANGED BODY','合成我方账号','KNOWN','UNKNOWN')").bind(Uuid::new_v4()).bind(refs[0]).bind(package).execute(database.pool()).await.unwrap();
    publish_current_body(&database, refs[0]).await;
    let changed = read_topic_map(&database, &query).await.unwrap();
    let alternative: Uuid = changed.alternatives[0]["alternativeRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let original = read_saved_alternative(&database, domain(), alternative)
        .await
        .unwrap();
    assert_eq!(original.source_state, "version_changed_available");
    assert!(
        original
            .fragments
            .iter()
            .any(|f| f.work_ref == refs[0] && f.field == "body" && f.text.contains("第0篇"))
    );
    assert!(
        original
            .fragments
            .iter()
            .all(|f| !f.text.contains("SYNTHETIC CHANGED BODY"))
    );
    assert_eq!(
        original.original_definition.as_ref().unwrap()["definitionRef"],
        definition_ref.to_string()
    );

    assert!(
        changed
            .alternatives
            .iter()
            .all(|a| a["sourceState"] == "version_changed_available" && a["angle"].is_string())
    );
    assert!(
        changed
            .alternatives
            .iter()
            .all(|a| a["definitionRef"] == definition_ref.to_string()
                && a["evidenceWorkRefs"].as_array().unwrap().len() == 2)
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL pgvector proof harness"]
async fn overview_batches_preserve_complete_statistics_and_canonical_work_fields() {
    let db = fixture_database("topic_map_overview_perf").await;
    let mut refs = Vec::new();
    for i in 0..501 {
        refs.push(
            work(
                &db,
                i,
                if i % 2 == 0 { "xhs" } else { "douyin" },
                if i % 3 == 0 { None } else { Some(i as i64) },
                1,
            )
            .await,
        );
    }
    let (parent, _) = topic(&db, "overview-parent", "合成父主题", None, &refs[..3]).await;
    let (child, _) = topic(
        &db,
        "overview-child",
        "合成子主题",
        Some(parent),
        &refs[..3],
    )
    .await;
    let q: TopicMapQuery = serde_json::from_value(json!({"domainRef":domain()})).unwrap();
    let map = read_topic_map(&db, &q).await.unwrap();
    assert_eq!(
        map.works.len(),
        501,
        "must cross the 500-work batch boundary without truncation"
    );
    assert_eq!(map.statistics["workCount"], 501);
    let platforms = map.statistics["platforms"].as_array().unwrap();
    assert_eq!(
        platforms
            .iter()
            .map(|p| p["unknownLikeCount"].as_u64().unwrap())
            .sum::<u64>(),
        167
    );
    for batch in refs.chunks(100) {
        let full_q = serde_json::from_value(json!({"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery","domainRef":domain(),"publicRefs":batch})).unwrap();
        let full = linggan_evidence::read_work_resources(&db, &full_q)
            .await
            .unwrap();
        let compact = linggan_evidence::read_work_resource_overviews(
            &db,
            domain(),
            batch,
            map.scope["asOf"].as_str().unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(compact.len(), full.items.len());
        for item in compact {
            let original = full
                .items
                .iter()
                .find(|f| f.identity.public_ref == item.identity.public_ref)
                .unwrap();
            assert_eq!(
                serde_json::to_value(&item.display).unwrap(),
                serde_json::to_value(&original.display).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&item.preview).unwrap(),
                serde_json::to_value(&original.preview).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&item.evidence_fragment).unwrap(),
                serde_json::to_value(&original.evidence_fragment).unwrap()
            );
            assert_eq!(
                item.inspector["derivatives"],
                original.inspector["derivatives"]
            );
            assert_eq!(
                item.summary.restriction_state,
                original.summary.restriction_state
            );
            assert_eq!(item.media["cover"], original.media["cover"]);
        }
    }
    // Pin the pool to one free backend so both the read and checks use it.
    let mut held = Vec::new();
    for _ in 0..3 {
        held.push(db.pool().acquire().await.unwrap());
    }
    let mut connection = db.pool().acquire().await.unwrap();
    sqlx::raw_sql("SET jit=on; SET jit_above_cost=0; SET plan_cache_mode=force_generic_plan")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    for _ in 0..8 {
        let compact = linggan_evidence::read_work_resource_overviews(
            &db,
            domain(),
            &refs[..3],
            map.scope["asOf"].as_str().unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(compact.len(), 3);
        let qualified = linggan_evidence::creator_discovery::load_topic_materials_for_works(
            &db,
            domain(),
            &refs[..3],
        )
        .await
        .unwrap();
        assert_eq!(qualified.works.len(), 3);
        let mut connection = db.pool().acquire().await.unwrap();
        let (jit, plan): (String, String) =
            sqlx::query_as("SELECT current_setting('jit'),current_setting('plan_cache_mode')")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
        assert_eq!(jit, "on", "local read setting must not leak to the pool");
        assert_eq!(
            plan, "force_generic_plan",
            "local plan selection must restore the caller setting"
        );
    }
    let mut connection = db.pool().acquire().await.unwrap();
    sqlx::raw_sql("RESET jit; RESET jit_above_cost; RESET plan_cache_mode")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    drop(held);
    let parent_topic = map.topics.iter().find(|t| t.topic_ref == parent).unwrap();
    assert_eq!(
        parent_topic.work_refs.len(),
        3,
        "parent union is deduplicated"
    );
    assert_eq!(parent_topic.statistics["workCount"], 3);
    let selected: TopicMapQuery =
        serde_json::from_value(json!({"domainRef":domain(),"topicRef":child})).unwrap();
    let selected = read_topic_map(&db, &selected).await.unwrap();
    assert_eq!(selected.works.len(), 3);
    assert_eq!(
        selected
            .topics
            .iter()
            .find(|t| t.topic_ref == parent)
            .unwrap()
            .statistics,
        parent_topic.statistics
    );
    let scope = serde_json::from_value(json!({"domain":domain()})).unwrap();
    let full = linggan_evidence::creator_discovery::load(&db, &scope)
        .await
        .unwrap();
    let bounded = linggan_evidence::creator_discovery::load_for_works(&db, &scope, &refs[..5])
        .await
        .unwrap();
    assert_eq!(bounded.works.len(), 3); // same xhs-only annotation scope
    for work in bounded.works {
        let original = full
            .works
            .iter()
            .find(|w| w.work_ref == work.work_ref)
            .unwrap();
        assert_eq!(
            serde_json::to_value(work.fragments).unwrap(),
            serde_json::to_value(&original.fragments).unwrap()
        );
    }
    assert!(
        linggan_evidence::creator_discovery::load_for_works(&db, &scope, &[])
            .await
            .unwrap()
            .works
            .is_empty()
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL pgvector proof harness"]
async fn manual_alternative_without_classification_keeps_sources_and_rejects_false_lineage() {
    let db = fixture_database("topic_map_manual_alternative_proof").await;
    let source = work(&db, 0, "xhs", Some(0), 1).await;
    let command = TopicMapCommand::SaveAlternative {
        idempotency_key: "manual-without-topic".into(),
        domain_ref: domain(),
        topic_ref: None,
        definition_ref: None,
        title: "人工选题草案".into(),
        angle: "由操作者提出的具体问题".into(),
        rationale: "仅保留材料，不声称已有主题或模型结论".into(),
        evidence_work_refs: vec![source],
        method_version: "topic-map.manual.v1".into(),
        research_result_ref: None,
        research_angle_index: None,
        research_opportunity_index: None,
    };
    let first = save_topic_map_command(&db, &command).await.unwrap();
    let repeated = save_topic_map_command(&db, &command).await.unwrap();
    assert_eq!(first.receipt_ref, repeated.receipt_ref);
    let saved = read_saved_alternative(&db, domain(), first.subject_ref.unwrap())
        .await
        .unwrap();
    assert_eq!(saved.source_state, "available");
    assert!(
        saved.topic_ref.is_none()
            && saved.definition_ref.is_none()
            && saved.original_definition.is_none()
    );
    assert!(!saved.fragments.is_empty());
    assert!(saved.fragments.iter().all(|f| f.work_ref == source));
    let map = read_topic_map(
        &db,
        &TopicMapQuery {
            domain_ref: Some(domain()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(map.alternatives.len(), 1);
    assert!(
        map.works[0].topic_refs.is_empty(),
        "saving a human idea must not classify its source"
    );
    let tasks: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_topic_research_window")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(tasks, 0, "saving does not start research");
    for change in [
        json!({"definitionRef":Uuid::new_v4()}),
        json!({"methodVersion":"topic-map.research.v6"}),
        json!({"researchAngleIndex":0}),
        json!({"evidenceWorkRefs":[]}),
        json!({"evidenceWorkRefs":[source,source]}),
        json!({"domainRef":Uuid::new_v4()}),
    ] {
        let mut value = serde_json::to_value(&command).unwrap();
        value["idempotencyKey"] = json!(Uuid::new_v4().to_string());
        for (key, value_change) in change.as_object().unwrap() {
            value[key] = value_change.clone();
        }
        let invalid: TopicMapCommand = serde_json::from_value(value).unwrap();
        assert!(
            save_topic_map_command(&db, &invalid).await.is_err(),
            "must reject {change}"
        );
    }
    let blank_source = work_with_text(&db, 1, "xhs", Some(0), 1, false).await;
    let mut blank_command = serde_json::to_value(&command).unwrap();
    blank_command["idempotencyKey"] = json!("manual-empty-fragments");
    blank_command["evidenceWorkRefs"] = json!([blank_source]);
    assert!(
        save_topic_map_command(&db, &serde_json::from_value(blank_command).unwrap())
            .await
            .is_err(),
        "empty author fragments cannot create an immediately unreadable manual alternative"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_alternative")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "invalid save rolls back its alternative and receipt"
    );
    // Restriction uses the append-only media origin and disposition contract.
    let package: Uuid = sqlx::query_scalar(
        "SELECT first_package_ref FROM linggan_material_content WHERE public_ref=$1",
    )
    .bind(source)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let slot = "synthetic-manual-source";
    let observation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason) VALUES($1,1,'accepted_for_media_identity','SYNTHETIC')").bind(package).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_slot(slot_key,platform,content_external_id,role,ordinal,first_package_ref) SELECT $1,platform,content_external_id,'cover',1,first_package_ref FROM linggan_material_content WHERE public_ref=$2").bind(slot).bind(source).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_observation(observation_ref,slot_key,package_ref,observed_external_uri,observed_at) VALUES($1,$2,$3,'https://example.invalid/synthetic.jpg',scope_001_now()::text)").bind(observation).bind(slot).bind(package).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_media_origin(observation_ref,content_public_ref,slot_key,package_ref,record_ordinal,source_generation,purpose,producer_ordinal,display_order_state,display_order_basis,candidate_set_state,composite_state) VALUES($1,$2,$3,$4,1,1,'cover',1,'UNKNOWN','unknown','UNKNOWN','NOT_APPLICABLE')").bind(observation).bind(source).bind(slot).bind(package).execute(db.pool()).await.unwrap();
    let projection = linggan_intelligence::topic_map::read_model::TopicMapReadModel::default();
    let projection_query = TopicMapQuery {
        domain_ref: Some(domain()),
        ..Default::default()
    };
    let before_restriction = projection.read(&db, &projection_query).await.unwrap();
    assert!(
        before_restriction
            .works
            .iter()
            .any(|w| w.work_ref == source && w.readable)
    );
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-proof','manual source withdrawal',clock_timestamp())").bind(Uuid::new_v4()).bind(slot).execute(db.pool()).await.unwrap();
    let after_restriction = projection.read(&db, &projection_query).await.unwrap();
    assert!(
        !after_restriction
            .works
            .iter()
            .any(|w| w.work_ref == source && w.readable),
        "withdrawal cannot reuse a qualified page snapshot"
    );
    let withdrawn = read_saved_alternative(&db, domain(), first.subject_ref.unwrap())
        .await
        .unwrap();
    assert_eq!(withdrawn.source_state, "source_unavailable");
    assert!(withdrawn.title.is_none() && withdrawn.fragments.is_empty());
    let mut again = serde_json::to_value(&command).unwrap();
    again["idempotencyKey"] = json!("withdrawn-manual");
    assert!(
        save_topic_map_command(&db, &serde_json::from_value(again).unwrap())
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL pgvector proof harness"]
async fn page_read_model_reuses_complete_results_and_invalidates_update_delete_and_ddl() {
    use linggan_intelligence::topic_map::read_model::{
        TopicMapReadModel, first_page, overview, source_revision,
    };
    use std::sync::Arc;
    let db = fixture_database("topic_map_page_read_model").await;
    let mut refs = Vec::new();
    for index in 0..13 {
        refs.push(work(&db, index, "xhs", Some(index as i64), 0).await);
    }
    let (topic_ref, _) = topic(&db, "read-model", "合成读取投影", None, &refs).await;
    let query = TopicMapQuery {
        domain_ref: Some(domain()),
        ..Default::default()
    };
    let model = TopicMapReadModel::default();
    let first = model.read(&db, &query).await.unwrap();
    let same = model.read(&db, &query).await.unwrap();
    assert!(
        Arc::ptr_eq(&first, &same),
        "unchanged authoritative rows reuse the complete projection"
    );
    assert_eq!(first.scope["totalWorkCount"], 13);
    let page = first_page(&first).unwrap();
    assert_eq!(page["works"].as_array().unwrap().len(), 12);
    assert_eq!(page["scope"]["totalWorkCount"], 13);
    assert_eq!(page["statistics"], first.statistics);
    let compact = overview(&first).unwrap();
    assert_eq!(compact["works"].as_array().unwrap().len(), 13);
    assert_eq!(compact["statistics"], first.statistics);
    assert!(compact["works"][0].get("sourceManifest").is_none());
    // Producer state and patrol fairness are separate from material permissions.
    sqlx::query("INSERT INTO linggan_creator_author_override(domain_ref,platform,author_external_id)VALUES($1,'xhs','synthetic-cache-author')")
        .bind(domain()).execute(db.pool()).await.unwrap();
    let target = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,source)VALUES($1,'xhs','creator','synthetic-cache-target','manual')")
        .bind(target).execute(db.pool()).await.unwrap();
    let controlled = model.read(&db, &query).await.unwrap();
    let control_revision = source_revision(&db).await.unwrap();
    sqlx::query("UPDATE collection_observation_target SET last_scheduler_considered_at=scope_001_now() WHERE target_ref=$1")
        .bind(target).execute(db.pool()).await.unwrap();
    assert_eq!(control_revision, source_revision(&db).await.unwrap());
    assert!(Arc::ptr_eq(
        &controlled,
        &model.read(&db, &query).await.unwrap()
    ));
    sqlx::query(
        "UPDATE linggan_creator_author_override SET manual_overrides='{\"syntheticProbe\":true}'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let result_revision = source_revision(&db).await.unwrap();
    assert_ne!(
        control_revision, result_revision,
        "human annotations invalidate the current source projection"
    );
    sqlx::query(
        "UPDATE collection_observation_target SET lifecycle_state='dismissed' WHERE target_ref=$1",
    )
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    assert_ne!(
        result_revision,
        source_revision(&db).await.unwrap(),
        "target lifecycle remains a source dependency"
    );

    // Accepted definitions are append-only; create the next version through its owner.
    let revised = import_topic_workspace(
        &db,
        &TopicWorkspaceImport {
            idempotency_key: "read-model-next-definition".into(),
            domain_key: "adhd-family".into(),
            canonical_key: "read-model".into(),
            display_name: "合成更新后的定义".into(),
            definition_text: "合成定义：读取投影修订".into(),
            expected_version: Some(1),
            adjudication_note: "合成当前资格反例".into(),
            source_boundary: "合成验证".into(),
            members: refs
                .iter()
                .enumerate()
                .map(|(index, r)| TopicMaterialMemberImport {
                    work_public_ref: *r,
                    role: if index == 0 {
                        TopicMaterialRole::Support
                    } else {
                        TopicMaterialRole::Boundary
                    },
                    rationale: "合成来源".into(),
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    assert_eq!(revised.topic_ref, topic_ref);
    let changed = model.read(&db, &query).await.unwrap();
    assert!(!Arc::ptr_eq(&first, &changed));
    assert_eq!(
        changed
            .topics
            .iter()
            .find(|t| t.topic_ref == topic_ref)
            .unwrap()
            .display_name,
        "合成更新后的定义"
    );
    // A new relation is automatically covered; same-count replacement and DELETE are distinct.
    let before = source_revision(&db).await.unwrap();
    sqlx::query("CREATE TABLE topic_map_revision_probe(value integer)")
        .execute(db.pool())
        .await
        .unwrap();
    let created = source_revision(&db).await.unwrap();
    assert_ne!(before, created);
    sqlx::query("INSERT INTO topic_map_revision_probe VALUES(1)")
        .execute(db.pool())
        .await
        .unwrap();
    let inserted = source_revision(&db).await.unwrap();
    assert_ne!(created, inserted);
    sqlx::query("UPDATE topic_map_revision_probe SET value=2")
        .execute(db.pool())
        .await
        .unwrap();
    let updated = source_revision(&db).await.unwrap();
    assert_ne!(inserted, updated);
    sqlx::query("DELETE FROM topic_map_revision_probe")
        .execute(db.pool())
        .await
        .unwrap();
    assert_ne!(updated, source_revision(&db).await.unwrap());
    // New accepted material invalidates full statistics and crosses the first-page boundary.
    let added = work(&db, 13, "xhs", None, 0).await;
    let updated = model.read(&db, &query).await.unwrap();
    assert_eq!(updated.scope["totalWorkCount"], 14);
    assert!(updated.works.iter().any(|w| w.work_ref == added));
    sqlx::query("INSERT INTO linggan_creator_discovery_threshold(domain_ref,platform,like_threshold) VALUES($1,'xhs',10)")
        .bind(domain()).execute(db.pool()).await.unwrap();
    let settings = model.read(&db, &query).await.unwrap();
    let before_tick = source_revision(&db).await.unwrap();
    let mut executor = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "synthetic-topic-read-heartbeat",
        4,
    )
    .await
    .unwrap();
    executor.heartbeat().await.unwrap();
    executor.request_drain().await.unwrap();
    executor.complete_drain().await.unwrap();
    assert_eq!(
        before_tick,
        source_revision(&db).await.unwrap(),
        "pure worker ticks cannot invalidate source qualification"
    );
    assert!(Arc::ptr_eq(
        &settings,
        &model.read(&db, &query).await.unwrap()
    ));
    sqlx::query(
        "UPDATE linggan_creator_discovery_threshold SET like_threshold=11,revision=revision+1",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let changed_policy = source_revision(&db).await.unwrap();
    assert_ne!(
        before_tick, changed_policy,
        "business settings remain qualification dependencies"
    );
    assert!(!Arc::ptr_eq(
        &settings,
        &model.read(&db, &query).await.unwrap()
    ));
    sqlx::query("UPDATE linggan_model_workspace SET workspace_ref=$1")
        .bind(Uuid::new_v4())
        .execute(db.pool())
        .await
        .unwrap();
    assert_ne!(
        changed_policy,
        source_revision(&db).await.unwrap(),
        "workspace identity is never discarded with its heartbeat"
    );
}
