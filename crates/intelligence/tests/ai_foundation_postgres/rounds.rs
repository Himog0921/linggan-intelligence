//! Counter/transaction proof only. Every invocation below is explicitly constructed synthetic data.
use super::{fixture, new_connection, version};
use linggan_intelligence::ai_foundation::rounds::evaluate_ready_rounds;
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

async fn storage_batch(db: &Database, task: &str, count: i32) -> Uuid {
    let command = Uuid::new_v4();
    let config = Uuid::new_v4();
    let batch = Uuid::new_v4();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,'synthetic_storage_fixture','{\"syntheticFixture\":true}','{\"syntheticFixture\":true}')")
        .bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,execution_contract,material_columns,budget_mode,origin,command_ref) VALUES($1,$2,'synthetic.round-storage.v1','[\"title\"]','unconfigured','human',$3)")
        .bind(config).bind(task).bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref) VALUES($1,$2,$3,CASE WHEN $4>50 THEN 'interactive' ELSE 'trial' END,repeat('0',64),$4,$5)")
        .bind(batch).bind(task).bind(config).bind(count).bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) SELECT gen_random_uuid(),$1,n,n::text,'{\"kind\":\"synthetic_round_fixture\",\"syntheticFixture\":true}' FROM generate_series(0,$2-1)n")
        .bind(batch).bind(count).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    batch
}
async fn settle_storage_batch(db: &Database, batch: Uuid, version: Uuid, failed: i32, usable: i32) {
    let owner = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        db,
        "synthetic-round-fixture",
        4,
    )
    .await
    .unwrap();
    let mut tx = owner.fence().begin(db, true).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control c SET state='running',lease_generation=lease_generation+1,
        lease_token=gen_random_uuid(),lease_instance_ref=$2,lease_instance_generation=$3,lease_expires_at=scope_001_now()+interval '1 minute'
        FROM linggan_ai_batch_item i WHERE i.item_ref=c.item_ref AND i.batch_ref=$1")
        .bind(batch).bind(owner.fence().instance_ref()).bind(owner.fence().generation()).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result)
        SELECT item_ref,$2,'analyze',repeat('0',64),'running',0,0,'{\"syntheticFixture\":true}' FROM linggan_ai_batch_item WHERE batch_ref=$1")
        .bind(batch).bind(version).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control c SET invocation_ref=i.item_ref,attempt_count=1,
        executed_config_ref=b.task_config_ref,executed_skill_version_ref=config.skill_version_ref,execution_contract=config.execution_contract
        FROM linggan_ai_batch_item i JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_task_config config ON config.task_config_ref=b.task_config_ref
        WHERE i.item_ref=c.item_ref AND i.batch_ref=$1")
        .bind(batch).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_model_invocation invocation SET state=CASE WHEN i.position<$2 THEN 'failed' ELSE 'succeeded' END,
        failure_code=CASE WHEN i.position<$2 THEN 'provider_timeout' ELSE NULL END,
        input_tokens=0,output_tokens=0,finished_at=scope_001_now() FROM linggan_ai_batch_item i WHERE invocation.invocation_ref=i.item_ref AND i.batch_ref=$1")
        .bind(batch).bind(failed).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control c SET state=CASE WHEN i.position<$2 THEN 'failed' ELSE 'succeeded' END,
        counts_as_failure=i.position<$2,usable=i.position>=$2 AND i.position<$2+$3,
        output=CASE WHEN i.position<$2 THEN NULL ELSE '{\"syntheticFixture\":true}'::jsonb END,
        lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL,finished_at=scope_001_now()
        FROM linggan_ai_batch_item i WHERE i.item_ref=c.item_ref AND i.batch_ref=$1")
        .bind(batch).bind(failed).bind(usable).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}
async fn skip_storage_batch(db: &Database, batch: Uuid) {
    sqlx::query("UPDATE linggan_ai_batch_item_control c SET state='skipped',usable=false,finished_at=scope_001_now()
        FROM linggan_ai_batch_item i WHERE i.item_ref=c.item_ref AND i.batch_ref=$1")
        .bind(batch).execute(db.pool()).await.unwrap();
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_round_storage_enforces_strict_ratio_atomic_off_and_terminal_receipts() {
    let db = fixture::proof_database("ai_round_ratio").await;
    let connection = new_connection(&db).await;
    let v = version(&db, connection, 1, "https://synthetic-round.example/api").await;
    for (failed, braked) in [(15, false), (16, true)] {
        let batch = storage_batch(&db, "note_interpretation", 50).await;
        assert!(
            evaluate_ready_rounds(&db, batch).await.unwrap().is_empty(),
            "unsettled items cannot make a round"
        );
        settle_storage_batch(&db, batch, v, failed, 50 - failed).await;
        let round = evaluate_ready_rounds(&db, batch).await.unwrap();
        assert_eq!(round.len(), 1);
        let row = sqlx::query(
            "SELECT attempted,failed,usable,brake_reason FROM linggan_ai_round WHERE round_ref=$1",
        )
        .bind(round[0])
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.get::<i32, _>("attempted"), 50);
        assert_eq!(row.get::<i32, _>("failed"), failed);
        assert_eq!(
            row.get::<Option<String>, _>("brake_reason").is_some(),
            braked
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT state FROM linggan_ai_batch_control WHERE batch_ref=$1"
            )
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap(),
            "completed"
        );
        assert!(evaluate_ready_rounds(&db, batch).await.unwrap().is_empty());
        assert!(
            sqlx::query("UPDATE linggan_ai_round SET attempted=1 WHERE round_ref=$1")
                .bind(round[0])
                .execute(db.pool())
                .await
                .is_err()
        );
    }
    let off=sqlx::query("SELECT c.enabled,c.actor_kind,c.reason,command.receipt FROM linggan_ai_control c JOIN linggan_ai_command command ON command.command_ref=c.command_ref WHERE c.scope_key='task:note_interpretation'")
        .fetch_one(db.pool()).await.unwrap();
    assert!(!off.get::<bool, _>("enabled"));
    assert_eq!(off.get::<String, _>("actor_kind"), "system");
    assert_eq!(
        off.get::<String, _>("reason"),
        "automatic_brake:failure_ratio"
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_ai_control_event WHERE scope_key='task:note_interpretation'").fetch_one(db.pool()).await.unwrap(),1);
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_round_storage_keeps_cross_batch_empty_streak_but_never_brakes_empty_or_interactive_rounds()
 {
    let db = fixture::proof_database("ai_round_empty").await;
    let connection = new_connection(&db).await;
    let v = version(&db, connection, 1, "https://synthetic-empty.example/api").await;
    for iteration in 1..=3 {
        let batch = storage_batch(&db, "note_interpretation", 50).await;
        if iteration <= 2 {
            settle_storage_batch(&db, batch, v, 0, 0).await;
        } else {
            skip_storage_batch(&db, batch).await;
        }
        assert_eq!(evaluate_ready_rounds(&db, batch).await.unwrap().len(), 1);
        let row=sqlx::query("SELECT attempted,consecutive_without_usable,brake_reason FROM linggan_ai_round WHERE batch_ref=$1")
            .bind(batch).fetch_one(db.pool()).await.unwrap();
        assert_eq!(
            row.get::<i32, _>("consecutive_without_usable"),
            iteration.min(2)
        );
        assert_eq!(
            row.get::<i32, _>("attempted"),
            if iteration <= 2 { 50 } else { 0 }
        );
        assert_eq!(
            row.get::<Option<String>, _>("brake_reason").is_some(),
            iteration == 2
        );
    }
    let interactive = storage_batch(&db, "writing_legacy", 100).await;
    settle_storage_batch(&db, interactive, v, 100, 0).await;
    let evaluated = evaluate_ready_rounds(&db, interactive).await.unwrap();
    assert_eq!(evaluated.len(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ai_round WHERE batch_ref=$1 AND brake_reason IS NOT NULL"
        )
        .bind(interactive)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_ai_control_event WHERE scope_key='task:note_interpretation'").fetch_one(db.pool()).await.unwrap(),1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ai_control_event WHERE scope_key='task:writing_legacy'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
}
