//! Explicit synthetic accepted records for read-projection tests, never provider output.
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn seed_work(db: &Database, domain: Uuid, work: Uuid, fingerprint: &str, result: Value) {
    let mut tx = db.pool().begin().await.unwrap();
    // A synthetic local owner holds the real fence for these accepted test receipts.
    // It never starts an executor or contacts the fixture's deliberately closed endpoint.
    sqlx::query("SELECT pg_advisory_xact_lock(linggan_ai_executor_lock_key())")
        .execute(&mut *tx)
        .await
        .unwrap();
    let owner = Uuid::new_v4();
    let generation: i64 =
        sqlx::query_scalar("SELECT generation+1 FROM linggan_ai_executor_current WHERE singleton")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    sqlx::query("INSERT INTO linggan_ai_executor_instance(instance_ref,generation,owner_backend_pid,process_id,source_revision,concurrency,state) VALUES($1,$2,pg_backend_pid(),1,'SYNTHETIC read fixture',4,'running')")
        .bind(owner).bind(generation).execute(&mut *tx).await.unwrap();
    sqlx::query(
        "UPDATE linggan_ai_executor_current SET instance_ref=$1,generation=$2 WHERE singleton",
    )
    .bind(owner)
    .bind(generation)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("SELECT set_config('linggan.ai_executor_instance',$1,true),set_config('linggan.ai_executor_generation',$2,true)").bind(owner.to_string()).bind(generation.to_string()).execute(&mut *tx).await.unwrap();
    let (command, batch, item, invocation, connection, version, model, config, request) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'SYNTHETIC read fixture','openai-completions','http://127.0.0.1:1',true,$3)").bind(version).bind(connection).bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-read-fixture','manual')").bind(model).bind(version).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,8192,1000,30,1)").bind(config).bind(model).execute(&mut *tx).await.unwrap();
    let task: Uuid = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='creator_research'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,'SYNTHETIC_ACCEPTED_WORK','{}','{}')").bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref) VALUES($1,'creator_research',$2,'interactive',repeat('0',64),1,$3)").bind(batch).bind(task).bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) VALUES($1,$2,0,'SYNTHETIC_ACCEPTED_WORK',$3)").bind(item).bind(batch).bind(json!({"kind":"creator_research","requestRef":request})).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) VALUES($1,$2,$3,$4,'analyze',repeat('0',64),'succeeded',0,0,$5,scope_001_now())").bind(invocation).bind(version).bind(model).bind(config).bind(json!({"synthetic":true,"notProviderOutput":true})).execute(&mut *tx).await.unwrap();
    // Synthetic receipts still pass the real lease/attempt/terminal guards.
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=lease_generation+1,lease_token=$2,lease_expires_at=scope_001_now()+interval '1 minute',lease_instance_ref=$3,lease_instance_generation=$4,started_at=scope_001_now() WHERE item_ref=$1")
        .bind(item).bind(Uuid::new_v4()).bind(owner).bind(generation).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control control SET attempt_count=1,invocation_ref=$2,executed_config_ref=config.task_config_ref,executed_skill_version_ref=config.skill_version_ref,execution_contract=config.execution_contract FROM linggan_ai_task_config config WHERE control.item_ref=$1 AND config.task_config_ref=$3")
        .bind(item).bind(invocation).bind(task).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='succeeded',usable=true,output=$2,finished_at=scope_001_now(),lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL WHERE item_ref=$1")
        .bind(item).bind(&result).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_creator_research_request(request_ref,command_ref,domain_ref,work_ref,knowledge_cutoff,input_fingerprint,task_config_ref,batch_ref,method_version) VALUES($1,$2,$3,$4,scope_001_now(),$5,$6,$7,'creator-work-research.v1')").bind(request).bind(command).bind(domain).bind(work).bind(fingerprint).bind(task).bind(batch).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_creator_research_result(request_ref,invocation_ref,result) VALUES($1,$2,$3)").bind(request).bind(invocation).bind(result).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_executor_instance SET state='drained',drain_requested_at=scope_001_now(),drained_at=scope_001_now() WHERE instance_ref=$1").bind(owner).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}
pub async fn replace_hints(db: &Database, domain: Uuid, work: Uuid, hints: Value) {
    let mut result:Value=sqlx::query_scalar("SELECT result.result FROM linggan_creator_research_request request JOIN linggan_creator_research_result result USING(request_ref) WHERE request.domain_ref=$1 AND request.work_ref=$2 ORDER BY request.created_at DESC,request.request_ref DESC LIMIT 1").bind(domain).bind(work).fetch_one(db.pool()).await.unwrap();
    result["topicHints"] = hints;
    let fingerprint:String=sqlx::query_scalar("SELECT input_fingerprint FROM linggan_creator_research_request WHERE domain_ref=$1 AND work_ref=$2 ORDER BY created_at DESC,request_ref DESC LIMIT 1").bind(domain).bind(work).fetch_one(db.pool()).await.unwrap();
    seed_work(db, domain, work, &fingerprint, result).await;
}
