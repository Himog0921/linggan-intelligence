//! PostgreSQL evaluation of the production queue predicate with synthetic ledger states.
//! Temporary tables keep this matrix independent of model calls and mutable provider settings.
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

#[tokio::test]
#[ignore = "isolated PostgreSQL only"]
async fn topic_resolution_domain_order_releases_terminal_work_and_never_blocks_review() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("isolated proof URL");
    let mut db = PgConnection::connect(&url).await.unwrap();
    sqlx::raw_sql("CREATE TEMP TABLE linggan_topic_research_run(run_ref uuid,domain_ref uuid);
        CREATE TEMP TABLE linggan_topic_research_window(window_ref uuid,run_ref uuid);
        CREATE TEMP TABLE linggan_topic_research_stage(item_ref uuid,window_ref uuid,stage_number integer);
        CREATE TEMP TABLE linggan_ai_batch_item_control(item_ref uuid,state text,failure_code text);")
        .execute(&mut db).await.unwrap();
    let domain = Uuid::new_v4();
    let first = window(&mut db, domain).await;
    let second = window(&mut db, domain).await;
    let other_domain = window(&mut db, Uuid::new_v4()).await;
    let query = concat!(
        "SELECT EXISTS(SELECT 1 FROM (SELECT $1::uuid AS item_ref) item WHERE true ",
        include_str!("../../src/ai_foundation/executor/topic_resolution_order.sql"),
        ")"
    );
    // No claim yet: another merely queued window must not hold the domain.
    assert!(eligible(&mut db, query, second[2]).await);
    // Phase 0/1 delays or failures never take the assignment slot.
    state(&mut db, first[0], "running", None).await;
    state(&mut db, first[1], "failed", Some("ai_provider_failed")).await;
    assert!(eligible(&mut db, query, second[2]).await);
    state(&mut db, first[2], "running", None).await;
    assert!(!eligible(&mut db, query, second[2]).await);
    assert!(eligible(&mut db, query, first[3]).await);
    assert!(eligible(&mut db, query, other_domain[2]).await);
    state(&mut db, first[2], "succeeded", None).await;
    assert!(!eligible(&mut db, query, second[2]).await);
    state(&mut db, first[3], "running", None).await;
    assert!(!eligible(&mut db, query, second[2]).await);
    // Even legacy overlapping assignments can both drain their review stages.
    state(&mut db, second[2], "succeeded", None).await;
    assert!(eligible(&mut db, query, first[3]).await);
    assert!(eligible(&mut db, query, second[3]).await);
    state(&mut db, second[2], "pending", None).await;
    for terminal in ["succeeded", "failed", "skipped", "cancelled"] {
        state(&mut db, first[3], terminal, None).await;
        assert!(eligible(&mut db, query, second[2]).await, "{terminal}");
    }
    // A failed assignment cannot strand the domain behind its pending review.
    state(&mut db, first[2], "failed", Some("ai_provider_failed")).await;
    state(&mut db, first[3], "pending", None).await;
    assert!(eligible(&mut db, query, second[2]).await);
    for stage in [2, 3] {
        state(&mut db, first[stage], "failed", Some("ai_dispatch_unknown")).await;
        // A fenced failed call cannot publish; source-scoped retry protection lives in start().
        assert!(eligible(&mut db, query, second[2]).await);
        assert!(eligible(&mut db, query, other_domain[2]).await);
        assert!(eligible(&mut db, query, first[3]).await);
        state(&mut db, first[stage], "failed", Some("ai_provider_failed")).await;
    }
}

async fn window(db: &mut PgConnection, domain: Uuid) -> [Uuid; 4] {
    let run = Uuid::new_v4();
    let window = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_research_run VALUES($1,$2)")
        .bind(run)
        .bind(domain)
        .execute(&mut *db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_topic_research_window VALUES($1,$2)")
        .bind(window)
        .bind(run)
        .execute(&mut *db)
        .await
        .unwrap();
    let items = std::array::from_fn(|_| Uuid::new_v4());
    for (stage, item) in items.iter().enumerate() {
        sqlx::query("INSERT INTO linggan_topic_research_stage VALUES($1,$2,$3)")
            .bind(item)
            .bind(window)
            .bind(stage as i32)
            .execute(&mut *db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO linggan_ai_batch_item_control VALUES($1,'pending',NULL)")
            .bind(item)
            .execute(&mut *db)
            .await
            .unwrap();
    }
    items
}
async fn state(db: &mut PgConnection, item: Uuid, state: &str, code: Option<&str>) {
    sqlx::query(
        "UPDATE linggan_ai_batch_item_control SET state=$2,failure_code=$3 WHERE item_ref=$1",
    )
    .bind(item)
    .bind(state)
    .bind(code)
    .execute(db)
    .await
    .unwrap();
}
async fn eligible(db: &mut PgConnection, query: &'static str, item: Uuid) -> bool {
    sqlx::query_scalar(query)
        .bind(item)
        .fetch_one(db)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "isolated PostgreSQL only"]
async fn topic_resolution_concurrent_claims_take_only_one_domain_assignment() {
    use linggan_intelligence::ai_foundation::{
        batch_store::InputRef,
        executor::{ExecutorSession, queue::claim_next},
    };
    let db = super::dossier::fixture::proof_database("topic_queue_order").await;
    let work = super::dossier::work(&db).await;
    // Queue-only fixture: no provider, reservation, or model dispatch is performed.
    for _ in 0..2 {
        let command = Uuid::new_v4();
        let run = Uuid::new_v4();
        let window = Uuid::new_v4();
        let mut tx = db.pool().begin().await.unwrap();
        sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,'synthetic_queue','{}','{}')")
            .bind(command).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO linggan_topic_research_run(run_ref,command_ref,domain_ref,work_refs,knowledge_cutoff,configurations) VALUES($1,$2,$3,$4,clock_timestamp(),'[null,null,null,null]')")
            .bind(run).bind(command).bind(super::dossier::DOMAIN).bind(vec![work]).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO linggan_topic_research_window(window_ref,run_ref,work_ref,input_hash,input_refs) VALUES($1,$2,$3,repeat('0',64),'{}')")
            .bind(window).bind(run).bind(work).execute(&mut *tx).await.unwrap();
        for stage in [2, 3] {
            let batch = Uuid::new_v4();
            let item = Uuid::new_v4();
            let task = if stage == 2 {
                "topic_research_resolve"
            } else {
                "topic_research_resolve_review"
            };
            sqlx::query("INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref) SELECT $1,task_key,current_config_ref,'interactive',repeat('0',64),1,$3 FROM linggan_ai_task_setting WHERE task_key=$2")
                .bind(batch).bind(task).bind(command).execute(&mut *tx).await.unwrap();
            sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) VALUES($1,$2,0,'synthetic',$3)")
                .bind(item).bind(batch).bind(serde_json::to_value(InputRef::TopicResearch{window_ref:window,stage_number:stage}).unwrap()).execute(&mut *tx).await.unwrap();
            sqlx::query("INSERT INTO linggan_topic_research_stage(item_ref,window_ref,stage_number) VALUES($1,$2,$3)")
                .bind(item).bind(window).bind(stage as i32).execute(&mut *tx).await.unwrap();
        }
        tx.commit().await.unwrap();
    }
    for scope in [
        super::ControlScope::Global,
        super::ControlScope::Executor,
        super::ControlScope::Task(super::research::TASKS[2]),
        super::ControlScope::Task(super::research::TASKS[3]),
    ] {
        super::setup::enable(&db, scope).await;
    }
    let owner = ExecutorSession::acquire(&db, "synthetic-topic-queue", 4)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        claim_next(&db, owner.fence()),
        claim_next(&db, owner.fence())
    );
    let claims: Vec<_> = [a.unwrap(), b.unwrap()].into_iter().flatten().collect();
    assert_eq!(
        claims.len(),
        1,
        "same-domain assignments must not claim together"
    );
    assert!(matches!(
        claims[0].input,
        InputRef::TopicResearch {
            stage_number: 2,
            ..
        }
    ));
    assert!(claim_next(&db, owner.fence()).await.unwrap().is_none());
}
