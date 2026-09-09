#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_intelligence::*, comment_intelligence_actions::*, comment_intelligence_problems::*,
};
use research_fixture::*;
use serde_json::json;
use uuid::Uuid;
fn own() -> Uuid {
    Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap()
}
fn all() -> ResearchScope {
    ResearchScope {
        domain: Some(own()),
        from: Some("2020-01-01T00:00:00Z".into()),
        to: Some("2099-01-01T00:00:00Z".into()),
        ..Default::default()
    }
}
#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn one_scope_counts_unique_comments_and_preserves_first_observation() {
    let db = fixture::proof_database("ci_scope_identity").await;
    detail(&db, "ci-note", "真实上下文 SYNTHETIC").await;
    let first = comment(
        &db,
        "ci-note",
        "same",
        "A娃执行功能困难，写作业需要陪伴",
        "2026-08-28T10:00:00Z",
    )
    .await;
    let newer = comment(
        &db,
        "ci-note",
        "same",
        "A娃执行功能困难，写作业需要陪伴",
        "2026-08-29T10:00:00Z",
    )
    .await;
    comment(
        &db,
        "ci-note",
        "second",
        "另一个用户讨论睡眠",
        "2026-08-28T10:00:00Z",
    )
    .await;
    let r = read(&db, &all()).await.unwrap();
    assert_eq!(r["summary"]["comments"], 2);
    assert_eq!(r["page"]["total"], 2);
    let item = r["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["sourceRef"] == json!(newer))
        .unwrap();
    assert_eq!(item["canonicalRef"], json!(first));
    assert!(r["summary"]["lenses"]["need"].is_null());
    execute_action(&db,json!({"commandRef":Uuid::new_v4(),"domain":own(),"kind":"bookmark","sourceRef":first,"expectedRevision":0,"payload":{"enabled":true,"note":"观察"},"reason":""})).await.unwrap();
    let q = ResearchScope {
        bookmarked_only: Some(true),
        ..all()
    };
    let bookmarked = read(&db, &q).await.unwrap();
    assert_eq!(bookmarked["summary"]["comments"], 1);
    assert_eq!(bookmarked["page"]["items"][0]["sourceRef"], json!(newer));
    reconcile_problem_index(&db, 100).await.unwrap();
    let q = ResearchScope {
        term: Some("执行功能".into()),
        ..all()
    };
    let term = read(&db, &q).await.unwrap();
    assert_eq!(term["summary"]["comments"], 1);
    let revision = read(&db, &all()).await.unwrap()["scope"]["resultRevision"]
        .as_str()
        .unwrap()
        .to_string();
    execute_action(&db,json!({"commandRef":Uuid::new_v4(),"domain":own(),"kind":"term_hide","expectedRevision":0,"payload":{"term":"执行功能","hidden":true},"reason":"合成词表更正"})).await.unwrap();
    let updated = read(
        &db,
        &ResearchScope {
            result_revision: Some(revision),
            ..all()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated["scope"]["updated"], true);
    assert!(
        !updated["terms"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["term"] == "执行功能")
    );
    let q = ResearchScope {
        time_basis: Some("published".into()),
        ..all()
    };
    let unknown = read(&db, &q).await.unwrap();
    assert_eq!(unknown["summary"]["comments"], 0);
    assert_eq!(unknown["summary"]["excludedPublished"], 2);
    linggan_evidence::comment_research_read::restrict_comment_research_source(
        &db,
        first,
        "合成撤回",
    )
    .await
    .unwrap();
    assert_eq!(read(&db, &all()).await.unwrap()["summary"]["comments"], 1);
    assert!(source(&db, own(), first).await.is_err());
}
#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn sources_from_another_domain_never_join_own_query_or_actions() {
    let db = fixture::proof_database("ci_scope_domain").await;
    detail(&db, "own-note", "own").await;
    let source_ref = comment(
        &db,
        "own-note",
        "own",
        "SYNTHETIC 用户问题",
        "2026-08-28T10:00:00Z",
    )
    .await;
    let external = Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
    let q = ResearchScope {
        domain: Some(external),
        ..all()
    };
    assert_eq!(read(&db, &q).await.unwrap()["summary"]["comments"], 0);
    assert!(source(&db, external, source_ref).await.is_err());
    assert!(execute_action(&db,json!({"commandRef":Uuid::new_v4(),"domain":external,"kind":"bookmark","sourceRef":source_ref,"expectedRevision":0,"payload":{"enabled":true},"reason":""})).await.is_err());
    let r = prepare(
        &db,
        &Prepare {
            scope: all(),
            source_refs: Some(vec![source_ref]),
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(r["count"], 1);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn semantic_lenses_are_nonexclusive_and_resonance_has_group_evidence() {
    let db = fixture::proof_database("ci_semantic_scoped").await;
    for i in 0..6 {
        let note = format!("scope-work-{}", i / 2);
        detail(&db, &note, &format!("合成作品 {}", i / 2)).await;
        let body = format!("A娃每天写作业需要家长一直陪伴，想知道怎么办，例子{i}");
        let id = comment(
            &db,
            &note,
            &format!("scope-comment-{i}"),
            &body,
            "2026-08-29T10:00:00Z",
        )
        .await;
        let evidence = json!([{"sourceRef":id,"startChar":0,"endChar":body.chars().count()}]);
        let result = json!({"sourceRef":id,"sourceSha256":linggan_intelligence::comment_research::comment_source_hash(&body),"spans":[],"contextRefs":{"researchSourceRefs":[id]},"semantic":{"outcome":"interpretable","labels":[{"label":"need","evidence":evidence},{"label":"story","evidence":evidence}],"problems":[{"name":"家长陪伴成本","meaning":"孩子写作业需要持续陪伴，家长在问如何减轻持续陪伴","evidence":evidence}],"stances":[{"target":"写作业需要持续陪伴","position":"support","evidence":evidence}],"contextMissing":[],"uncertaintyReason":null}});
        sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) SELECT $1,$2,rule.schema_version,'synthetic.v1','succeeded',$3 FROM linggan_comment_research_rule_active active JOIN linggan_comment_research_rule_revision rule USING(rule_revision_ref)").bind(Uuid::new_v4()).bind(id).bind(result).execute(db.pool()).await.unwrap();
    }
    seed_current_query_qualification(&db).await;
    reconcile_problem_index(&db, 100).await.unwrap();
    let r = read(&db, &all()).await.unwrap();
    assert_eq!(r["summary"]["comments"], 6);
    assert_eq!(r["summary"]["lenses"]["need"], 6);
    assert_eq!(r["summary"]["lenses"]["story"], 6);
    assert_eq!(r["summary"]["lenses"]["resonance"], 6);
    assert_eq!(r["problems"].as_array().unwrap().len(), 1);
    assert!(r["observations"].as_array().unwrap().is_empty());
    // Explicit synthetic SQL fixture only exercises the release gate; never a real evaluation.
    sqlx::query("INSERT INTO linggan_ci_rule_release(domain_ref,revision,advanced_release_enabled,evaluation_report,approved_at) VALUES($1,1,true,'{\"isSynthetic\":true}',scope_001_now())").bind(own()).execute(db.pool()).await.unwrap();
    let released = read(&db, &all()).await.unwrap();
    assert_eq!(released["observations"][0]["type"], "recurrence");
    assert_eq!(released["observations"][0]["comments"], 6);
    assert_eq!(released["observations"][0]["works"], 3);
    assert!(released["observations"][0]["representative"]["body"].is_string());
    let q = ResearchScope {
        lenses: Some("need,story".into()),
        ..all()
    };
    assert_eq!(read(&db, &q).await.unwrap()["page"]["total"], 6);
    let first = Uuid::parse_str(r["page"]["items"][0]["sourceRef"].as_str().unwrap()).unwrap();
    let detail = source(&db, own(), first).await.unwrap();
    assert_eq!(detail["source"]["labels"].as_array().unwrap().len(), 3);
    assert_eq!(detail["problems"].as_array().unwrap().len(), 1);
    assert!(detail["source"]["researchRevision"].is_number());
    assert_eq!(detail["thread"].as_array().unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL synthetic performance proof"]
async fn hundred_thousand_comment_scope_is_paged_and_reports_server_p95() {
    let db = fixture::proof_database("ci_scope_scale").await;
    let domain = Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
    // Synthetic SQL fixture measures the read path only, not producer ingestion or model quality.
    sqlx::query("INSERT INTO cross_industry_sample(sample_ref,domain_ref,platform,content_external_id,title) SELECT md5('ci-scale-work-'||n)::uuid,$1,'xhs','SYNTHETIC-'||n,'SYNTHETIC / NOT EVIDENCE '||n FROM generate_series(1,10) n").bind(domain).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO cross_industry_comment(comment_ref,sample_ref,domain_ref,comment_external_id,body_text,body_state,like_count) SELECT md5('ci-scale-comment-'||n)::uuid,md5('ci-scale-work-'||(1+n%10))::uuid,$1,'SYNTHETIC-'||n,'SYNTHETIC / NOT EVIDENCE 评论样本，讨论作业和家庭沟通 '||n,'KNOWN',n%80 FROM generate_series(1,100000) n").bind(domain).execute(db.pool()).await.unwrap();
    sqlx::raw_sql("ANALYZE cross_industry_comment; ANALYZE cross_industry_sample;")
        .execute(db.pool())
        .await
        .unwrap();
    let q = ResearchScope {
        domain: Some(domain),
        ..all()
    };
    let cold = std::time::Instant::now();
    let first = read(&db, &q).await.unwrap();
    let cold = cold.elapsed().as_millis();
    assert_eq!(first["page"]["total"], 100000);
    assert_eq!(first["page"]["items"].as_array().unwrap().len(), 20);
    assert!(first.to_string().len() < 100000);
    let mut times = Vec::new();
    for _ in 0..20 {
        let now = std::time::Instant::now();
        read(&db, &q).await.unwrap();
        times.push(now.elapsed().as_millis());
    }
    eprintln!("CI_SCALE_OVERVIEW_SAMPLES_MS {times:?}");
    times.sort_unstable();
    let p95 = times[18];
    eprintln!(
        "CI_SCALE_SYNTHETIC cross-industry 100000 rows cold_ms={cold} warm_p95_ms={p95}; includes DB reads and title enrichment; excludes HTTP/browser/LLM"
    );
    std::fs::write(
        "/tmp/ci-scale-plan.json",
        serde_json::to_string_pretty(&explain_scope(&db, &q).await.unwrap()).unwrap(),
    )
    .unwrap();
    let voices = ResearchScope {
        view: Some("voices".into()),
        ..q.clone()
    };
    let now = std::time::Instant::now();
    let page = read(&db, &voices).await.unwrap();
    let voices_cold = now.elapsed().as_millis();
    assert_eq!(page["page"]["total"], 100000);
    assert_eq!(
        page["scope"]["resultRevision"],
        first["scope"]["resultRevision"]
    );
    assert!(page["trend"].is_null());
    let mut voice_times = vec![];
    for _ in 0..20 {
        let now = std::time::Instant::now();
        read(&db, &voices).await.unwrap();
        voice_times.push(now.elapsed().as_millis());
    }
    eprintln!("CI_SCALE_VOICES_SAMPLES_MS {voice_times:?}");
    voice_times.sort_unstable();
    eprintln!(
        "CI_SCALE_VOICES SYNTHETIC cross-industry 100000 rows cold_ms={voices_cold} warm_p95_ms={}; excludes HTTP/browser/LLM",
        voice_times[18]
    );
    std::fs::write(
        "/tmp/ci-voices-scale-plan.json",
        serde_json::to_string_pretty(&explain_scope(&db, &voices).await.unwrap()).unwrap(),
    )
    .unwrap();
    assert!(
        voice_times[18] < 500,
        "voices P95 exceeds contract: {}ms",
        voice_times[18]
    );
    assert!(p95 < 1000, "overview P95 exceeds contract: {p95}ms");
}

async fn seed_current_query_qualification(db: &linggan_storage_postgres::Database) {
    // This SQL fixture tests the reader's nonexclusive labels, not extraction compatibility.
    // Since CI-AUTO-004 the reader requires an explicit current qualification; a same-body
    // historical result alone must not become current coverage. Production builds this locally.
    sqlx::query(r#"INSERT INTO linggan_comment_research_eligibility_current
      (source_identity,source_ref,source_sha256,fingerprint,rule_revision_ref,rule_hash,schema_version,
       selector_version,source_context_revision,current_analysis_ref,last_accepted_analysis_ref,
       result_state,execution_state,reason_code,eligible_to_dispatch,input_manifest)
      SELECT encode(sha256(convert_to(source.work_ref::text||':'||source.comment_external_id,'UTF8')),'hex'),
       source.source_ref,source.source_sha256,
       encode(sha256(convert_to('SYNTHETIC-reader-fixture:'||source.source_ref::text,'UTF8')),'hex'),
       rule.rule_revision_ref,rule.canonical_hash,rule.schema_version,rule.selector_version,
       linggan_comment_research_context_revision(source.source_ref),analysis.work_ref,analysis.work_ref,
       'studied','idle','current_result',false,'{"isSyntheticReadFixture":true}'::jsonb
      FROM linggan_ci_source source
      JOIN linggan_comment_analysis_work analysis ON analysis.source_ref=source.source_ref AND analysis.state='succeeded'
      CROSS JOIN linggan_comment_research_rule_active active
      JOIN linggan_comment_research_rule_revision rule USING(rule_revision_ref)
      WHERE source.domain_ref=$1 AND analysis.rule_version=rule.schema_version"#)
      .bind(own()).execute(db.pool()).await.unwrap();
}
