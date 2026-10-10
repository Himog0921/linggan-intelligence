use super::*;

async fn copied_terminal_source(
    tx: &mut Transaction<'_, Postgres>,
    source: Uuid,
    request: Uuid,
    definition: Uuid,
) -> Uuid {
    let task = Uuid::new_v4();
    let recall = json!({"backfill":{"authorizationRequestRef":request,
        "forcedDefinitionRefs":[definition],"sourceTaskRef":source,
        "reason":"synthetic reassignment snapshot"}});
    sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,state,phase,distilled_json,resolutions_json,recall_manifest,updated_at) SELECT $1,run_ref,domain_ref,work_public_ref,input_hash,input_refs,'succeeded','resolve',distilled_json,resolutions_json,$3,updated_at+interval '1 microsecond' FROM linggan_topic_map_research_task WHERE task_ref=$2")
        .bind(task).bind(source).bind(recall).execute(&mut **tx).await.unwrap();
    task
}

async fn wait_for_comparison_policy(db: &Database, blocker: i32) {
    for _ in 0..500 {
        let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity a WHERE $1=ANY(pg_blocking_pids(a.pid)) AND a.wait_event_type='Lock' AND a.query LIKE '%SELECT p.status,p.automatic_enabled,d.status AS domain_status%' AND a.query LIKE '%FOR UPDATE OF p%')")
            .bind(blocker).fetch_one(db.pool()).await.unwrap();
        if blocked {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("comparison never reached its policy lock after building the captured input");
}

async fn pending_authorized_comparison(db: &Database, run: Uuid, definition: Uuid) -> (Uuid, Uuid) {
    let source: Uuid = sqlx::query_scalar("SELECT task_ref FROM linggan_topic_map_research_task WHERE run_ref=$1 AND phase='resolve' AND state='succeeded' AND jsonb_array_length(resolutions_json)>0 ORDER BY updated_at DESC,task_ref DESC LIMIT 1")
        .bind(run).fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let request = Uuid::new_v4();
    let task = copied_terminal_source(&mut tx, source, request, definition).await;
    let scope: Value = sqlx::query_scalar(
        "SELECT input_scope FROM linggan_topic_map_research_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let permission = json!({"backfillReopened":true,"backfillAuthorizations":[{
        "requestRef":request,"workRefs":scope["workRefs"],"definitionRefs":[definition]}]});
    sqlx::query(
        "UPDATE linggan_topic_map_research_run SET input_scope=input_scope||$2 WHERE run_ref=$1",
    )
    .bind(run)
    .bind(permission)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    (task, request)
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL snapshot and row locks; synthetic adapter only"]
async fn comparison_wait_cannot_attach_an_old_grant_or_checkpoint_to_changed_dependencies() {
    let (db, config, adapter) = setup("topic_core_comparison_snapshot_fence").await;
    let work = work(
        &db,
        "comparison-snapshot",
        "SYNTHETIC COMPARE 开始练习需要明确步骤。",
    )
    .await;
    comments::comment_with_author(
        &db,
        "comparison-snapshot",
        "snapshot-reader",
        "SYNTHETIC CHALLENGE 开始练习还需要第一步的示范。",
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let receipt = apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    finish_pending(&db, &adapter).await;
    let counts = request_counts(&db, run).await;
    assert_eq!(
        counts.2, 1,
        "the unchanged source produced its initial comparison"
    );
    assert_eq!(run_state(&db, run).await, "completed");
    let definition = initial_definition(&db).await;
    let (source, request) = pending_authorized_comparison(&db, run, definition).await;
    let prior_scope: Value = sqlx::query_scalar(
        "SELECT input_scope FROM linggan_topic_map_research_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();

    let mut owner = db.pool().begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *owner)
        .await
        .unwrap();
    sqlx::query(
        "SELECT domain_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(D)
    .execute(&mut *owner)
    .await
    .unwrap();
    let worker = launch(&db, &adapter);
    wait_for_comparison_policy(&db, blocker).await;
    assert!(!worker.is_finished());
    // The new dependency uses a definition absent from the frozen request.
    // Commit it while the queue writer is waiting, after its input was built.
    let changed = revision_in(&mut owner, definition).await;
    copied_terminal_source(&mut owner, source, request, changed).await;
    mark_definition_scanned(&mut owner, changed).await;
    owner.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), worker)
        .await
        .unwrap()
        .unwrap();
    let after: Value = sqlx::query_scalar(
        "SELECT input_scope FROM linggan_topic_map_research_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        after, prior_scope,
        "a stale snapshot cannot write a comparison checkpoint"
    );
    assert_eq!(request_counts(&db, run).await, counts);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_task WHERE run_ref=$1 AND phase='compare'")
        .bind(run).fetch_one(db.pool()).await.unwrap(), 1);
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        request_counts(&db, run).await,
        counts,
        "a later tick cannot renew the old grant"
    );
}
