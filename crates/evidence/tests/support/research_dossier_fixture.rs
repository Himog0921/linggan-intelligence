#[path = "material_fixture.rs"]
pub mod fixture;
use linggan_evidence::*;
use serde_json::json;
use uuid::Uuid;
pub const DOMAIN: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
pub async fn work(db: &linggan_storage_postgres::Database) -> Uuid {
    let package=fixture::submit_package(db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成底稿验收","bodyText":format!("{}结尾不能丢失🙂", "完整正文。".repeat(900)),"authorId":"synthetic-author","likedCount":0}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id='dossier-note'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    work
}

pub async fn current_image_reading(db: &linggan_storage_postgres::Database) -> Uuid {
    let preview_bytes = std::env::var("DOSSIER_PROOF_IMAGE")
        .ok()
        .map(|path| std::fs::read(path).unwrap());
    current_image_reading_with_bytes(db, preview_bytes).await
}

pub async fn acquired_image_source(
    db: &linggan_storage_postgres::Database,
) -> (Uuid, Uuid, String) {
    acquired_image_source_with_bytes(db, None).await
}
pub async fn acquired_image_source_with_bytes(
    db: &linggan_storage_postgres::Database,
    preview_bytes: Option<Vec<u8>>,
) -> (Uuid, Uuid, String) {
    let observation = Uuid::new_v4();
    let target = json!({"contentExternalId":"dossier-note","imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":1}});
    fixture::submit_custom_package(db,"xhs",&["media_slots"],json!({"contentExternalId":"dossier-note"}),"media_slots","xhs",
        json!({"target":target,"layers":[fixture::coverage_layer("media_slots",1)]}),
        vec![json!({"kind":"media_slot","slotKey":"xhs:dossier-note:image:1","observationRef":observation,
            "slot":{"role":"image","ordinal":1,"displayOrdinal":1,"displayOrderBasis":"platform_explicit"},
            "observation":{"externalUri":"https://media.example/dossier.jpg","candidateUris":["https://media.example/dossier.jpg"],"observedAt":"2026-10-14T00:00:00Z"},
            "sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"}})]).await;
    let (sha, size) = preview_bytes.as_ref().map_or_else(
        || ("a".repeat(64), 12),
        |bytes| {
            use sha2::{Digest, Sha256};
            (
                Sha256::digest(bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                bytes.len() as i64,
            )
        },
    );
    let (mime_type, storage_key) = if preview_bytes
        .as_ref()
        .is_some_and(|bytes| bytes.starts_with(b"\x89PNG\r\n\x1a\n"))
    {
        ("image/png", format!("blobs/{sha}.png"))
    } else {
        ("image/jpeg", format!("blobs/{sha}.jpg"))
    };
    if let Some(bytes) = preview_bytes {
        let root = std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT")
            .expect("synthetic browser image requires an owned local media root");
        let path = std::path::PathBuf::from(root).join(&storage_key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    admit_media_blob(db, observation, &sha, mime_type, size, &storage_key)
        .await
        .unwrap();
    let row:(Uuid,Uuid)=sqlx::query_as("SELECT o.content_public_ref,m.materialization_ref FROM linggan_material_media_origin o JOIN linggan_media_download_attempt a ON a.media_observation_ref=o.observation_ref JOIN linggan_media_materialization m USING(download_attempt_ref) WHERE o.observation_ref=$1 ORDER BY m.verified_at DESC LIMIT 1").bind(observation).fetch_one(db.pool()).await.unwrap();
    (row.0, row.1, sha)
}

pub async fn current_image_reading_with_bytes(
    db: &linggan_storage_postgres::Database,
    preview_bytes: Option<Vec<u8>>,
) -> Uuid {
    let (work, version, sha) = acquired_image_source_with_bytes(db, preview_bytes).await;
    let row = (work, version);
    let mut blocks:Vec<_>=(0..4).map(|i|json!({"kind":"paragraph","text":format!("第{i}行连续的图片正文，完整可用。"),"speaker":"unconfirmed"})).collect();
    blocks.push(json!({"kind":"heading","text":"封面大字独立保留","speaker":"unconfirmed"}));
    blocks.push(json!({"kind":"paragraph","text":"用途不清的背景文字","speaker":"unconfirmed","issues":[{"key":"background","code":"uncertain_reading","message":"背景文字，归属待核验","resolution":"reading_edit"}]}));
    let mut tx = db.pool().begin().await.unwrap();
    let reading=material_reading::publish_in(&mut tx,material_reading::NewReading {
        content_public_ref:row.0,unit_key:"image:xhs:dossier-note:image:1".into(),kind:"image".into(),slot_key:Some("xhs:dossier-note:image:1".into()),page:Some(1),source_version:row.1.to_string(),source_state:"present".into(),derivative_ref:None,producer_ref:Some(Uuid::new_v4()),generated_by:"cloud_vision".into(),rule_version:"dossier-content.v2:synthetic-current".into(),blocks,issues:vec![],source_location:json!({"slotKey":"xhs:dossier-note:image:1","materializationRef":row.1,"blobSha256":sha,"sourceVersion":row.1}),publication:material_reading::Publication::Automatic{expected_current:None}
    }).await.unwrap();
    tx.commit().await.unwrap();
    reading.reading_ref
}

#[allow(dead_code)]
pub async fn model_config(db: &linggan_storage_postgres::Database) {
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Synthetic preview only','openai-completions','http://127.0.0.1:1',true,$3)").bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-preview','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,8192,1000,30,2)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,config_ref,daily_token_limit) VALUES($1,'xhs',$2,100000)").bind(DOMAIN).bind(config).execute(db.pool()).await.unwrap();
}
