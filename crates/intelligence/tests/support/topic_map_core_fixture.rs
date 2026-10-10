//! Shared, handwritten synthetic fixture; no provider or platform requests.
#[path = "../../../evidence/tests/support/material_fixture.rs"]
pub mod fixture;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets, pi_adapter::PiAdapter,
    topic_map_research::ResearchCommand, topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
pub const D: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
pub async fn setup(name: &str) -> (Database, Uuid, PiAdapter) {
    let db = fixture::proof_database(name).await;
    let conn = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision)VALUES($1,1)")
        .bind(conn)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref)VALUES($1,$2,1,'Synthetic','openai-completions','http://127.0.0.1:18080',true,$3)").bind(version).bind(conn).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin)VALUES($1,$2,'synthetic-topic-map','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts)VALUES($1,$2,8192,1000,30,$3)").bind(config).bind(model).bind(if name=="topic_map_research_unsent"{1}else{3}).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result)VALUES($1,$2,$3,'probe','synthetic','succeeded',0,0,$4)").bind(Uuid::new_v4()).bind(version).bind(model).bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true})).execute(db.pool()).await.unwrap();
    let node = std::env::var_os("CREATOR_PROOF_NODE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let output = std::process::Command::new("which")
                .arg("node")
                .output()
                .expect("locate synthetic proof Node executable");
            assert!(
                output.status.success(),
                "set CREATOR_PROOF_NODE to a local Node executable"
            );
            std::path::PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
        });
    assert!(node.is_file(), "synthetic proof Node executable must exist");
    let adapter = PiAdapter::configured_with_test_command(
        node,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/topic_map_research_adapter.mjs"),
    );
    (db, config, adapter)
}
pub async fn work(db: &Database, id: &str, body: &str) -> Uuid {
    work_with_title(db, id, "", body).await
}
pub async fn work_with_title(db: &Database, id: &str, title: &str, body: &str) -> Uuid {
    work_at(db, id, title, body, 10, "2026-08-28T10:00:00Z").await
}
pub async fn work_at(
    db: &Database,
    id: &str,
    title: &str,
    body: &str,
    likes: i64,
    observed_at: &str,
) -> Uuid {
    let package=fixture::submit_package_at(db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":title,"bodyText":body,"authorId":"synthetic-creator","likes":likes}}),observed_at).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref)VALUES($1,$2,$3,'primary','legacy_domain_migration',$4) ON CONFLICT DO NOTHING").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    work
}
pub fn configure(config: Uuid, daily: i64, auto: bool) -> ResearchCommand {
    ResearchCommand::Configure {
        request_ref: Uuid::new_v4(),
        domain_ref: D,
        model_config_ref: config,
        daily_token_limit: daily,
        run_token_limit: 100000,
        automatic_enabled: auto,
        collection_enabled: false,
    }
}
pub fn start(works: Vec<Uuid>) -> ResearchCommand {
    ResearchCommand::Start {
        request_ref: Uuid::new_v4(),
        domain_ref: D,
        trigger: "on_demand".into(),
        work_refs: works,
        topic_ref: None,
    }
}
pub async fn finish_pending(db: &Database, adapter: &PiAdapter) {
    for _ in 0..64 {
        if !run_once(db, &SyntheticModelSecrets, adapter).await.unwrap() {
            return;
        }
    }
    panic!("synthetic bounded research did not reach an idle state");
}
