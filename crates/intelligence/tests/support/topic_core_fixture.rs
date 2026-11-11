//! Explicit synthetic upstream Core, admitted through public preview/start/accept.
use super::dossier;
use base64::Engine as _;
use linggan_intelligence::{
    ai_foundation::{content_understanding as owner, executor::ExecutorSession},
    content_understanding as contract,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

pub fn bytes() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=").unwrap()
}
pub async fn image(db: &Database, id: &str) {
    let observation = Uuid::new_v4();
    let slot = format!("xhs:{id}:image:1");
    dossier::fixture::submit_custom_package(db,"xhs",&["media_slots"],json!({"contentExternalId":id}),"media_slots","xhs",
        json!({"target":{"contentExternalId":id,"imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":1}},"layers":[dossier::fixture::coverage_layer("media_slots",1)]}),
        vec![json!({"kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"image","ordinal":1,"displayOrdinal":1,"displayOrderBasis":"platform_explicit"},"observation":{"externalUri":"https://media.example/synthetic.jpg","candidateUris":["https://media.example/synthetic.jpg"],"observedAt":"2026-10-14T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":id}})]).await;
    let bytes = bytes();
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let storage = format!("blobs/{hash}.png");
    let root = std::path::PathBuf::from(
        std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT").expect("owned proof media root"),
    );
    std::fs::create_dir_all(root.join("blobs")).unwrap();
    std::fs::write(root.join(&storage), &bytes).unwrap();
    linggan_evidence::admit_media_blob(
        db,
        observation,
        &hash,
        "image/png",
        bytes.len() as i64,
        &storage,
    )
    .await
    .unwrap();
}
pub async fn prepare(db: &Database, work: Uuid, executor: &ExecutorSession) {
    let preview = owner::preview(db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(preview["canStart"], true, "{preview}");
    let receipt = owner::start(
        db,
        &owner::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            work_ref: work,
            knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
            input_hash: preview["inputHash"].as_str().unwrap().into(),
            task_config_ref: serde_json::from_value(preview["taskConfigRef"].clone()).unwrap(),
            task_revision: preview["taskRevision"].as_i64().unwrap(),
        },
    )
    .await
    .unwrap();
    let row=sqlx::query("SELECT r.input_manifest,r.task_config_ref,t.model_config_ref,t.prompt_version_ref FROM linggan_content_understanding_request r JOIN linggan_ai_task_config t USING(task_config_ref) WHERE request_ref=$1").bind(receipt.request_ref).fetch_one(db.pool()).await.unwrap();
    let manifest: Value = row.get("input_manifest");
    let snapshot: contract::Snapshot =
        serde_json::from_value(manifest["snapshot"].clone()).unwrap();
    let source = snapshot.texts.iter().find(|s| s.kind == "body").unwrap();
    let quote = "结尾不能丢失🙂";
    assert!(source.text.contains(quote));
    let draft = json!({"core":{"summary":{"text":"合成正文末尾保留","unitIds":["u1"]},"contentType":"practical_guide","mainTopicUnitId":"u1","knowledgeUnits":[{"id":"u1","unitType":"discussion","statement":"正文末尾不能丢失。","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":["e1"]}],"candidateLabels":[]},
        "imageContributions":snapshot.images.iter().map(|i|json!({"inputCitationId":i.citation_id,"status":"no_new_information","summary":"合成图片不增加主题证据","unitIds":[],"evidenceRefs":[]})).collect::<Vec<_>>(),
        "evidenceCandidates":{"text":[{"id":"e1","inputCitationId":source.citation_id,"quote":quote}],"vision":[]}});
    let config: Uuid = row.get("model_config_ref");
    let accepted = contract::accept(
        draft,
        &snapshot,
        &contract::Generation {
            analysis_ref: receipt.request_ref,
            skill_version_ref: "synthetic-upstream".into(),
            prompt_version_ref: row.get::<Uuid, _>("prompt_version_ref").to_string(),
            model_config_ref: config.to_string(),
            provider_returned_model: "synthetic-upstream-no-call".into(),
            created_at: preview["knowledgeCutoff"].as_str().unwrap().into(),
        },
    )
    .unwrap();
    let mut tx = executor.fence().begin(db, true).await.unwrap();
    let inv = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,m.connection_version_ref,m.model_ref,$2,'analyze',$3,'succeeded',0,0,'{\"synthetic\":true}',scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2").bind(inv).bind(config).bind("c".repeat(64)).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=1,lease_token=$2,lease_instance_ref=$3,lease_instance_generation=$4,lease_expires_at=clock_timestamp()+interval '1 minute' WHERE item_ref=$1").bind(receipt.item_ref).bind(Uuid::new_v4()).bind(executor.fence().instance_ref()).bind(executor.fence().generation()).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control ctl SET invocation_ref=$2,attempt_count=1,executed_config_ref=cfg.task_config_ref,executed_skill_version_ref=cfg.skill_version_ref,execution_contract=cfg.execution_contract FROM linggan_ai_task_config cfg WHERE ctl.item_ref=$1 AND cfg.task_config_ref=$3").bind(receipt.item_ref).bind(inv).bind(row.get::<Uuid,_>("task_config_ref")).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_content_understanding_result(request_ref,work_ref,material_snapshot_identity,method_version,invocation_ref,result) VALUES($1,$2,$3,$4,$5,$6)").bind(receipt.request_ref).bind(work).bind(snapshot.material_snapshot_identity).bind(contract::METHOD).bind(inv).bind(accepted).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='succeeded',usable=true,output='{\"synthetic\":true}',finished_at=scope_001_now(),lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL WHERE item_ref=$1").bind(receipt.item_ref).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}
