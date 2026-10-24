//! Canonical preparation and live saved-source readers against an owned PostgreSQL.
//! Accepted decisions below are explicit SYNTHETIC fixtures, never provider output.
//! No retired scheduler, model secret, adapter process, or real model is invoked.
#[path = "../../../../evidence/tests/support/research_dossier_fixture.rs"]
pub(super) mod dossier_fixture;
use crate::topic_map_research as research;
use crate::{
    topic_map::{self, TopicMapCommand},
    topic_map_research_analysis as analysis,
};
pub(super) use dossier_fixture::fixture;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub(super) const D: Uuid = dossier_fixture::DOMAIN;

pub(super) async fn setup(name: &str) -> (Database, Uuid) {
    let db = fixture::proof_database(name).await;
    super::signal_support::install_schema(&db).await;
    let (connection, version, model, config) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'SYNTHETIC reader fixture','openai-completions','http://127.0.0.1:1',true,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-saved-source','manual')")
        .bind(model).bind(version).execute(db.pool()).await.unwrap();
    let capability = seed_capacity(&db, model).await;
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,$3,65536,1000,30,1)")
        .bind(config).bind(model).bind(capability).execute(db.pool()).await.unwrap();
    (db, config)
}
pub(super) async fn seed_capacity(db: &Database, model: Uuid) -> Uuid {
    let capability = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode) SELECT $1,identity.identity_ref,endpoint.endpoint_ref,'manual','SYNTHETIC saved-reader fixture, not provider qualification',131072,8192,60,'[\"off\"]','{}','{}','prompt_only' FROM linggan_model_entry model JOIN linggan_model_connection_version connection ON connection.version_ref=model.connection_version_ref JOIN linggan_ai_model_identity identity ON identity.connection_ref=connection.connection_ref AND identity.model_id=model.model_id JOIN linggan_ai_connection_endpoint endpoint ON endpoint.version_ref=connection.version_ref WHERE model.model_ref=$2")
        .bind(capability).bind(model).execute(db.pool()).await.unwrap();
    capability
}
pub(super) async fn synthetic_citation_config(
    db: &Database,
    original: Uuid,
    selector: &str,
) -> Uuid {
    let (model, config) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) SELECT $1,m.connection_version_ref,$3,'manual' FROM linggan_model_entry m JOIN linggan_model_config c USING(model_ref) WHERE c.config_ref=$2")
        .bind(model).bind(original).bind(selector).execute(db.pool()).await.unwrap();
    let capability = seed_capacity(db, model).await;
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,$2,$4,input_token_limit,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$3")
        .bind(config).bind(model).bind(original).bind(capability).execute(db.pool()).await.unwrap();
    config
}
pub(super) async fn tail_fixture_config(db: &Database, original: Uuid) -> Uuid {
    synthetic_citation_config(db, original, "synthetic-topic-map-cite-tail").await
}
pub(super) async fn work(db: &Database, id: &str, body: &str) -> Uuid {
    work_at(db, id, "", body, 10, "2026-08-28T10:00:00Z").await
}
pub(super) async fn work_at(
    db: &Database,
    id: &str,
    title: &str,
    body: &str,
    likes: i64,
    observed: &str,
) -> Uuid {
    let package=fixture::submit_package_at(db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":title,"bodyText":body,"authorId":"synthetic-creator","likes":likes}}),observed).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4) ON CONFLICT DO NOTHING")
        .bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    work
}

pub(super) async fn projected_research(db: &Database, work: Uuid) -> Value {
    let snapshot = topic_map::read_topic_map(
        db,
        &topic_map::TopicMapQuery {
            domain_ref: Some(D),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    snapshot
        .works
        .into_iter()
        .find(|candidate| candidate.work_ref == work)
        .expect("work remains in the current domain")
        .research
        .expect("qualified frozen research remains projected")
}
pub(super) mod comments {
    use super::*;
    async fn insert(
        db: &Database,
        note: &str,
        id: &str,
        parent: Option<&str>,
        body: &str,
        author: Option<&str>,
        observed: &str,
    ) -> Uuid {
        let kind = if parent.is_some() { "reply" } else { "comment" };
        let capability = if parent.is_some() {
            "replies"
        } else {
            "comments"
        };
        let mut payload = json!({"commentId":id,"noteId":note,"text":body});
        if let Some(parent) = parent {
            payload["rootCommentId"] = json!(parent);
            payload["parentCommentId"] = json!(parent);
        }
        if let Some(author) = author {
            payload["authorId"] = json!(author);
        }
        let package=fixture::submit_package_at(db,capability,json!({"contentExternalId":note}),json!({"kind":kind,"sourceObject":{"platform":"xhs","type":"content","externalId":note},"payload":payload}),observed).await;
        sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1")
            .bind(package)
            .fetch_one(db.pool())
            .await
            .unwrap()
    }
    pub(crate) async fn comment_with_author(
        db: &Database,
        note: &str,
        id: &str,
        body: &str,
        author: Option<&str>,
        observed: &str,
    ) -> Uuid {
        insert(db, note, id, None, body, author, observed).await
    }
    pub(crate) async fn reply_with_author(
        db: &Database,
        note: &str,
        id: &str,
        parent: &str,
        body: &str,
        author: Option<&str>,
        observed: &str,
    ) -> Uuid {
        insert(db, note, id, Some(parent), body, author, observed).await
    }
}

pub(super) async fn seed_research_inputs(db: &Database, config: Uuid, works: &[Uuid]) {
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    for command in [
        research::ResearchCommand::Configure {
            request_ref: Uuid::new_v4(),
            domain_ref: D,
            model_config_ref: config,
            daily_token_limit: 100000,
            run_token_limit: 100000,
            automatic_enabled: false,
            collection_enabled: false,
        },
        research::ResearchCommand::Start {
            request_ref: Uuid::new_v4(),
            domain_ref: D,
            trigger: "on_demand".into(),
            work_refs: works.to_vec(),
            topic_ref: None,
        },
    ] {
        assert_eq!(
            research::apply_research_command(db, &command)
                .await
                .unwrap_err()
                .code(),
            "ai_legacy_execution_retired"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        before
    );
    let inputs = research::load_inputs_for_works(db, D, config, works)
        .await
        .unwrap();
    assert_eq!(inputs.len(), works.len());
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_map_research_run(run_ref,domain_ref,request_ref,trigger,config_ref,method_version,token_limit,state,last_reason) VALUES($1,$2,$3,'on_demand',$4,$5,100000,'completed','SYNTHETIC accepted-record fixture; no execution')")
        .bind(run).bind(D).bind(Uuid::new_v4()).bind(config).bind(analysis::METHOD_VERSION).execute(db.pool()).await.unwrap();
    for input in inputs {
        for window in research::research_windows(&input) {
            let manifest = research::reference_manifest(&window);
            assert_eq!(manifest["inputContract"], research::INPUT_CONTRACT);
            for fragment in manifest["fragments"].as_array().unwrap() {
                assert!(fragment.get("text").is_none());
                assert!(fragment["asOf"].as_str().is_some());
                assert!(fragment["blockRef"].as_str().is_some());
            }
            sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,state,last_reason) VALUES($1,$2,$3,$4,$5,$6,'stopped','SYNTHETIC accepted-record fixture; no dispatch')")
                .bind(Uuid::new_v4()).bind(run).bind(D).bind(input.work.work_ref).bind(&window.hash).bind(manifest).execute(db.pool()).await.unwrap();
        }
    }
}

pub(super) async fn seed_all_accepted(db: &Database) {
    let tasks:Vec<Uuid>=sqlx::query_scalar("SELECT task_ref FROM linggan_topic_map_research_task WHERE state='stopped' ORDER BY created_at,task_ref")
        .fetch_all(db.pool()).await.unwrap();
    for task in tasks {
        seed_accepted_source_task(db, task).await;
    }
}

/// Test-only construction of an already accepted decision. Production scheduling
/// remains retired; this fixture proves input identity, persistence, and readers.
pub(super) async fn seed_accepted_source_task(db: &Database, task: Uuid) {
    let row=sqlx::query("SELECT task.work_public_ref,task.input_refs,run.run_ref,run.config_ref,model.model_id FROM linggan_topic_map_research_task task JOIN linggan_topic_map_research_run run USING(run_ref) JOIN linggan_model_config config USING(config_ref) JOIN linggan_model_entry model USING(model_ref) WHERE task.task_ref=$1")
        .bind(task).fetch_one(db.pool()).await.unwrap();
    let work: Uuid = row.get("work_public_ref");
    let config: Uuid = row.get("config_ref");
    let mode: String = row.get("model_id");
    let manifest: Value = row.get("input_refs");
    let inputs = research::load_inputs_for_works_at(
        db,
        D,
        config,
        &[work],
        research::manifest_as_of(&manifest).unwrap(),
    )
    .await
    .unwrap();
    let input = research::restore_scoped_window(&inputs, work, &manifest)
        .expect("current canonical preparation must restore exact frozen fixture");
    let primary = if mode == "synthetic-topic-map-cite-linked-reply" {
        let child = input
            .comment_study
            .as_array()
            .unwrap()
            .iter()
            .find(|c| {
                c["parentFragmentIds"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty())
            })
            .unwrap();
        input
            .fragments
            .iter()
            .find(|f| Some(f.fragment_id.as_str()) == child["fragmentId"].as_str())
            .unwrap()
    } else if mode == "synthetic-topic-map-cite-tail" {
        input
            .fragments
            .iter()
            .filter(|f| research::is_research_evidence(f))
            .max_by_key(|f| f.end)
            .unwrap()
    } else {
        input
            .fragments
            .iter()
            .find(|f| research::is_research_evidence(f))
            .unwrap()
    };
    let end = primary.end;
    let start = if mode == "synthetic-topic-map-cite-tail" {
        end.saturating_sub(1000).max(primary.start)
    } else {
        primary.start
    };
    let citation = analysis::Citation {
        fragment_id: primary.fragment_id.clone(),
        start,
        end,
    };
    let sustain = primary.text.contains("SUSTAIN_ONLY");
    let definition = if sustain {
        "练习已经开始以后维持注意与持续执行的困难及支持"
    } else {
        "安排并开始一次具体练习或实践的行动、困难与支持方式"
    };
    let label = if sustain {
        "合成持续练习"
    } else {
        "合成实践启动"
    };
    let output:analysis::ResearchOutput=serde_json::from_value(json!({
        "contract":analysis::EXTRACT_CONTRACT,"outcome":"analyzed",
        "discussions":[{"label":label,"topicRef":null,"evidence":[citation],"statement":"SYNTHETIC：此单元仅验证已冻结引文的保存和重开", "definition":definition,"inclusionCriteria":["合成材料明确包含本单元实践问题"],"exclusionCriteria":["没有对应实践问题的其他材料"],"speakerRole":analysis::fragment_role(primary),"evidenceRole":"support","rationale":"SYNTHETIC / NOT EVIDENCE"}],
        "scenes":[],"journey":{"mainStage":"unclear","involvedStages":[],"overlays":[],"path":"unknown","rationale":"SYNTHETIC / NOT EVIDENCE","evidence":[]},"responseMatches":[],
        "angles":[{"label":"合成保存来源","title":"合成完整来源角度","answerTask":"验证冻结来源可重开","evidence":[citation]}],"productOpportunities":[],"limitations":["SYNTHETIC accepted-record fixture, not production model execution"]
    })).unwrap();
    analysis::validate_output(&output, &input.fragments, &[]).unwrap();
    analysis::validate_context_links(&output, &input.fragments, &input.comment_study).unwrap();
    let units = crate::topic_map_core::units(&input, &output);
    let unavailable = crate::topic_map_core::unavailable_definitions(db, Some(D))
        .await
        .unwrap();
    let candidates:Vec<Value>=sqlx::query("SELECT topic_ref,definition_ref,display_name,definition_text FROM linggan_topic_definition definition JOIN linggan_topic_map_concept_rule rule USING(definition_ref) WHERE rule.domain_ref=$1 ORDER BY definition.created_at,definition.definition_ref")
        .bind(D).fetch_all(db.pool()).await.unwrap().into_iter()
        .filter(|row|!unavailable.contains(&row.get::<Uuid,_>("definition_ref")))
        .map(|row|json!({"topicRef":row.get::<Uuid,_>("topic_ref"),"definitionRef":row.get::<Uuid,_>("definition_ref"),"label":row.get::<String,_>("display_name"),"definition":row.get::<String,_>("definition_text")})).collect();
    let matched = candidates
        .iter()
        .find(|candidate| candidate["definition"] == definition);
    let resolution:analysis::ResolutionOutput=serde_json::from_value(json!({"contract":analysis::RESOLVE_CONTRACT,"decisions":[{
        "unitId":units[0].0,"status":if matched.is_some(){"matched"}else{"new"},
        "matches":matched.map(|c|vec![json!({"topicRef":c["topicRef"],"definitionRef":c["definitionRef"],"reason":"SYNTHETIC exact fixture boundary"})]).unwrap_or_default(),
        "proposedTopic":if matched.is_some(){Value::Null}else{json!({"label":label,"definition":definition,"inclusionCriteria":["合成材料明确包含本单元实践问题"],"exclusionCriteria":["没有对应实践问题的其他材料"],"domainFit":"in_scope","domainReason":"SYNTHETIC / NOT EVIDENCE","abstractionReason":"合成数据库引用边界测试","parent":{"kind":"root","topicRef":null,"definitionRef":null,"proposal":null,"reason":"没有合成父方向"}})},
        "relations":if matched.is_some(){vec![]}else{candidates.iter().map(|c|json!({"topicRef":c["topicRef"],"definitionRef":c["definitionRef"],"relation":"distinct","reason":"SYNTHETIC separate fixture boundary"})).collect::<Vec<_>>()},
        "reason":"SYNTHETIC accepted-record fixture"}]})).unwrap();
    let candidates = json!(candidates);
    analysis::validate_resolution(&resolution, &units, &candidates).unwrap();
    let invocation = Uuid::new_v4();
    let request_hash =
        linggan_evidence::creator_discovery::hash(&format!("synthetic:{invocation}"));
    let verification = json!({"contract":analysis::verification::VERIFY_CONTRACT,"passed":true,"fixture":"SYNTHETIC acceptance seed, not semantic review"});
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,model.connection_version_ref,model.model_ref,$2,'analyze',$3,'succeeded',0,0,$4,scope_001_now() FROM linggan_model_config config JOIN linggan_model_entry model USING(model_ref) WHERE config.config_ref=$2")
        .bind(invocation).bind(config).bind(&request_hash).bind(json!({"synthetic":true,"fixture":"SYNTHETIC / NOT EVIDENCE; no provider call"})).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,deadline_at,phase) VALUES($1,$2,$3,1,$4,$5,current_date,scope_001_now(),'resolve')")
        .bind(invocation).bind(task).bind(row.get::<Uuid,_>("run_ref")).bind(&request_hash).bind(json!({"source":manifest,"topics":candidates,"synthetic":true})).execute(&mut *tx).await.unwrap();
    let eligible: Vec<Uuid> = candidates
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["definitionRef"].as_str().unwrap().parse().unwrap())
        .collect();
    let accepted = crate::topic_map_core::accept_in(
        &mut tx,
        crate::topic_map_core::Acceptance {
            input: &input,
            task,
            invocation,
            units: &units,
            resolution: &resolution,
            candidates: &candidates,
            recall: &json!({"synthetic":true}),
            semantic_verification: &verification,
            eligible_definitions: &eligible,
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_result(result_ref,domain_ref,work_public_ref,input_hash,output_json,method_version,invocation_ref,core_json) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(Uuid::new_v4()).bind(D).bind(work).bind(&input.hash).bind(json!(output)).bind(analysis::METHOD_VERSION).bind(invocation).bind(json!({"methodVersion":crate::topic_map_core::METHOD_VERSION,"coverage":input.coverage,"units":accepted})).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_task SET state='succeeded',phase='resolve',last_reason='SYNTHETIC accepted record; no execution' WHERE task_ref=$1")
        .bind(task).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

pub(super) async fn source_task(
    db: &Database,
    work: Uuid,
    config: Uuid,
    field: &str,
    source: Option<Uuid>,
    start: usize,
) -> Uuid {
    sqlx::query_scalar("SELECT t.task_ref FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run run USING(run_ref) WHERE t.work_public_ref=$1 AND run.config_ref=$2 AND EXISTS(SELECT 1 FROM jsonb_array_elements(t.input_refs->'fragments') f WHERE f->>'field'=$3 AND ($4::uuid IS NULL OR f->>'sourceRef'=$4::text) AND (f->>'start')::bigint<=$5 AND (f->>'end')::bigint>$5) ORDER BY t.created_at,t.task_ref LIMIT 1")
        .bind(work).bind(config).bind(field).bind(source).bind(start as i64).fetch_one(db.pool()).await.unwrap()
}

pub(super) async fn task_angle_command(
    db: &Database,
    task: Uuid,
    work: Uuid,
    key: &str,
) -> TopicMapCommand {
    let row = sqlx::query("SELECT result.result_ref,result.method_version,result.core_json FROM linggan_topic_map_research_result result JOIN linggan_topic_map_research_request request USING(invocation_ref) WHERE request.task_ref=$1 ORDER BY result.created_at DESC LIMIT 1")
        .bind(task).fetch_one(db.pool()).await.unwrap();
    let core: Value = row.get("core_json");
    let assignment = &core["units"][0]["assignments"][0];
    TopicMapCommand::SaveAlternative {
        idempotency_key: key.into(),
        domain_ref: D,
        topic_ref: Some(assignment["topicRef"].as_str().unwrap().parse().unwrap()),
        definition_ref: Some(
            assignment["definitionRef"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
        ),
        title: "合成完整来源角度".into(),
        angle: "引用已有合格原文的合成测试".into(),
        rationale: "SYNTHETIC / NOT EVIDENCE".into(),
        evidence_work_refs: vec![work],
        method_version: row.get("method_version"),
        research_result_ref: Some(row.get("result_ref")),
        research_angle_index: Some(0),
        research_opportunity_index: None,
    }
}

pub(super) async fn save_task_angle(db: &Database, task: Uuid, work: Uuid, key: &str) -> Uuid {
    topic_map::save_topic_map_command(db, &task_angle_command(db, task, work, key).await)
        .await
        .unwrap()
        .subject_ref
        .unwrap()
}

pub(super) async fn assert_tail_angle_citation(
    db: &Database,
    task: Uuid,
    field: &str,
    source: &str,
    emoji: &str,
) {
    let row = sqlx::query("SELECT result.output_json,task.input_refs FROM linggan_topic_map_research_result result JOIN linggan_topic_map_research_request request USING(invocation_ref) JOIN linggan_topic_map_research_task task USING(task_ref) WHERE request.task_ref=$1 ORDER BY result.created_at DESC LIMIT 1")
        .bind(task).fetch_one(db.pool()).await.unwrap();
    let output: Value = row.get("output_json");
    let manifest: Value = row.get("input_refs");
    let input = &manifest;
    assert_eq!(input["inputContract"], research::INPUT_CONTRACT);
    let tail = input["fragments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fragment| {
            fragment["field"] == field
                && fragment["start"].as_u64().unwrap() <= 12000
                && fragment["end"].as_u64().unwrap() > 12000
        })
        .expect("requested tail fragment must be frozen in the accepted input");
    let end = source.chars().count();
    assert_eq!(tail["end"], json!(end));
    let citations = output["angles"][0]["evidence"].as_array().unwrap();
    assert_eq!(citations.len(), 1, "fixture must explicitly cite its tail");
    assert_eq!(citations[0]["fragmentId"], tail["fragmentId"]);
    let window_start = end
        .saturating_sub(1000)
        .max(tail["start"].as_u64().unwrap() as usize);
    assert_eq!(citations[0]["start"], json!(window_start));
    assert_eq!(citations[0]["end"], json!(end));
    let emoji_byte = source.find(emoji).expect("fixture tail contains its emoji");
    let emoji_start = source[..emoji_byte].chars().count();
    let emoji_end = emoji_start + emoji.chars().count();
    assert!(window_start <= emoji_start && emoji_end <= end);
    let quoted: String = source
        .chars()
        .skip(window_start)
        .take(end - window_start)
        .collect();
    assert!(
        quoted.contains(emoji),
        "actual cited scalar range covers the emoji"
    );
}

pub(super) async fn reference_only_work(db: &Database, id: &str) -> Uuid {
    let package = fixture::submit_package(db,"content_detail",json!({"contentExternalId":id}),
        json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},
            "payload":{"title":"","bodyText":"","authorId":"synthetic-creator"}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'reference','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    work
}
