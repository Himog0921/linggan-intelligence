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

pub async fn ordinary_ocr(db: &linggan_storage_postgres::Database) -> Uuid {
    let preview_bytes = std::env::var("DOSSIER_PROOF_IMAGE")
        .ok()
        .map(|path| std::fs::read(path).unwrap());
    ordinary_ocr_with_bytes(db, preview_bytes).await
}

pub async fn ordinary_ocr_with_bytes(
    db: &linggan_storage_postgres::Database,
    preview_bytes: Option<Vec<u8>>,
) -> Uuid {
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
        ("image/png", "blobs/dossier.png")
    } else {
        ("image/jpeg", "blobs/dossier.jpg")
    };
    if let Some(bytes) = preview_bytes {
        let root = std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT")
            .expect("synthetic browser image requires an owned local media root");
        let path = std::path::PathBuf::from(root).join(storage_key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    admit_media_blob(db, observation, &sha, mime_type, size, storage_key)
        .await
        .unwrap();
    ensure_media_processing_work(db).await.unwrap();
    let worker = Uuid::new_v4();
    let MediaProcessingClaimOutcome::Claimed(claim) =
        claim_media_processing_work(db, worker, &["image_ocr".into()])
            .await
            .unwrap()
    else {
        panic!("OCR claim");
    };
    let mut lines: Vec<_> = (0..4)
        .map(|i| OcrLineInput {
            text: format!("第{i}行连续的图片正文，完整可用。"),
            confidence: 0.98,
            bbox_norm: [
                0.12,
                0.28 + f64::from(i) * 0.05,
                0.88,
                0.305 + f64::from(i) * 0.05,
            ],
        })
        .collect();
    lines.push(OcrLineInput {
        text: "封面大字独立保留".into(),
        confidence: 0.98,
        bbox_norm: [0.1, 0.10, 0.85, 0.17],
    });
    lines.push(OcrLineInput {
        text: "用途不清的背景文字".into(),
        confidence: 0.6,
        bbox_norm: [0.0, 0.9, 0.2, 0.92],
    });
    let raw = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    complete_media_processing_ocr(
        db,
        &claim,
        worker,
        &OcrCompletionInput {
            engine_version: "synthetic-paddle".into(),
            image_width: 100,
            image_height: 100,
            raw_text: raw,
            raw_content_hash: "b".repeat(64),
            raw_storage_key: "derived/dossier.txt".into(),
            layout_content_hash: "c".repeat(64),
            layout_byte_size: 12,
            layout_storage_key: "derived/dossier.json".into(),
            lines,
            layering: OcrLayeringInput {
                layer_version: "rules-v1".into(),
                state: "PARTIAL".into(),
                cover_headline: None,
                image_substantive_text: None,
                retained_ordinals: vec![0, 1, 2, 3, 4, 5],
                headline_ordinals: vec![],
                substantive_ordinals: vec![],
                excluded_lines: vec![],
            },
        },
    )
    .await
    .unwrap()
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
