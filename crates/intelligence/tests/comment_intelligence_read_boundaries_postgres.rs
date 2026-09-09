//! Synthetic read-boundary regressions. No provider, shared database, or media processor is called.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_intelligence::{ResearchScope, read},
    comment_research::comment_source_hash,
};
use linggan_storage_postgres::Database;
use research_fixture::{comment, detail};
use serde_json::{Value, json};
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
fn result(source: Uuid, body: &str, label: &str, context: Value) -> Value {
    let evidence = json!([{"sourceRef":source,"startChar":0,"endChar":body.chars().count()}]);
    json!({"sourceRef":source,"sourceSha256":comment_source_hash(body),"spans":[],"contextRefs":context,
        "semantic":{"outcome":"interpretable","labels":[{"label":label,"evidence":evidence}],"problems":[],"stances":[],"contextMissing":[],"uncertaintyReason":null}})
}
async fn analysis(db: &Database, source: Uuid, value: Value, version: &str, at: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result,created_at) SELECT $1,$2,rule.schema_version,$3,'succeeded',$4,$5::timestamptz FROM linggan_comment_research_rule_active active JOIN linggan_comment_research_rule_revision rule USING(rule_revision_ref)")
        .bind(id).bind(source).bind(version).bind(value).bind(at).execute(db.pool()).await.unwrap();
    qualify_current_read_fixture(db, source, id).await;
    id
}
// Explicit synthetic reader input, not a production extraction/reuse pipeline proof.
// Historical analysis alone has no current qualification. The test supplies a current
// rule/context decision only for the same readable identity and exact current body.
async fn qualify_current_read_fixture(db: &Database, source: Uuid, analysis: Uuid) {
    let written = sqlx::query(r#"INSERT INTO linggan_comment_research_eligibility_current
      (source_identity,source_ref,source_sha256,fingerprint,rule_revision_ref,rule_hash,schema_version,
       selector_version,source_context_revision,current_analysis_ref,last_accepted_analysis_ref,
       result_state,execution_state,reason_code,eligible_to_dispatch,input_manifest)
      SELECT encode(sha256(convert_to(current.work_ref::text||':'||current.comment_external_id,'UTF8')),'hex'),
       current.source_ref,current.source_sha256,
       encode(sha256(convert_to('SYNTHETIC-boundary-reader:'||current.source_ref::text,'UTF8')),'hex'),
       rule.rule_revision_ref,rule.canonical_hash,rule.schema_version,rule.selector_version,
       linggan_comment_research_context_revision(current.source_ref),analysis.work_ref,analysis.work_ref,
       'studied','idle','current_result',false,'{"isSyntheticReadFixture":true}'::jsonb
      FROM linggan_ci_source current
      JOIN linggan_comment_research_readable readable ON readable.material_ref=current.source_ref
      JOIN linggan_comment_analysis_work analysis ON analysis.work_ref=$3 AND analysis.state='succeeded'
      JOIN linggan_material_comment original ON original.material_ref=analysis.source_ref
       AND original.content_public_ref=current.work_ref AND original.comment_external_id=current.comment_external_id
      CROSS JOIN linggan_comment_research_rule_active active
      JOIN linggan_comment_research_rule_revision rule USING(rule_revision_ref)
      WHERE current.domain_ref=$1 AND current.source_ref=$2 AND analysis.rule_version=rule.schema_version
       AND analysis.result->>'sourceSha256'=current.source_sha256
       AND linggan_ci_analysis_context_readable(analysis.result)
      ON CONFLICT(source_identity) DO UPDATE SET source_ref=EXCLUDED.source_ref,source_sha256=EXCLUDED.source_sha256,
       fingerprint=EXCLUDED.fingerprint,rule_revision_ref=EXCLUDED.rule_revision_ref,rule_hash=EXCLUDED.rule_hash,
       schema_version=EXCLUDED.schema_version,selector_version=EXCLUDED.selector_version,
       source_context_revision=EXCLUDED.source_context_revision,current_analysis_ref=EXCLUDED.current_analysis_ref,
       last_accepted_analysis_ref=EXCLUDED.last_accepted_analysis_ref,result_state=EXCLUDED.result_state,
       execution_state=EXCLUDED.execution_state,reason_code=EXCLUDED.reason_code,
       eligible_to_dispatch=EXCLUDED.eligible_to_dispatch,input_manifest=EXCLUDED.input_manifest"#)
      .bind(own()).bind(source).bind(analysis).execute(db.pool()).await.unwrap();
    assert_eq!(
        written.rows_affected(),
        1,
        "fixture requires matching current source, body, rule and readable context"
    );
}

async fn config(db: &Database) -> Uuid {
    // Immutable synthetic configuration satisfies relational integrity without probing or publishing it.
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref) VALUES($1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'SYNTHETIC','openai-completions','http://127.0.0.1:1',true,$3)").bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'SYNTHETIC','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts,auto_source_limit,auto_token_limit) VALUES($1,$2,4096,1024,5,2,100,100000)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    config
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn restored_comment_text_reuses_qualified_old_analysis_instead_of_newer_other_text() {
    let db = fixture::proof_database("ci_read_restored_text").await;
    detail(&db, "restore", "SYNTHETIC 上下文").await;
    let a = "每天需要家长一直陪伴，想知道怎么办";
    let b = "试用了一个新的时间安排方法";
    let old_a = comment(&db, "restore", "same", a, "2026-08-28T10:00:00Z").await;
    let qualified_a = analysis(
        &db,
        old_a,
        result(old_a, a, "need", json!({"researchSourceRefs":[old_a]})),
        "SYNTHETIC",
        "2026-08-28T11:00:00Z",
    )
    .await;
    let middle_b = comment(&db, "restore", "same", b, "2026-08-29T10:00:00Z").await;
    analysis(
        &db,
        middle_b,
        result(
            middle_b,
            b,
            "solution",
            json!({"researchSourceRefs":[middle_b]}),
        ),
        "SYNTHETIC",
        "2026-08-29T11:00:00Z",
    )
    .await;
    let restored = comment(&db, "restore", "same", a, "2026-08-30T10:00:00Z").await;
    qualify_current_read_fixture(&db, restored, qualified_a).await;
    let r = read(&db, &all()).await.unwrap();
    assert_eq!(r["summary"]["comments"], 1);
    assert_eq!(r["summary"]["analyzed"], 1);
    assert_eq!(r["page"]["items"][0]["sourceRef"], json!(restored));
    assert_eq!(r["page"]["items"][0]["labels"], json!(["need"]));
    assert_eq!(r["summary"]["lenses"]["solution"], 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn withdrawing_legacy_parent_hides_derived_analysis_but_keeps_child_raw() {
    let db = fixture::proof_database("ci_read_parent_withdraw").await;
    detail(&db, "legacy-parent", "SYNTHETIC 上下文").await;
    let parent = comment(
        &db,
        "legacy-parent",
        "parent",
        "每天安排作业都很费劲",
        "2026-08-28T10:00:00Z",
    )
    .await;
    let body = "我家也是这样，想知道怎么办";
    let child = comment(&db, "legacy-parent", "child", body, "2026-08-28T10:01:00Z").await;
    analysis(
        &db,
        child,
        result(child, body, "need", json!({"parentSourceRef":parent})),
        "legacy-SYNTHETIC",
        "2026-08-28T11:00:00Z",
    )
    .await;
    assert_eq!(read(&db, &all()).await.unwrap()["summary"]["analyzed"], 1);
    linggan_evidence::comment_research_read::restrict_comment_research_source(
        &db,
        parent,
        "SYNTHETIC 撤回父评论",
    )
    .await
    .unwrap();
    let r = read(&db, &all()).await.unwrap();
    assert_eq!(r["summary"]["comments"], 1);
    assert_eq!(r["summary"]["analyzed"], 0);
    assert_eq!(r["page"]["items"][0]["body"], body);
    assert_eq!(r["page"]["items"][0]["labels"], json!([]));
    assert_eq!(
        read(
            &db,
            &ResearchScope {
                lenses: Some("need".into()),
                ..all()
            }
        )
        .await
        .unwrap()["page"]["total"],
        0
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn failed_daily_item_is_visible_as_failed_and_not_pending() {
    let db = fixture::proof_database("ci_read_failed").await;
    let config = config(&db).await;
    detail(&db, "failed", "SYNTHETIC 上下文").await;
    let source = comment(
        &db,
        "failed",
        "failed",
        "我想了解日常作业安排",
        "2026-08-28T10:00:00Z",
    )
    .await;
    let batch = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request) VALUES($1,'selected',$2,'2026-08-28','2026-08-29',1,4096,'{}')").bind(batch).bind(config).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_daily_item(batch_ref,source_ref,state,attempts,failure_code) VALUES($1,$2,'failed',1,'provider_timeout')").bind(batch).bind(source).execute(db.pool()).await.unwrap();
    let r = read(&db, &all()).await.unwrap();
    assert_eq!(r["page"]["items"][0]["analysisState"], "failed");
    assert_eq!(r["page"]["items"][0]["executionState"], "failed");
    assert_eq!(r["page"]["items"][0]["reasonCode"], "provider_timeout");
    assert_eq!(r["page"]["items"][0]["failureCode"], "provider_timeout");
    assert_eq!(r["summary"]["analyzed"], 0);
    assert_eq!(r["daily"]["items"][0]["counts"]["failed"], 1);
    let other = read(
        &db,
        &ResearchScope {
            domain: Some(Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap()),
            ..all()
        },
    )
    .await
    .unwrap();
    assert!(other["daily"]["items"].as_array().unwrap().is_empty());
    assert_eq!(other["daily"]["schedule"]["enabled"], false);
    let excluded = read(
        &db,
        &ResearchScope {
            text: Some("不匹配的合成筛选".into()),
            ..all()
        },
    )
    .await
    .unwrap();
    assert!(excluded["daily"]["items"].as_array().unwrap().is_empty());
    assert_eq!(
        read(
            &db,
            &ResearchScope {
                processing_state: Some("failed".into()),
                ..all()
            }
        )
        .await
        .unwrap()["page"]["total"],
        1
    );
    assert_eq!(
        read(
            &db,
            &ResearchScope {
                processing_state: Some("pending".into()),
                ..all()
            }
        )
        .await
        .unwrap()["page"]["total"],
        0
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; synthetic admitted media metadata only"]
async fn withdrawing_acquired_ocr_removes_derived_lens_from_aggregate() {
    let db = fixture::proof_database("ci_read_media_withdraw").await;
    let note = "media-boundary";
    detail(&db, note, "SYNTHETIC 媒体上下文").await;
    let body = "图里的方法我尝试过，想知道如何坚持";
    let source = comment(&db, note, "media-comment", body, "2026-08-28T10:00:00Z").await;
    let observation = Uuid::new_v4();
    let slot = format!("xhs:{note}:image:1");
    fixture::submit_package(&db,"media_slots",json!({"contentExternalId":note}),json!({
        "kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"image","ordinal":1},
        "observation":{"externalUri":"https://media.example/SYNTHETIC","candidateUris":["https://media.example/SYNTHETIC"],"observedAt":"2026-08-28T10:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":note}})).await;
    linggan_evidence::admit_media_blob(
        &db,
        observation,
        "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
        "image/jpeg",
        12,
        "synthetic/media-read-boundary",
    )
    .await
    .unwrap();
    let job:Uuid=sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE slot_key=$1 AND processor_kind='image_ocr'").bind(&slot).fetch_one(db.pool()).await.unwrap();
    let derivative = linggan_evidence::record_media_derivative_completion(
        &db,
        job,
        "ocr_text",
        "1320b046a60f7c39a3480dea50b655ca92ce61db269ea07e4037e7a6f0788e5a",
        12,
        None,
    )
    .await
    .unwrap();
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text) VALUES($1,$2,'ocr_text','SYNTHETIC OCR','SYNTHETIC OCR')").bind(derivative).bind(work).execute(db.pool()).await.unwrap();
    let value = result(
        source,
        body,
        "need",
        json!({"researchSourceRefs":[source],"workRef":work,"mediaJobs":[job]}),
    );
    analysis(
        &db,
        source,
        value.clone(),
        "SYNTHETIC",
        "2026-08-28T11:00:00Z",
    )
    .await;
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT linggan_ci_analysis_context_readable($1)")
            .bind(&value)
            .fetch_one(db.pool())
            .await
            .unwrap()
    );
    assert_eq!(
        read(&db, &all()).await.unwrap()["summary"]["lenses"]["need"],
        1
    );
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,derivative_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','SYNTHETIC','SYNTHETIC OCR撤回',scope_001_now())").bind(Uuid::new_v4()).bind(derivative).execute(db.pool()).await.unwrap();
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT linggan_ci_analysis_context_readable($1)")
            .bind(&value)
            .fetch_one(db.pool())
            .await
            .unwrap()
    );
    let r = read(&db, &all()).await.unwrap();
    assert_eq!(r["summary"]["comments"], 1);
    assert_eq!(r["summary"]["analyzed"], 0);
    assert_eq!(r["page"]["items"][0]["labels"], json!([]));
    assert_eq!(
        read(
            &db,
            &ResearchScope {
                lenses: Some("need".into()),
                ..all()
            }
        )
        .await
        .unwrap()["page"]["total"],
        0
    );
}

async fn clock(db: &Database, at: &str) {
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS ci_boundary_clock(now_at timestamptz NOT NULL); CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT COALESCE((SELECT now_at FROM ci_boundary_clock LIMIT 1),clock_timestamp()) $$").execute(db.pool()).await.unwrap();
    sqlx::query("DELETE FROM ci_boundary_clock")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO ci_boundary_clock VALUES($1::timestamptz)")
        .bind(at)
        .execute(db.pool())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn comparison_counts_config_versions_not_individual_semantic_work_ids() {
    let db = fixture::proof_database("ci_read_model_version").await;
    clock(&db, "2026-08-28T10:00:00Z").await;
    detail(&db, "versions", "SYNTHETIC 版本口径").await;
    let stable = linggan_intelligence::comment_daily::version(Uuid::new_v4());
    let problem = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ci_problem(problem_ref,domain_ref,name,meaning,origin) VALUES($1,$2,'SYNTHETIC 问题','SYNTHETIC 版本测试中的同一问题','exact_definition')").bind(problem).bind(own()).execute(db.pool()).await.unwrap();
    for (period, at) in [
        ("previous", "2026-08-29T10:00:00Z"),
        ("current", "2026-09-03T10:00:00Z"),
    ] {
        clock(&db, at).await;
        for n in 0..3 {
            let body = format!("日常作业安排中家长需要持续陪伴 {period} {n}");
            let source = comment(&db, "versions", &format!("{period}-{n}"), &body, at).await;
            let a = analysis(
                &db,
                source,
                result(
                    source,
                    &body,
                    "need",
                    json!({"researchSourceRefs":[source]}),
                ),
                &format!("{stable}:{}", Uuid::new_v4()),
                at,
            )
            .await;
            sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,analysis_ref) VALUES($1,$2,'exact_definition',$3)").bind(problem).bind(source).bind(a).execute(db.pool()).await.unwrap();
        }
    }
    clock(&db, "2026-09-08T10:00:00Z").await;
    let r = read(&db, &all()).await.unwrap();
    let c = &r["comparisons"][0];
    assert_eq!(r["comparisons"].as_array().unwrap().len(), 1);
    assert_eq!(c["previousWindow"]["modelVersions"], 1);
    assert_eq!(c["currentWindow"]["modelVersions"], 1);
    assert_eq!(c["previousWindow"]["modelSignature"], stable);
    assert_eq!(c["currentWindow"]["modelSignature"], stable);
    // Different configurations must stay different; stripping every version would pass the first half falsely.
    clock(&db, "2026-09-04T10:00:00Z").await;
    let body = "另一配置理解同一个家长持续陪伴问题";
    let source = comment(
        &db,
        "versions",
        "other-config",
        body,
        "2026-09-04T10:00:00Z",
    )
    .await;
    let other = linggan_intelligence::comment_daily::version(Uuid::new_v4());
    let a = analysis(
        &db,
        source,
        result(source, body, "need", json!({"researchSourceRefs":[source]})),
        &format!("{other}:{}", Uuid::new_v4()),
        "2026-09-04T11:00:00Z",
    )
    .await;
    sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,analysis_ref) VALUES($1,$2,'exact_definition',$3)").bind(problem).bind(source).bind(a).execute(db.pool()).await.unwrap();
    clock(&db, "2026-09-08T10:00:00Z").await;
    let r = read(&db, &all()).await.unwrap();
    assert_eq!(r["comparisons"][0]["currentWindow"]["modelVersions"], 2);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL migration proof"]
async fn upgrading_pauses_legacy_grants_and_old_endpoints_explain_retirement() {
    use linggan_intelligence::{model_plans::*, model_settings::ModelError};
    let db = fixture::proof_database_before_comment_intelligence("ci_legacy_retirement").await;
    let config = config(&db).await;
    let plan = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_plan(plan_ref,config_ref,kind,enabled,source_limit,token_limit,request) VALUES($1,$2,'automatic',true,10,10000,'{}')").bind(plan).bind(config).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_model_workspace SET active_auto_plan_ref=$1 WHERE singleton")
        .bind(plan)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0048_comment_intelligence.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    let enabled: bool =
        sqlx::query_scalar("SELECT enabled FROM linggan_model_plan WHERE plan_ref=$1")
            .bind(plan)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(!enabled);
    let active: Option<Uuid> = sqlx::query_scalar(
        "SELECT active_auto_plan_ref FROM linggan_model_workspace WHERE singleton",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(active.is_none());
    assert!(matches!(
        resume_model_plan(
            &db,
            plan,
            &ResumeModelPlan {
                expected_revision: 1
            }
        )
        .await,
        Err(ModelError::ResearchPlanRetired)
    ));
    assert!(matches!(
        start_model_plan(
            &db,
            &StartModelPlan {
                plan_ref: Uuid::new_v4(),
                config_ref: config,
                kind: "automatic".into(),
                source_refs: vec![],
                source_limit: 10,
                token_limit: 10000,
                expected_auto_plan_ref: None
            }
        )
        .await,
        Err(ModelError::ResearchPlanRetired)
    ));
    assert_eq!(
        linggan_intelligence::model_settings_read::read_model_settings(&db, true)
            .await
            .unwrap()["legacyPlansAvailable"],
        false
    );
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; deliberate synthetic UUID collisions"]
async fn cross_domain_uuid_collision_never_inherits_or_overwrites_native_research() {
    use linggan_intelligence::comment_intelligence::{source, source_with_scope};
    use linggan_intelligence::comment_intelligence_actions::{execute_action, source_research};
    let db = fixture::proof_database("ci_cross_uuid_collision").await;
    detail(&db, "collision", "SYNTHETIC native title").await;
    let body = "每天都需要陪伴，想知道怎样安排时间";
    let first = comment(&db, "collision", "same-id", body, "2026-08-28T10:00:00Z").await;
    let qualified_native = analysis(
        &db,
        first,
        result(first, body, "need", json!({"researchSourceRefs":[first]})),
        "SYNTHETIC",
        "2026-08-28T11:00:00Z",
    )
    .await;
    let asset = research_fixture::asset(first, body);
    linggan_intelligence::comment_research::save_comment_asset(&db, &asset)
        .await
        .unwrap();
    let problem = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ci_problem(problem_ref,domain_ref,name,meaning,origin) VALUES($1,$2,'SYNTHETIC native problem','SYNTHETIC native meaning','manual')")
        .bind(problem).bind(own()).execute(db.pool()).await.unwrap();
    execute_action(&db, json!({"commandRef":Uuid::new_v4(),"domain":own(),"kind":"correct","sourceRef":first,"expectedRevision":0,"payload":{"labels":["need"],"problemRefs":[problem],"needsContext":false},"reason":"SYNTHETIC native correction"})).await.unwrap();
    let latest = comment(&db, "collision", "same-id", body, "2026-08-29T10:00:00Z").await;
    qualify_current_read_fixture(&db, latest, qualified_native).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(first)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let external = Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
    // Every native identity join key and body hash collides, while domain ownership differs.
    sqlx::query("INSERT INTO cross_industry_sample(sample_ref,domain_ref,platform,content_external_id,title) VALUES($1,$2,'xhs','collision','SYNTHETIC external title')").bind(work).bind(external).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO cross_industry_comment(comment_ref,sample_ref,domain_ref,comment_external_id,body_text,body_state) VALUES($1,$2,$3,'same-id',$4,'KNOWN')").bind(first).bind(work).bind(external).bind(body).execute(db.pool()).await.unwrap();
    assert_eq!(read(&db, &all()).await.unwrap()["summary"]["analyzed"], 1);
    let native = source(&db, own(), first).await.unwrap();
    assert_eq!(native["source"]["bookmarked"], true);
    assert_eq!(native["source"]["labels"], json!(["need"]));
    assert_eq!(native["research"]["history"].as_array().unwrap().len(), 1);
    assert_eq!(
        native["research"]["memberships"].as_array().unwrap().len(),
        1
    );
    assert_eq!(native["bookmarks"].as_array().unwrap().len(), 1);
    assert!(!native["processing"].is_null());
    let external_q = ResearchScope {
        domain: Some(external),
        ..all()
    };
    let outside = source_with_scope(&db, &external_q, first).await.unwrap();
    assert_eq!(outside["source"]["body"], body);
    assert_eq!(outside["source"]["workTitle"], "SYNTHETIC external title");
    assert_eq!(outside["source"]["bookmarked"], false);
    assert_eq!(outside["source"]["labels"], json!([]));
    for key in ["history", "memberships", "problemRefs"] {
        assert_eq!(outside["research"][key], json!([]), "{key}");
    }
    assert_eq!(outside["annotations"], json!({"human":[],"analysis":[]}));
    assert_eq!(outside["bookmarks"], json!([]));
    assert_eq!(outside["processing"], Value::Null);
    assert_eq!(
        read(&db, &external_q).await.unwrap()["summary"]["analyzed"],
        0
    );
    assert!(
        source_with_scope(&db, &external_q, latest).await.is_err(),
        "native historical alias must not resolve an external comment"
    );
    assert!(source_research(&db, external, latest).await.is_err());
    let before = source_research(&db, own(), first).await.unwrap();
    let command = Uuid::new_v4();
    let error = execute_action(&db, json!({"commandRef":command,"domain":external,"kind":"bookmark","sourceRef":first,"expectedRevision":0,"payload":{"enabled":true}})).await.unwrap_err();
    assert!(
        matches!(error, linggan_storage_postgres::StorageError::Statement(sqlx::Error::Protocol(ref code)) if code == "CI_SOURCE_DOMAIN_COLLISION")
    );
    assert_eq!(source_research(&db, own(), first).await.unwrap(), before);
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM linggan_ci_command WHERE command_ref=$1)"
        )
        .bind(command)
        .fetch_one(db.pool())
        .await
        .unwrap()
    );
}
