//! Synthetic transactions in the disposable PostgreSQL harness; never platform proof.
#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod fixture;
use linggan_contracts::{parse_producer_attempt, parse_producer_submission};
use linggan_evidence::collection_run::{
    CommentMode, CreateRun, RunCommand, RunQuery, RunSource, command_run, create_run, list_runs,
    read_run,
};
use linggan_evidence::{
    AccountEligibilityObservation, DispatchDecision, activate_installation_credential,
    bind_observation_account, decide_dispatch, report_account_eligibility,
    rotate_installation_credential, set_station_accepting, start_producer_attempt,
    submit_producer_package,
};
use serde_json::json;
use uuid::Uuid;

pub async fn station(db: &linggan_storage_postgres::Database) -> (Uuid, String) {
    let install = Uuid::new_v4();
    let station = Uuid::new_v4();
    sqlx::query("INSERT INTO execution_station(station_ref,display_name,daily_work_quota) VALUES($1,'合成测试工位',10000)").bind(station).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO plugin_installation(installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version,capabilities) VALUES($1,$2,$3,'person',scope_001_now(),'0.8.61','[\"discovery_search\",\"author_profile\",\"profile_discovery\",\"content_detail\",\"comments\",\"replies\",\"media_slots\",\"xhs.pageAccess\",\"collection.single_run.v1\",\"collection.all_public.v1\"]'::jsonb)").bind(install).bind(install.to_string()).bind(station).execute(db.pool()).await.unwrap();
    set_station_accepting(db, station, true, "person")
        .await
        .unwrap();
    let grant = rotate_installation_credential(db, install).await.unwrap();
    let credential = grant.raw_credential.expose_once().to_owned();
    activate_installation_credential(db, install, grant.credential_ref, &credential)
        .await
        .unwrap();
    let account = report_account_eligibility(
        db,
        install,
        &credential,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "collection-single-run-synthetic-account",
        },
        Some(b"collection-single-run-fixture-digest-32-bytes"),
    )
    .await
    .unwrap();
    bind_observation_account(db, account.account_ref.unwrap(), install, "person")
        .await
        .unwrap();
    (install, credential)
}
pub async fn deliver(
    db: &linggan_storage_postgres::Database,
    install: Uuid,
    spec: &serde_json::Value,
    kind: &str,
    records: Vec<serde_json::Value>,
    target: serde_json::Value,
) -> serde_json::Value {
    let attempt = Uuid::new_v4();
    let count = records.len();
    let command=parse_producer_attempt(&json!({"contractVersion":"linggan.producer.attempt.v1","taskId":spec["taskId"],"attemptId":attempt,"producerInstanceId":install}).to_string()).unwrap();
    start_producer_attempt(db, &command).await.unwrap();
    let package = json!({"contractVersion":"linggan.producer.capture-package.v1","packageRef":Uuid::new_v4(),"packageKind":kind,"platform":"xhs","observedAt":"2026-10-13T09:00:00Z","capturedAt":"2026-10-13T09:00:01Z","coverage":{"target":target,"layers":[{"capability":kind,"observed":count,"attempted":count,"acquired":count,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"surface_ended"}]},"records":records});
    let submission=parse_producer_submission(&json!({"contractVersion":"linggan.producer.capture-package.v1","taskId":spec["taskId"],"attemptId":attempt,"producerInstanceId":install,"submissionId":Uuid::new_v4(),"capturePackage":package}).to_string()).unwrap();
    submit_producer_package(db, &submission).await.unwrap();
    package
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn actual_search_dispatch_receipt_freezes_37_in_rank_order_and_never_refills_50() {
    let db = fixture::proof_database("single_run_search_37").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let created = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成查询原词".into(),
            },
            ranking: Some("comprehensive".into()),
            work_limit: 50,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(created["runRef"].clone()).unwrap();
    let decision = decide_dispatch(&db, &install.to_string(), &credential)
        .await
        .unwrap();
    let DispatchDecision::Dispatch { task_spec, .. } = decision else {
        panic!("synthetic search must dispatch: {decision:?}")
    };
    assert_eq!(task_spec["target"]["query"], "合成查询原词");
    assert_eq!(task_spec["ranking"], "comprehensive");
    assert_eq!(task_spec["maximumQuota"], 50);
    let records=(1..=37).map(|n|json!({"kind":"discovery_card","resultPosition":n,"sourceObject":{"platform":"xhs","type":"content","externalId":format!("fixture{n}")},"payload":{"noteId":format!("fixture{n}"),"url":format!("https://www.xiaohongshu.com/explore/fixture{n}?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user"),"title":format!("合成作品 {n}"),"resultPosition":n}})).collect();
    deliver(
        &db,
        install,
        &task_spec,
        "discovery_search",
        records,
        json!({"basis":"maximum_quota","maximumQuota":50,"query":"合成查询原词"}),
    )
    .await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let frozen:Vec<(i32,String)>=sqlx::query_as("SELECT w.ordinal,c.content_external_id FROM collection_run_work w JOIN linggan_material_content c ON c.public_ref=w.content_public_ref WHERE w.run_ref=$1 ORDER BY w.ordinal").bind(run).fetch_all(db.pool()).await.unwrap();
    assert_eq!(frozen.len(), 37);
    assert_eq!(
        frozen.iter().map(|v| v.1.clone()).collect::<Vec<_>>(),
        (1..=37).map(|n| format!("fixture{n}")).collect::<Vec<_>>()
    );
    let before = read_run(&db, run, d).await.unwrap();
    assert_eq!(before["work_limit"], 50);
    assert_eq!(before["selected_works"], 37);
    assert_eq!(before["ended"], false);
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_run_work WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(total, 37);
    let searches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_run_order WHERE run_ref=$1 AND stage='discovery'",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(searches, 1);
}

pub async fn next(
    db: &linggan_storage_postgres::Database,
    install: Uuid,
    credential: &str,
) -> serde_json::Value {
    match decide_dispatch(db, &install.to_string(), credential)
        .await
        .unwrap()
    {
        DispatchDecision::Dispatch { task_spec, .. } => task_spec,
        other => panic!("expected a frozen synthetic lane, got {other:?}"),
    }
}
async fn next_discussion(
    db: &linggan_storage_postgres::Database,
    install: Uuid,
    credential: &str,
) -> serde_json::Value {
    let spec = next(db, install, credential).await;
    if spec["capabilitiesRequested"][0] == "media_slots" {
        assert_eq!(spec["acquireMedia"], "slots");
        let media_scope:bool=sqlx::query_scalar("SELECT m.acquire_media AND m.allow_ocr AND m.allow_asr FROM collection_work_order_material_target m JOIN collection_work_order_lease l USING(work_order_ref) JOIN collection_work_order_lease_task t USING(lease_ref) WHERE t.task_id=$1").bind(spec["taskId"].as_str().unwrap().parse::<Uuid>().unwrap()).fetch_one(db.pool()).await.unwrap();
        assert!(media_scope);
        deliver(
            db,
            install,
            &spec,
            "media_slots",
            vec![],
            json!({"basis":"known_set","contentExternalId":spec["target"]["contentExternalId"]}),
        )
        .await;
        next(db, install, credential).await
    } else {
        spec
    }
}
fn discussion_record(n: usize, note: &str) -> serde_json::Value {
    let payload = if n < 28 {
        json!({"noteId":note,"commentId":format!("c{n}"),"text":format!("合成有效主评论 {n}")})
    } else {
        json!({"noteId":note,"commentId":format!("c{n}"),"rootCommentId":"c0","parentCommentId":"c0","text":format!("合成有效回复 {n}")})
    };
    json!({"kind":if n<28 {"comment"}else{"reply"},"sourceObject":{"platform":"xhs","type":"content","externalId":note},"payload":payload})
}
pub async fn deliver_work_segment(
    db: &linggan_storage_postgres::Database,
    install: Uuid,
    credential: &str,
    start: usize,
    end: usize,
    deep: bool,
    complete: bool,
) {
    let detail = next(db, install, credential).await;
    assert_eq!(detail["capabilitiesRequested"][0], "content_detail");
    let note = detail["target"]["contentExternalId"]
        .as_str()
        .unwrap()
        .to_owned();
    deliver(db,install,&detail,"content_detail",vec![json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":note},"payload":{"noteId":note,"title":"合成详情","bodyText":if start>0{"后来深采详情词"}else{"合成详情先交付，与慢评论独立。"},"authorId":"syntheticauthor","authorName":"合成作者"}})],json!({"basis":"known_set","contentExternalId":note})).await;
    let comments = next_discussion(db, install, credential).await;
    assert_eq!(comments["capabilitiesRequested"][0], "comments");
    if deep {
        assert_eq!(comments["commentMode"], "all_public");
        assert_eq!(
            comments["incrementalCommentBudget"]["knownCommentIds"]
                .as_array()
                .unwrap()
                .len(),
            start
        );
    }
    let mut receipt = json!({"version":1,"noteId":note,"scope":if deep{"all_public_comments"}else if comments.get("incrementalCommentBudget").is_some(){"incremental_new"}else{"detail_window"},"pageCommentCount":500,"expectedCount":if deep{500}else{comments["commentLimit"].as_u64().unwrap() as usize},"uniqueCollectedCount":end-start,"state":if complete{"complete"}else{"partial"},"analysisUsability":if end>start{"usable"}else{"empty"},"targetIdentity":"matched","stopReason":if complete{"comment_area_end"}else{"comment_cap_reached"}});
    if deep {
        receipt["traversal"] =
            json!({"mainExhausted":complete,"repliesExhausted":complete,"identityReliable":true});
    } else {
        receipt["requestedLimit"] = comments["commentLimit"].clone();
    }
    deliver(
        db,
        install,
        &comments,
        "comments",
        (start..end)
            .filter(|n| *n < 28)
            .map(|n| discussion_record(n, &note))
            .collect(),
        json!({"basis":"known_set","contentExternalId":note,"commentCollection":receipt}),
    )
    .await;
    let replies = next(db, install, credential).await;
    assert_eq!(replies["capabilitiesRequested"][0], "replies");
    if deep {
        assert_eq!(replies["commentMode"], "all_public");
        assert!(
            replies.get("replyExpandLimit").is_none(),
            "deep replies inherit no per-root sample cap"
        );
    }
    deliver(
        db,
        install,
        &replies,
        "replies",
        (start..end)
            .filter(|n| *n >= 28)
            .map(|n| discussion_record(n, &note))
            .collect(),
        json!({"basis":"known_set","contentExternalId":note}),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn standard_30_deep_500_standard_30_keep_versions_and_scope_results() {
    let db = fixture::proof_database("single_run_30_500_30").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let first = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成三轮范围".into(),
            },
            ranking: Some("comprehensive".into()),
            work_limit: 1,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let first: Uuid = serde_json::from_value(first["runRef"].clone()).unwrap();
    let search = next(&db, install, &credential).await;
    deliver(&db,install,&search,"discovery_search",vec![json!({"kind":"discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":"deepfixture"},"payload":{"noteId":"deepfixture","url":"https://www.xiaohongshu.com/explore/deepfixture?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user","title":"合成三轮作品"}})],json!({"basis":"maximum_quota","query":"合成三轮范围","maximumQuota":1})).await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    deliver_work_segment(&db, install, &credential, 0, 30, false, true).await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    assert_eq!(
        read_run(&db, first, d).await.unwrap()["delivered_comments"],
        30
    );
    let work: Uuid =
        sqlx::query_scalar("SELECT content_public_ref FROM collection_run_work WHERE run_ref=$1")
            .bind(first)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let deep = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::SelectedWorks {
                work_refs: vec![work],
            },
            ranking: None,
            work_limit: 1,
            comment_mode: CommentMode::AllPublic,
        },
    )
    .await
    .unwrap();
    let deep: Uuid = serde_json::from_value(deep["runRef"].clone()).unwrap();
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    for start in (0..500).step_by(30) {
        let end = (start + 30).min(500);
        deliver_work_segment(&db, install, &credential, start, end, true, end == 500).await;
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
    }
    let results = read_run(&db, deep, d).await.unwrap();
    assert_eq!(results["delivered_comments"], 500);
    assert_eq!(results["new_comments"], 470);
    assert_eq!(results["ended"], true);
    let last = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::SelectedWorks {
                work_refs: vec![work],
            },
            ranking: None,
            work_limit: 1,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let last: Uuid = serde_json::from_value(last["runRef"].clone()).unwrap();
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    deliver_work_segment(&db, install, &credential, 0, 30, false, true).await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    assert_eq!(read_run(&db, last, d).await.unwrap()["new_comments"], 0);
    assert_eq!(
        read_run(&db, first, d).await.unwrap()["delivered_comments"],
        30
    );
    let scoped_query:linggan_contracts::EvidenceQuery=serde_json::from_value(json!({"text":null,"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery","domainRef":d,"collectionRunRef":first})).unwrap();
    let scoped = linggan_evidence::read_work_resources(&db, &scoped_query)
        .await
        .unwrap();
    assert_eq!(scoped.items.len(), 1);
    assert_eq!(
        scoped.items[0]
            .lane_summaries
            .iter()
            .find(|l| l.lane == "comments")
            .unwrap()
            .retained,
        Some(30),
        "future 500 comments must not enter the original 30-result card"
    );
    let old_text:linggan_contracts::EvidenceQuery=serde_json::from_value(json!({"text":"后来深采详情词","scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery","domainRef":d,"collectionRunRef":first})).unwrap();
    assert!(
        linggan_evidence::read_work_resources(&db, &old_text)
            .await
            .unwrap()
            .items
            .is_empty(),
        "future body text cannot match the old run"
    );
    let old_comment:linggan_contracts::EvidenceQuery=serde_json::from_value(json!({"text":"合成有效回复 499","scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery","domainRef":d,"collectionRunRef":first})).unwrap();
    assert!(
        linggan_evidence::read_work_resources(&db, &old_comment)
            .await
            .unwrap()
            .items
            .is_empty(),
        "future comments cannot match the old run"
    );
    assert_eq!(
        scoped.items[0].display.creator_display_name.as_deref(),
        Some("合成作者")
    );
    let cumulative:i64=sqlx::query_scalar("SELECT count(DISTINCT comment_external_id) FROM linggan_material_comment WHERE content_public_ref=$1").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(cumulative, 500);
    let page =
        linggan_evidence::collection_run_material_read::comments(&db, first, d, work, None, None)
            .await
            .unwrap();
    assert_eq!(page["total"], 30);
    assert_eq!(page["returned"], 20);
    assert!(page["nextCursor"].is_string());
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn fifty_fixed_works_deepen_in_shared_queue_without_thirty_or_two_reply_truncation() {
    let db = fixture::proof_database("single_run_fifty_deep").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let run = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成50篇深采".into(),
            },
            ranking: Some("comprehensive".into()),
            work_limit: 50,
            comment_mode: CommentMode::AllPublic,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(run["runRef"].clone()).unwrap();
    let search = next(&db, install, &credential).await;
    let records=(0..50).map(|n|json!({"kind":"discovery_card","resultPosition":n+1,"sourceObject":{"platform":"xhs","type":"content","externalId":format!("allfixture{n}")},"payload":{"noteId":format!("allfixture{n}"),"url":format!("https://www.xiaohongshu.com/explore/allfixture{n}?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user"),"title":format!("合成深采作品 {n}")}})).collect();
    deliver(
        &db,
        install,
        &search,
        "discovery_search",
        records,
        json!({"basis":"maximum_quota","maximumQuota":50,"query":"合成50篇深采"}),
    )
    .await;
    for _ in 0..1000 {
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
        if read_run(&db, run, d).await.unwrap()["ended"] == true {
            break;
        }
        let spec = next(&db, install, &credential).await;
        let kind = spec["capabilitiesRequested"][0].as_str().unwrap();
        let note = spec["target"]["contentExternalId"].as_str().unwrap();
        match kind {
            "content_detail" => {
                deliver(&db,install,&spec,kind,vec![json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":note},"payload":{"noteId":note,"title":format!("详情 {note}"),"bodyText":"合成正文，先交付，评论分段。"}})],json!({"basis":"known_set","contentExternalId":note})).await;
            }
            "media_slots" => {
                deliver(
                    &db,
                    install,
                    &spec,
                    kind,
                    vec![],
                    json!({"basis":"known_set","contentExternalId":note}),
                )
                .await;
            }
            "comments" | "replies" => {
                assert_eq!(spec["commentMode"], "all_public");
                let start = spec["incrementalCommentBudget"]["knownCommentIds"]
                    .as_array()
                    .unwrap()
                    .len();
                let total = if note == "allfixture0" { 500 } else { 31 };
                let end = (start + 30).min(total);
                let records = (start..end)
                    .filter(|n| {
                        if kind == "comments" {
                            *n < 28
                        } else {
                            *n >= 28
                        }
                    })
                    .map(|n| discussion_record(n, note))
                    .collect();
                let mut target = json!({"basis":"known_set","contentExternalId":note});
                if kind == "comments" {
                    target["commentCollection"] = json!({"version":1,"noteId":note,"scope":"all_public_comments","pageCommentCount":total,"expectedCount":total,"uniqueCollectedCount":end-start,"state":if end==total{"complete"}else{"partial"},"analysisUsability":"usable","targetIdentity":"matched","stopReason":if end==total{"comment_area_end"}else{"comment_cap_reached"},"traversal":{"mainExhausted":end==total,"repliesExhausted":end==total,"identityReliable":true}});
                } else {
                    assert!(spec.get("replyExpandLimit").is_none());
                }
                deliver(&db, install, &spec, kind, records, target).await;
            }
            _ => panic!("unexpected deep scope capability {kind}"),
        }
    }
    let result = read_run(&db, run, d).await.unwrap();
    assert_eq!(result["ended"], true);
    assert_eq!(result["selected_works"], 50);
    assert_eq!(result["available_details"], 50);
    assert_eq!(result["delivered_comments"], 2019);
    let page = linggan_evidence::collection_run::read_run_works(&db, run, d, 3)
        .await
        .unwrap();
    assert_eq!(page["total"], 50);
    assert_eq!(page["items"].as_array().unwrap().len(), 10);
    assert_eq!(page["items"][0]["ordinal"], 41);
    assert_eq!(page["items"][9]["ordinal"], 50);
}

async fn domain(db: &linggan_storage_postgres::Database) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO observation_domain(domain_ref,name,status) VALUES($1,$2,'active')")
        .bind(id)
        .bind(format!("fixture-{id}"))
        .execute(db.pool())
        .await
        .unwrap();
    id
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn temporary_run_is_transactional_idempotent_and_not_a_long_term_target() {
    let db = fixture::proof_database("single_run_identity").await;
    let d = domain(&db).await;
    let before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_observation_target WHERE purpose_kind='long_term'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let input = CreateRun {
        idempotency_key: Uuid::new_v4(),
        domain_ref: d,
        source: RunSource::TemporaryKeyword {
            query: "合成关键词".into(),
        },
        ranking: Some("comprehensive".into()),
        work_limit: 50,
        comment_mode: CommentMode::AllPublic,
    };
    let first = create_run(&db, &input).await.unwrap();
    let replay = create_run(&db, &input).await.unwrap();
    assert_eq!(first["runRef"], replay["runRef"]);
    assert_eq!(replay["replayed"], true);
    let after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_observation_target WHERE purpose_kind='long_term'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(before, after);
    let run: Uuid = serde_json::from_value(first["runRef"].clone()).unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_run_order WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    let row = read_run(&db, run, d).await.unwrap();
    assert_eq!(row["comment_mode"], "all_public");
    assert_eq!(row["selected_works"], 0);
    assert_eq!(row["ended"], false);
    assert!(read_run(&db, run, Uuid::new_v4()).await.is_err());
    let mut changed = input;
    changed.work_limit = 37;
    assert!(create_run(&db, &changed).await.is_err());
    assert_eq!(
        linggan_evidence::count_targets(&db, None)
            .await
            .unwrap()
            .total,
        before
    );
    let targets = linggan_evidence::observation_domain::read_observation_domains(&db)
        .await
        .unwrap();
    assert_eq!(
        targets
            .iter()
            .find(|v| v.domain_ref == d)
            .unwrap()
            .target_count,
        0
    );
    let bad_rule = sqlx::query(
        "UPDATE collection_observation_target SET monitoring_enabled=true WHERE purpose_run_ref=$1",
    )
    .bind(run)
    .execute(db.pool())
    .await;
    assert!(bad_rule.is_err());
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn stop_is_durable_versioned_and_cannot_be_reversed_by_replay() {
    let db = fixture::proof_database("single_run_controls").await;
    let d = domain(&db).await;
    let created = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "fixture".into(),
            },
            ranking: None,
            work_limit: 5,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let run = serde_json::from_value(created["runRef"].clone()).unwrap();
    let pause = RunCommand {
        command_ref: Uuid::new_v4(),
        domain_ref: d,
        expected_revision: 1,
        action: "pause".into(),
    };
    assert_eq!(command_run(&db, run, &pause).await.unwrap()["revision"], 2);
    assert_eq!(command_run(&db, run, &pause).await.unwrap()["revision"], 2);
    let stop = RunCommand {
        command_ref: Uuid::new_v4(),
        domain_ref: d,
        expected_revision: 2,
        action: "stop".into(),
    };
    command_run(&db, run, &stop).await.unwrap();
    let resume = RunCommand {
        command_ref: Uuid::new_v4(),
        domain_ref: d,
        expected_revision: 3,
        action: "continue".into(),
    };
    assert!(command_run(&db, run, &resume).await.is_err());
    assert_eq!(
        read_run(&db, run, d).await.unwrap()["control_intent"],
        "stopped"
    );
    assert!(
        sqlx::query("UPDATE collection_run SET control_intent='running' WHERE run_ref=$1")
            .bind(run)
            .execute(db.pool())
            .await
            .is_err()
    );
    let all = list_runs(
        &db,
        &RunQuery {
            domain_ref: Some(d),
            filter: Some("ended".into()),
            q: None,
            page: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(all["total"], 1);
    let wrong = list_runs(
        &db,
        &RunQuery {
            domain_ref: Some(Uuid::new_v4()),
            filter: None,
            q: None,
            page: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(wrong["total"], 0);
    assert!(
        sqlx::query("UPDATE collection_run SET work_limit=200 WHERE run_ref=$1")
            .bind(run)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert_eq!(json!(all["items"][0]["run_ref"]), created["runRef"]);
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn existing_keyword_uses_selected_domain_without_mutating_identity_or_rules() {
    let db = fixture::proof_database("single_run_existing_domains").await;
    let primary = domain(&db).await;
    let chosen = domain(&db).await;
    let target = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source) VALUES($1,'xhs','keyword','合成原词::most_liked','合成原词','manual')").bind(target).execute(db.pool()).await.unwrap();
    for (d, role) in [(primary, "primary"), (chosen, "reference")] {
        sqlx::query(
            "INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,$3)",
        )
        .bind(d)
        .bind(target)
        .bind(role)
        .execute(db.pool())
        .await
        .unwrap();
    }
    let before: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(t) FROM collection_observation_target t WHERE target_ref=$1",
    )
    .bind(target)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let request = CreateRun {
        idempotency_key: Uuid::new_v4(),
        domain_ref: chosen,
        source: RunSource::ObservationTarget { target_ref: target },
        ranking: Some("latest".into()),
        work_limit: 5,
        comment_mode: CommentMode::Sample30,
    };
    let run = create_run(&db, &request).await.unwrap();
    let run: Uuid = serde_json::from_value(run["runRef"].clone()).unwrap();
    let (install, credential) = station(&db).await;
    let spec = next(&db, install, &credential).await;
    assert_eq!(spec["target"]["query"], "合成原词");
    assert_eq!(spec["ranking"], "latest");
    let after: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(t) FROM collection_observation_target t WHERE target_ref=$1",
    )
    .bind(target)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(before, after);
    let rules: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_monitor_rule WHERE target_ref=$1")
            .bind(target)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(rules, 0);
    assert!(read_run(&db, run, primary).await.is_err());
    assert_eq!(
        read_run(&db, run, chosen).await.unwrap()["source_target_ref"],
        serde_json::json!(target)
    );
    sqlx::query("DELETE FROM observation_domain_target WHERE target_ref=$1")
        .bind(target)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM collection_observation_target WHERE target_ref=$1")
        .bind(target)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        read_run(&db, run, chosen).await.unwrap()["control_intent"],
        "stopped"
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn pause_resume_stop_late_and_duplicate_delivery_do_not_restart_or_refill() {
    let db = fixture::proof_database("single_run_late_delivery").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let receipt = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成迟到交付".into(),
            },
            ranking: None,
            work_limit: 50,
            comment_mode: CommentMode::AllPublic,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    let spec = next(&db, install, &credential).await;
    let task: Uuid = serde_json::from_value(spec["taskId"].clone()).unwrap();
    let pause = RunCommand {
        command_ref: Uuid::new_v4(),
        domain_ref: d,
        expected_revision: 1,
        action: "pause".into(),
    };
    command_run(&db, run, &pause).await.unwrap();
    assert_eq!(
        linggan_evidence::probe_collection_run_control(
            &db,
            &install.to_string(),
            &credential,
            task
        )
        .await
        .unwrap()["mayAct"],
        false
    );
    let resume = RunCommand {
        command_ref: Uuid::new_v4(),
        domain_ref: d,
        expected_revision: 2,
        action: "continue".into(),
    };
    command_run(&db, run, &resume).await.unwrap();
    assert_eq!(
        linggan_evidence::probe_collection_run_control(
            &db,
            &install.to_string(),
            &credential,
            task
        )
        .await
        .unwrap()["mayAct"],
        true
    );
    let stop = RunCommand {
        command_ref: Uuid::new_v4(),
        domain_ref: d,
        expected_revision: 3,
        action: "stop".into(),
    };
    command_run(&db, run, &stop).await.unwrap();
    assert_eq!(
        linggan_evidence::probe_collection_run_control(
            &db,
            &install.to_string(),
            &credential,
            task
        )
        .await
        .unwrap()["mayAct"],
        false
    );
    let package=deliver(&db,install,&spec,"discovery_search",vec![json!({"kind":"discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":"latefixture"},"payload":{"noteId":"latefixture","url":"https://www.xiaohongshu.com/explore/latefixture?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user","title":"合成迟到作品"}})],json!({"basis":"maximum_quota","maximumQuota":50,"query":"合成迟到交付"})).await;
    let package_ref: Uuid = serde_json::from_value(package["packageRef"].clone()).unwrap();
    let row: (Uuid,Uuid)=sqlx::query_as("SELECT submission_id,attempt_id FROM linggan_runtime_submission_receipt WHERE package_ref=$1").bind(package_ref).fetch_one(db.pool()).await.unwrap();
    let replay=parse_producer_submission(&json!({"contractVersion":"linggan.producer.capture-package.v1","taskId":spec["taskId"],"attemptId":row.1,"producerInstanceId":install,"submissionId":row.0,"capturePackage":package}).to_string()).unwrap();
    submit_producer_package(&db, &replay).await.unwrap();
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let stopped = read_run(&db, run, d).await.unwrap();
    assert_eq!(stopped["control_intent"], "stopped");
    assert_eq!(
        stopped["selected_works"], 1,
        "already acquired late discovery is retained without dispatching detail"
    );
    let orders: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_run_order WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(orders, 1);
    let packages: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_run_package WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(packages, 1);
    let diagnostic = linggan_evidence::collection_run::executions(&db, Some(d), Some(run), 1)
        .await
        .unwrap();
    assert_eq!(diagnostic["total"], 1);
    assert_eq!(
        diagnostic["items"][0]["attempts"].as_array().unwrap().len(),
        1
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn old_plugin_waits_instead_of_silently_sampling_all_public() {
    let db = fixture::proof_database("single_run_old_protocol").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    sqlx::query("UPDATE plugin_installation SET capabilities=capabilities-'collection.all_public.v1' WHERE installation_ref=$1").bind(install).execute(db.pool()).await.unwrap();
    let receipt = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成兼容等待".into(),
            },
            ranking: None,
            work_limit: 5,
            comment_mode: CommentMode::AllPublic,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    assert!(!matches!(
        decide_dispatch(&db, &install.to_string(), &credential)
            .await
            .unwrap(),
        DispatchDecision::Dispatch { .. }
    ));
    let state = read_run(&db, run, d).await.unwrap();
    assert_eq!(state["ended"], false);
    assert_eq!(state["needs_attention"], false);
    let leases: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order_lease")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(leases, 0);
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn stop_and_real_dispatch_compete_without_post_stop_execution() {
    let db = fixture::proof_database("single_run_stop_dispatch_race").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let run = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成停止竞争".into(),
            },
            ranking: None,
            work_limit: 50,
            comment_mode: CommentMode::AllPublic,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(run["runRef"].clone()).unwrap();
    let stop = RunCommand {
        command_ref: Uuid::new_v4(),
        domain_ref: d,
        expected_revision: 1,
        action: "stop".into(),
    };
    let install_key = install.to_string();
    let (stopped, dispatched) = tokio::join!(
        command_run(&db, run, &stop),
        decide_dispatch(&db, &install_key, &credential)
    );
    stopped.unwrap();
    if let DispatchDecision::Dispatch { task_spec, .. } = dispatched.unwrap() {
        let task: Uuid = serde_json::from_value(task_spec["taskId"].clone()).unwrap();
        assert_eq!(
            linggan_evidence::probe_collection_run_control(
                &db,
                &install.to_string(),
                &credential,
                task
            )
            .await
            .unwrap()["mayAct"],
            false
        );
    }
    assert!(!matches!(
        decide_dispatch(&db, &install.to_string(), &credential)
            .await
            .unwrap(),
        DispatchDecision::Dispatch { .. }
    ));
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    assert_eq!(
        read_run(&db, run, d).await.unwrap()["control_intent"],
        "stopped"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collection_run_order WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn unfinished_replies_and_one_risk_gap_do_not_complete_or_block_other_work() {
    let db = fixture::proof_database("single_run_reply_and_gap").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let run = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成独立失败".into(),
            },
            ranking: None,
            work_limit: 2,
            comment_mode: CommentMode::AllPublic,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(run["runRef"].clone()).unwrap();
    let search = next(&db, install, &credential).await;
    deliver(&db,install,&search,"discovery_search",(0..2).map(|n|json!({"kind":"discovery_card","resultPosition":n+1,"sourceObject":{"platform":"xhs","type":"content","externalId":format!("gapfixture{n}")},"payload":{"noteId":format!("gapfixture{n}"),"url":format!("https://www.xiaohongshu.com/explore/gapfixture{n}?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user"),"title":format!("合成独立作品 {n}")}})).collect(),json!({"basis":"maximum_quota","maximumQuota":2,"query":"合成独立失败"})).await;
    let mut unfinished_checked = false;
    for _ in 0..30 {
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
        if read_run(&db, run, d).await.unwrap()["ended"] == true {
            break;
        }
        let spec = next(&db, install, &credential).await;
        let kind = spec["capabilitiesRequested"][0].as_str().unwrap();
        let note = spec["target"]["contentExternalId"].as_str().unwrap();
        let start = spec["incrementalCommentBudget"]["knownCommentIds"]
            .as_array()
            .map_or(0, Vec::len);
        let end = if note == "gapfixture0" {
            2
        } else {
            (start + 30).min(34)
        };
        let records = if kind == "media_slots" {
            vec![]
        } else if kind == "content_detail" {
            vec![
                json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":note},"payload":{"noteId":note,"title":"合成详情","authorId":"syntheticauthor","authorName":"合成作者","bodyText":"合格详情仍可用"}}),
            ]
        } else {
            (start..end)
                .filter(|n| {
                    if kind == "comments" {
                        *n < 28
                    } else {
                        *n >= 28
                    }
                })
                .map(|n| discussion_record(n, note))
                .collect()
        };
        let mut target = json!({"basis":"known_set","contentExternalId":note});
        if kind == "comments" {
            target["commentCollection"] = json!({"version":1,"noteId":note,"scope":"all_public_comments","pageCommentCount":34,"expectedCount":34,"uniqueCollectedCount":end-start,"state":if note=="gapfixture0" || start==0{"partial"}else{"complete"},"analysisUsability":"usable","targetIdentity":"matched","stopReason":if note=="gapfixture0"{"risk_control"}else{"comment_area_end"},"traversal":{"mainExhausted":true,"repliesExhausted":start>0,"identityReliable":true}});
        }
        deliver(&db, install, &spec, kind, records, target).await;
        if note == "gapfixture1" && kind == "replies" && start == 0 {
            linggan_evidence::collection_run_progress::advance_runs(&db)
                .await
                .unwrap();
            assert_eq!(
                read_run(&db, run, d).await.unwrap()["ended"],
                false,
                "main exhaustion with missing replies is not completion"
            );
            unfinished_checked = true;
        }
    }
    let result = read_run(&db, run, d).await.unwrap();
    assert!(unfinished_checked);
    assert_eq!(result["ended"], true);
    assert_eq!(result["needs_attention"], true);
    assert_eq!(result["available_details"], 2);
    assert_eq!(result["delivered_comments"], 36);
    let works = linggan_evidence::collection_run::read_run_works(&db, run, d, 1)
        .await
        .unwrap();
    assert_eq!(works["items"][0]["gapReason"], "access_restricted");
    assert_eq!(works["items"][1]["checkpoint"]["commentsComplete"], true);
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn configured_periodic_rule_remains_byte_equal_after_one_off() {
    use linggan_evidence::{
        MonitorCommandActor, MonitorCommandKind, MonitorRuleCommand, MonitorRuleDraft,
        MonitorRuleMode, apply_monitor_rule_command,
    };
    let db = fixture::proof_database("single_run_configured_rule").await;
    let d = domain(&db).await;
    let target = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source) VALUES($1,'xhs','creator','syntheticperiodic','合成周期创作者','manual')").bind(target).execute(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,'primary')",
    )
    .bind(d)
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    let rule = apply_monitor_rule_command(
        &db,
        &MonitorRuleCommand {
            target_ref: target,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::SaveRule,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            slot_key: None,
            draft: Some(MonitorRuleDraft {
                mode: MonitorRuleMode::Fixed,
                automatic_enabled: true,
                creator_follow_details: true,
                run_on_weekdays: true,
                run_on_weekends: true,
                all_day: true,
                window_start_minute: None,
                window_end_minute: None,
                fixed_interval_seconds: Some(86400),
                fallback_interval_seconds: 86400,
                surface_key: "creator_profile".into(),
                ranking_key: None,
                scroll_rounds: None,
                top_by_likes: None,
                published_within_days: None,
                task_contract_version: "linggan.producer.task-spec.v2".into(),
            }),
        },
    )
    .await
    .unwrap();
    assert!(
        rule.applied_rule_revision_ref.is_some(),
        "rule rejected: {:?}",
        rule
    );
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('target',to_jsonb(t),'rules',(SELECT jsonb_agg(to_jsonb(r) ORDER BY rule_ref) FROM collection_monitor_rule r WHERE target_ref=$1),'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY revision) FROM collection_monitor_rule_revision v WHERE target_ref=$1)) FROM collection_observation_target t WHERE target_ref=$1").bind(target).fetch_one(db.pool()).await.unwrap();
    create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::ObservationTarget { target_ref: target },
            ranking: None,
            work_limit: 50,
            comment_mode: CommentMode::AllPublic,
        },
    )
    .await
    .unwrap();
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('target',to_jsonb(t),'rules',(SELECT jsonb_agg(to_jsonb(r) ORDER BY rule_ref) FROM collection_monitor_rule r WHERE target_ref=$1),'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY revision) FROM collection_monitor_rule_revision v WHERE target_ref=$1)) FROM collection_observation_target t WHERE target_ref=$1").bind(target).fetch_one(db.pool()).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn partial_sample_pause_resume_preserves_30_total_and_prior_reply_caps() {
    let db = fixture::proof_database("single_run_sample_resume").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let created = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成采样恢复".into(),
            },
            ranking: Some("comprehensive".into()),
            work_limit: 1,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(created["runRef"].clone()).unwrap();
    let search = next(&db, install, &credential).await;
    deliver(&db,install,&search,"discovery_search",vec![json!({"kind":"discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":"samplepartial"},"payload":{"noteId":"samplepartial","url":"https://www.xiaohongshu.com/explore/samplepartial?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user","title":"合成采样恢复"}})],json!({"basis":"maximum_quota","maximumQuota":1,"query":"合成采样恢复"})).await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let detail = next(&db, install, &credential).await;
    deliver(&db,install,&detail,"content_detail",vec![json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"samplepartial"},"payload":{"noteId":"samplepartial","title":"合成采样恢复","bodyText":"有意义的合成正文"}})],json!({"basis":"known_set","contentExternalId":"samplepartial"})).await;
    let comments = next_discussion(&db, install, &credential).await;
    deliver(&db,install,&comments,"comments",(0..8).map(|n|discussion_record(n,"samplepartial")).collect(),json!({"basis":"known_set","contentExternalId":"samplepartial","commentCollection":{"version":1,"noteId":"samplepartial","scope":"detail_window","pageCommentCount":100,"expectedCount":30,"requestedLimit":30,"uniqueCollectedCount":10,"state":"partial","analysisUsability":"usable","targetIdentity":"matched","stopReason":"manual_stop"}})).await;
    let replies = next(&db, install, &credential).await;
    deliver(
        &db,
        install,
        &replies,
        "replies",
        (28..30)
            .map(|n| discussion_record(n, "samplepartial"))
            .collect(),
        json!({"basis":"known_set","contentExternalId":"samplepartial"}),
    )
    .await;
    command_run(
        &db,
        run,
        &RunCommand {
            command_ref: Uuid::new_v4(),
            domain_ref: d,
            expected_revision: 1,
            action: "pause".into(),
        },
    )
    .await
    .unwrap();
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    assert_eq!(
        read_run(&db, run, d).await.unwrap()["delivered_comments"],
        10
    );
    command_run(
        &db,
        run,
        &RunCommand {
            command_ref: Uuid::new_v4(),
            domain_ref: d,
            expected_revision: 2,
            action: "continue".into(),
        },
    )
    .await
    .unwrap();
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let detail = next(&db, install, &credential).await;
    deliver(
        &db,
        install,
        &detail,
        "content_detail",
        vec![],
        json!({"basis":"known_set","contentExternalId":"samplepartial"}),
    )
    .await;
    let comments = next_discussion(&db, install, &credential).await;
    assert_eq!(comments["commentMode"], "sample_30");
    assert_eq!(comments["commentLimit"], 20);
    assert_eq!(
        comments["incrementalCommentBudget"]["knownCommentIds"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    assert_eq!(
        comments["incrementalCommentBudget"]["knownReplyCounts"]["c0"],
        2
    );
    deliver(&db,install,&comments,"comments",(8..28).map(|n|discussion_record(n,"samplepartial")).collect(),json!({"basis":"known_set","contentExternalId":"samplepartial","commentCollection":{"version":1,"noteId":"samplepartial","scope":"incremental_new","pageCommentCount":100,"expectedCount":20,"requestedLimit":20,"uniqueCollectedCount":20,"state":"complete","analysisUsability":"usable","targetIdentity":"matched","stopReason":"comment_cap_reached"}})).await;
    let replies = next(&db, install, &credential).await;
    assert_eq!(replies["replyExpandLimit"], 2);
    deliver(
        &db,
        install,
        &replies,
        "replies",
        vec![],
        json!({"basis":"known_set","contentExternalId":"samplepartial"}),
    )
    .await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let result = read_run(&db, run, d).await.unwrap();
    assert_eq!(result["received_comments"], 30);
    assert_eq!(result["delivered_comments"], 30);
    assert_eq!(result["ended"], true);
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn existing_work_occupies_its_rank_and_resume_cannot_replace_with_51() {
    let db = fixture::proof_database("single_run_existing_rank").await;
    let d = domain(&db).await;
    let (install, credential) = station(&db).await;
    let seed = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成已有作品".into(),
            },
            ranking: Some("comprehensive".into()),
            work_limit: 1,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let seed: Uuid = serde_json::from_value(seed["runRef"].clone()).unwrap();
    let search = next(&db, install, &credential).await;
    let card = |n| json!({"kind":"discovery_card","resultPosition":n,"sourceObject":{"platform":"xhs","type":"content","externalId":format!("existingrank{n}")},"payload":{"noteId":format!("existingrank{n}"),"url":format!("https://www.xiaohongshu.com/explore/existingrank{n}?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user"),"title":format!("合成顺位 {n}")}});
    deliver(
        &db,
        install,
        &search,
        "discovery_search",
        vec![card(1)],
        json!({"basis":"maximum_quota","maximumQuota":1,"query":"合成已有作品"}),
    )
    .await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    deliver_work_segment(&db, install, &credential, 0, 30, false, true).await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let old: Uuid =
        sqlx::query_scalar("SELECT content_public_ref FROM collection_run_work WHERE run_ref=$1")
            .bind(seed)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let created = create_run(
        &db,
        &CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: d,
            source: RunSource::TemporaryKeyword {
                query: "合成固定前50".into(),
            },
            ranking: Some("most_liked".into()),
            work_limit: 50,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let run: Uuid = serde_json::from_value(created["runRef"].clone()).unwrap();
    let search = next(&db, install, &credential).await;
    deliver(
        &db,
        install,
        &search,
        "discovery_search",
        (1..=50).map(card).collect(),
        json!({"basis":"maximum_quota","maximumQuota":50,"query":"合成固定前50"}),
    )
    .await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let initial:Vec<(i32,Uuid)>=sqlx::query_as("SELECT ordinal,content_public_ref FROM collection_run_work WHERE run_ref=$1 ORDER BY ordinal").bind(run).fetch_all(db.pool()).await.unwrap();
    assert_eq!(initial.len(), 50);
    assert_eq!(initial[0], (1, old));
    assert!(sqlx::query_scalar::<_,Option<Uuid>>("SELECT reused_detail_package_ref FROM collection_run_work WHERE run_ref=$1 AND ordinal=1").bind(run).fetch_one(db.pool()).await.unwrap().is_some());
    command_run(
        &db,
        run,
        &RunCommand {
            command_ref: Uuid::new_v4(),
            domain_ref: d,
            expected_revision: 1,
            action: "pause".into(),
        },
    )
    .await
    .unwrap();
    command_run(
        &db,
        run,
        &RunCommand {
            command_ref: Uuid::new_v4(),
            domain_ref: d,
            expected_revision: 2,
            action: "continue".into(),
        },
    )
    .await
    .unwrap();
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let after:Vec<(i32,Uuid)>=sqlx::query_as("SELECT ordinal,content_public_ref FROM collection_run_work WHERE run_ref=$1 ORDER BY ordinal").bind(run).fetch_all(db.pool()).await.unwrap();
    assert_eq!(initial, after);
    let searches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_run_order WHERE run_ref=$1 AND stage='discovery'",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(searches, 1);
    assert!(
        sqlx::query(
            "INSERT INTO collection_run_work(run_ref,content_public_ref,ordinal) VALUES($1,$2,51)"
        )
        .bind(run)
        .bind(old)
        .execute(db.pool())
        .await
        .is_err()
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL harness"]
async fn legacy_grouping_keeps_archive_rounds_supplements_and_unknown_history_separate() {
    let db = fixture::proof_database("single_run_legacy_groups").await;
    let d = domain(&db).await;
    let other = domain(&db).await;
    let target = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source) VALUES($1,'xhs','creator','syntheticlegacy','合成历史创作者','manual')").bind(target).execute(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,'primary')",
    )
    .bind(d)
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    async fn order(
        db: &linggan_storage_postgres::Database,
        target: Uuid,
        d: Uuid,
        lane: &str,
        conditions: serde_json::Value,
    ) -> Uuid {
        let request = Uuid::new_v4();
        let decision = Uuid::new_v4();
        let order = Uuid::new_v4();
        sqlx::query("INSERT INTO collection_acquisition_request(request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) VALUES($1,$2,$3,'primary',$4,'合成历史关系','person')").bind(request).bind(target).bind(d).bind(lane).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO collection_admission_decision(decision_ref,request_ref,outcome,reason_code,target_ref) VALUES($1,$2,'defer','synthetic_history',$3)").bind(decision).bind(request).bind(target).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO collection_work_order(work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,queue_state,scheduled_for,dedupe_key) VALUES($1,$2,$3,$4,50,$5,'queued',scope_001_now(),$1::text)").bind(order).bind(decision).bind(target).bind(lane).bind(conditions).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO collection_work_order_domain_usage(work_order_ref,request_ref,domain_ref,role,basis_kind) VALUES($1,$2,$3,'primary','admitted')").bind(order).bind(request).bind(d).execute(db.pool()).await.unwrap();
        order
    }
    let archive = order(&db, target, d, "deep_archive", json!({})).await;
    sqlx::query("UPDATE collection_work_order SET stop_conditions=jsonb_build_object('progressiveArchive',jsonb_build_object('rootWorkOrderRef',$1::text)) WHERE work_order_ref=$1").bind(archive).execute(db.pool()).await.unwrap();
    let child = order(
        &db,
        target,
        d,
        "deep_archive",
        json!({"progressiveArchive":{"rootWorkOrderRef":archive}}),
    )
    .await;
    let patrol = order(
        &db,
        target,
        d,
        "patrol",
        json!({"ruleVersion":"合成第一轮"}),
    )
    .await;
    let next_patrol = order(
        &db,
        target,
        d,
        "patrol",
        json!({"ruleVersion":"合成第二轮"}),
    )
    .await;
    let follow = order(
        &db,
        target,
        d,
        "deep_archive",
        json!({"collectionUserParentRef":patrol,"collectionUserOrigins":[{"workOrderRef":patrol}]}),
    )
    .await;
    let explicit = order(
        &db,
        target,
        d,
        "deep_archive",
        json!({"collectionUserOrigins":[{"workOrderRef":patrol}]}),
    )
    .await;
    let unknown = order(&db, target, d, "deep_archive", json!(["maximum_quota"])).await;
    let cross_domain = order(
        &db,
        target,
        other,
        "deep_archive",
        json!({"progressiveArchive":{"rootWorkOrderRef":archive}}),
    )
    .await;
    let mapped:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT work_order_ref,run_ref FROM collection_user_order WHERE domain_ref=$1 ORDER BY work_order_ref").bind(d).fetch_all(db.pool()).await.unwrap();
    let mapped: std::collections::HashMap<_, _> = mapped.into_iter().collect();
    assert_eq!(mapped[&archive], archive);
    assert_eq!(mapped[&child], archive);
    assert_eq!(mapped[&follow], patrol);
    assert_eq!(mapped[&next_patrol], next_patrol);
    assert_eq!(mapped[&explicit], explicit);
    assert_eq!(mapped[&unknown], unknown);
    let cross: Uuid =
        sqlx::query_scalar("SELECT run_ref FROM collection_user_order WHERE work_order_ref=$1")
            .bind(cross_domain)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(cross, cross_domain);
    let list = list_runs(
        &db,
        &RunQuery {
            domain_ref: Some(d),
            filter: Some("all".into()),
            q: None,
            page: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(list["total"], 5);
    for row in list["items"].as_array().unwrap() {
        assert_eq!(row["legacy"], true);
        assert_eq!(row["comment_mode"], serde_json::Value::Null);
        assert_eq!(row["new_comments"], serde_json::Value::Null);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_runtime_task")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "reading or migration never dispatches a historical task"
    );
}
