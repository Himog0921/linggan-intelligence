//! Same worker execution entry and existing media admission, lease and dossier readers.
//! Private holdout data is supplied only on the local machine and never printed.
#![allow(dead_code)]
#[path = "../../../crates/evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "../src/bin/media_worker_support/mod.rs"]
mod support;
use linggan_evidence::{ocr_adoption_persistence as store, *};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

async fn database(schema: &str) -> Database {
    let db = fixture::proof_database(schema).await;
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0142_ocr_adoption.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    db
}
pub(crate) async fn admit(
    db: &Database,
    work: &str,
    ordinal: u32,
    bytes: &[u8],
    mime: &str,
) -> Uuid {
    admit_with_slot_role(db, work, ordinal, bytes, mime, "image").await
}
pub(crate) async fn admit_with_slot_role(
    db: &Database,
    work: &str,
    ordinal: u32,
    bytes: &[u8],
    mime: &str,
    slot_role: &str,
) -> Uuid {
    let present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_content WHERE content_external_id=$1)",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    if !present {
        fixture::submit_package(db,"content_detail",json!({"contentExternalId":work}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":work},"payload":{"title":"Synthetic replay metadata","bodyText":"Native body must remain independent."}})).await;
    }
    let observation = Uuid::new_v4();
    let (slot, source_type, source_external_id, context) = match slot_role {
        "avatar" => (
            format!("xhs:author:synthetic-author:avatar:{ordinal}"),
            "author",
            "synthetic-author".to_owned(),
            Some(work),
        ),
        "comment_image" => (
            format!("xhs:comment:synthetic-comment-{ordinal}:comment_image:{ordinal}"),
            "comment",
            format!("synthetic-comment-{ordinal}"),
            Some(work),
        ),
        _ => (
            format!("xhs:{work}:image:{ordinal}"),
            "content",
            work.to_owned(),
            None,
        ),
    };
    let uri = format!("https://media.example/{work}/{ordinal}.jpg");
    let mut record = json!({"kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":slot_role,"ordinal":ordinal},"observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-21T00:00:00Z"},"sourceObject":{"platform":"xhs","type":source_type,"externalId":source_external_id}});
    if let Some(context) = context {
        record["contextContentExternalId"] = json!(context);
    }
    fixture::submit_package(db, "media_slots", json!({"contentExternalId":work}), record).await;
    let hash = support::sha256_hex(bytes);
    let key = format!("blobs/{hash}");
    let path = support::prepare_output(&key).unwrap();
    std::fs::write(path, bytes).unwrap();
    admit_media_blob(db, observation, &hash, mime, bytes.len() as i64, &key)
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'image_ocr',$4,'full_blob')")
        .bind(Uuid::new_v4()).bind(hash).bind(slot).bind(store::PROCESSOR_VERSION).execute(db.pool()).await.unwrap();
    ensure_media_processing_work(db).await.unwrap();
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap()
}
pub(crate) async fn execute_one(db: &Database) -> MediaProcessingClaim {
    let worker = Uuid::new_v4();
    let MediaProcessingClaimOutcome::Claimed(claim) = claim_media_processing_work_for_version(
        db,
        worker,
        &["image_ocr".into()],
        Some(store::PROCESSOR_VERSION),
    )
    .await
    .unwrap() else {
        panic!("owned proof job must be claimable")
    };
    support::ocr_vl::execute(db, worker, &claim)
        .await
        .expect("bounded local OCR worker completes");
    claim
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL runner only"]
async fn synthetic_worker_persists_only_adopted_text_with_exact_sources() {
    assert!(std::env::var_os("OCR_CHAIN_MANIFEST").is_none());
    let script = PathBuf::from(std::env::var_os("LINGGAN_PADDLE_VL_SCRIPT").unwrap());
    std::fs::write(&script, SYNTHETIC_BRIDGE).unwrap();
    let db = database("ocr_adoption_synthetic").await;
    let work = admit(&db, "ocr-synthetic", 1, b"synthetic-cover", "image/png").await;
    let before: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    execute_one(&db).await;
    let mut frozen_tx = db.pool().begin().await.unwrap();
    let old = research_dossier::read_in(&mut frozen_tx, &[work], &before)
        .await
        .unwrap()
        .remove(&work)
        .unwrap();
    assert!(
        old.media.iter().all(|m| m["ocrAdoption"].is_null()),
        "a later OCR result cannot enter a frozen cutoff"
    );
    frozen_tx.rollback().await.unwrap();
    admit(&db, "ocr-synthetic", 2, b"synthetic-interior", "image/png").await;
    execute_one(&db).await;
    let d = research_dossier::read(&db, work).await.unwrap().unwrap();
    assert!(
        d.fragments
            .iter()
            .any(|f| f.field == "body" && f.text == "Native body must remain independent.")
    );
    assert!(
        d.fragments
            .iter()
            .filter(|f| f.field == "ocr")
            .all(|f| !f.text.contains("CANARY-UNCONFIRMED"))
    );
    assert_eq!(
        d.media
            .iter()
            .filter(|m| m["ocrAdoption"].is_object())
            .count(),
        2
    );
    let p = &d.image_text_projection["projection"];
    assert!(p["imageText"].as_str().unwrap().contains("Synthetic body"));
    assert_eq!(p["images"].as_array().unwrap().len(), 2);
    assert!(p["coverTitle"].is_string());
    assert_eq!(
        d.media
            .iter()
            .find(|m| m["slotOrdinal"] == 2 && m["ocrAdoption"].is_object())
            .unwrap()["ocrAdoption"]["assessment"]["coverTitle"]["state"],
        "not_cover"
    );
    let counts:Vec<i64>=sqlx::query_scalar("SELECT count(*) FROM linggan_media_ocr_adoption UNION ALL SELECT count(*) FROM linggan_material_derived_text").fetch_all(db.pool()).await.unwrap();
    assert_eq!(counts, vec![2, 2]);
    let raw: Vec<Value> = sqlx::query_scalar("SELECT observation FROM linggan_media_ocr_adoption")
        .fetch_all(db.pool())
        .await
        .unwrap();
    assert!(
        raw.iter()
            .all(|v| v["layout"]["blocks"][2]["text"] == "CANARY-UNCONFIRMED")
    );
    assert!(
        raw.iter()
            .all(|v| v["layout"]["terminations"][2]["finishReason"] == "length")
    );
    assert!(
        sqlx::query("UPDATE linggan_media_ocr_adoption SET image_text='overwrite'")
            .execute(db.pool())
            .await
            .is_err()
    );
    // No claim may write a second result after the lease has finished.
    let idle = claim_media_processing_work_for_version(
        &db,
        Uuid::new_v4(),
        &["image_ocr".into()],
        Some(store::PROCESSOR_VERSION),
    )
    .await
    .unwrap();
    assert!(matches!(idle, MediaProcessingClaimOutcome::Idle));
    let query:linggan_contracts::EvidenceQuery=serde_json::from_value(json!({"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery","text":"Synthetic body"})).unwrap();
    let page = read_work_resources(&db, &query).await.unwrap();
    assert!(
        page.items.iter().any(|i| i
            .evidence_fragment
            .as_ref()
            .is_some_and(|f| f.text.contains("Synthetic body"))),
        "the public Corpus reader reaches the adopted image text"
    );
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,state,authority_ref,reason,effective_at) VALUES($1,'xhs:ocr-synthetic:image:2','WITHDRAWN_OR_RESTRICTED','synthetic-ocr-proof','synthetic restriction',clock_timestamp())").bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    let restricted = research_dossier::read(&db, work).await.unwrap().unwrap();
    assert!(
        restricted
            .media
            .iter()
            .filter(|m| m["slotOrdinal"] == 2)
            .all(|m| m["ocrAdoption"].is_null() && m["researchText"].is_null()),
        "restricted observations cannot enter the adopted-text projection"
    );
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,state,authority_ref,reason,effective_at) VALUES($1,'xhs:ocr-synthetic:image:1','WITHDRAWN_OR_RESTRICTED','synthetic-ocr-proof','synthetic restriction',clock_timestamp())").bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    let page = read_work_resources(&db, &query).await.unwrap();
    assert!(
        page.items.iter().all(|i| i
            .evidence_fragment
            .as_ref()
            .is_none_or(|f| !f.text.contains("Synthetic body"))),
        "the public Corpus reader must not recover restricted OCR via derived_text"
    );
    println!(
        "synthetic worker → lease → raw/layout/adoption → dossier/Corpus, cutoff and restriction proof passed"
    );
}

#[tokio::test]
#[ignore = "explicit private local manifest, local model and disposable PostgreSQL only"]
async fn private_unseen_images_use_actual_worker_and_database() {
    assert!(
        std::env::var_os("GITHUB_ACTIONS").is_none(),
        "private inputs are forbidden in CI"
    );
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("OCR_CHAIN_MANIFEST").unwrap()).unwrap(),
    )
    .unwrap();
    let out = PathBuf::from(std::env::var_os("OCR_CHAIN_OUTPUT").unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let db = database("ocr_adoption_private").await;
    let mut works = std::collections::BTreeSet::new();
    let start = std::time::Instant::now();
    let mut receipts = vec![];
    for image in manifest["images"].as_array().unwrap() {
        let bytes = std::fs::read(image["path"].as_str().unwrap()).unwrap();
        assert_eq!(
            support::sha256_hex(&bytes),
            image["sha256"].as_str().unwrap()
        );
        let work = admit(
            &db,
            image["workKey"].as_str().unwrap(),
            image["ordinal"].as_u64().unwrap() as u32,
            &bytes,
            image["mimeType"].as_str().unwrap(),
        )
        .await;
        let elapsed = std::time::Instant::now();
        let worker = Uuid::new_v4();
        let MediaProcessingClaimOutcome::Claimed(claim) = claim_media_processing_work_for_version(
            &db,
            worker,
            &["image_ocr".into()],
            Some(store::PROCESSOR_VERSION),
        )
        .await
        .unwrap() else {
            panic!("private owned proof claim")
        };
        if let Err(reason) = support::ocr_vl::execute(&db, worker, &claim).await {
            fail_media_processing_work(&db, &claim, worker, &reason)
                .await
                .unwrap();
            // This one-pass holdout does not rerun failures. Only its owned work lease changes.
            sqlx::query("UPDATE linggan_media_processing_work SET next_attempt_at=scope_001_now()+interval '1 day' WHERE job_ref=$1").bind(claim.job_ref).execute(db.pool()).await.unwrap();
            let failure =
                json!({"id":image["id"],"seconds":elapsed.elapsed().as_secs_f64(),"error":reason});
            std::fs::write(
                out.join(format!("{}.failure.json", image["id"].as_str().unwrap())),
                serde_json::to_vec_pretty(&failure).unwrap(),
            )
            .unwrap();
            receipts.push(failure);
            println!(
                "private local worker images attempted: {} (bounded failure retained)",
                receipts.len()
            );
            continue;
        }
        works.insert(work);
        let row=sqlx::query("SELECT a.observation,a.assessment,a.image_text,a.cover_title,a.adoption_ref,a.derivative_ref,a.layout_ref,d.storage_key FROM linggan_media_ocr_adoption a JOIN linggan_media_derivative d USING(derivative_ref) WHERE d.job_ref=$1").bind(claim.job_ref).fetch_one(db.pool()).await.unwrap();
        let raw_path = support::local_media_root().join(row.get::<String, _>("storage_key"));
        let saved = std::fs::read(raw_path).unwrap();
        let p = store::prepare(&saved).unwrap();
        assert_eq!(p.image_text, row.get::<String, _>("image_text"));
        assert_eq!(
            p.assessment.image.blob_sha256,
            image["sha256"].as_str().unwrap()
        );
        let receipt = json!({"id":image["id"],"workRef":work,"seconds":elapsed.elapsed().as_secs_f64(),"adoptionRef":row.get::<Uuid,_>("adoption_ref"),"derivativeRef":row.get::<Uuid,_>("derivative_ref"),"layoutRef":row.get::<Uuid,_>("layout_ref"),"observation":row.get::<Value,_>("observation"),"assessment":row.get::<Value,_>("assessment"),"imageText":p.image_text,"coverTitle":p.cover_title});
        std::fs::write(
            out.join(format!("{}.raw.json", image["id"].as_str().unwrap())),
            saved,
        )
        .unwrap();
        std::fs::write(
            out.join(format!("{}.json", image["id"].as_str().unwrap())),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        receipts.push(json!({"id":image["id"],"seconds":elapsed.elapsed().as_secs_f64()}));
        println!("private local worker images completed: {}", receipts.len());
    }
    for work in &works {
        let dossier = research_dossier::read(&db, *work).await.unwrap().unwrap();
        assert!(dossier.media.iter().any(|m| m["ocrAdoption"].is_object()));
        std::fs::write(
            out.join(format!("work-{work}.json")),
            serde_json::to_vec_pretty(&dossier).unwrap(),
        )
        .unwrap();
    }
    let summary = json!({"images":receipts.len(),"works":works.len(),"seconds":start.elapsed().as_secs_f64(),"failures":receipts.iter().filter(|r|r["error"].is_string()).count(),"receipts":receipts,"database":"owned_disposable_postgresql","deployed":false});
    std::fs::write(
        out.join("chain-summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!(
        "private local worker/database proof completed: {} images / {} works",
        receipts.len(),
        works.len()
    );
}

#[test]
#[ignore = "explicit locally saved bridge bundles only; no database or inference"]
fn private_saved_bridge_receipts_are_bound() {
    assert!(std::env::var_os("GITHUB_ACTIONS").is_none());
    let dir = PathBuf::from(std::env::var_os("OCR_SAVED_BRIDGES_DIR").unwrap());
    for name in ["B10", "B32"] {
        let bytes = std::fs::read(dir.join(format!("{name}.json"))).unwrap();
        let p = store::prepare(&bytes)
            .unwrap_or_else(|e| panic!("saved {name} receipt validation: {e:?}"));
        assert!(p.observation.layout.require_termination);
        let mut bridge: Value = serde_json::from_slice(&bytes).unwrap();
        bridge["observation"]["layout"]["terminations"][0]["finishReason"] = json!("length");
        assert!(
            store::prepare(&serde_json::to_vec(&bridge).unwrap()).is_err(),
            "finish reason cannot be changed independently of raw response"
        );
    }
}

pub(crate) const SYNTHETIC_BRIDGE: &str = r#"
import sys,json,hashlib
def encoded(x):return json.dumps(x,sort_keys=True,ensure_ascii=False,separators=(',',':'))
def sha(x):return hashlib.sha256(x.encode()).hexdigest()
m=json.load(open(sys.argv[2]));i=m['image'];i.update(width=100,height=100)
texts=['Synthetic title','Synthetic body','CANARY-UNCONFIRMED'];boxes=[[5,5,95,15],[5,30,95,40],[5,60,95,70]]
blocks=[dict(block_id=n,block_label='text',block_content=t,block_bbox=boxes[n])for n,t in enumerate(texts)]
anchors=dict(res=dict(rec_texts=texts[:2],rec_scores=[.99,.99],dt_polys=[[[b[0],b[1]],[b[2],b[1]],[b[2],b[3]],[b[0],b[3]]]for b in boxes[:2]]))
generations=[];terms=[]
for n,t in enumerate(texts):
 finish='length' if n==2 else 'stop';request='synthetic-'+str(n)
 response=dict(rawText=t,rawOutputSha256=sha(t),finishReason=finish,requestRef=request,generatedTokens=3,timeoutSeconds=45,maxTokens=4096,inputImageSha256='a'*64,rawResponse=dict(id=request,choices=[dict(finish_reason=finish,message=dict(content=t))]))
 generations.append(dict(label='text',bbox=boxes[n],response=response));terms.append(dict(blockRef=str(n),outputSha256=sha(t),finishReason=finish,requestRef=request,rawOutputSha256=sha(t),generatedTokens=3))
a=encoded(anchors);v=encoded(dict(paddle=dict(res=dict(width=100,height=100,parsing_res_list=blocks)),generations=generations,rechecks=[]))
ob=dict(schemaVersion=1,image=i,imageRole=m['imageRole'],sequenceOrdinal=m['sequenceOrdinal'],anchors=dict(image=i,pipeline=dict(version='synthetic-anchor',rawSha256=sha(a)),lines=[dict(lineRef=str(n),readingOrder=n,text=t,confidence=.99,bbox=boxes[n])for n,t in enumerate(texts[:2])]),layout=dict(image=i,pipeline=dict(version='synthetic-vl',rawSha256=sha(v)),completed=True,requireTermination=True,blocks=[dict(blockRef=str(n),readingOrder=n,label='text',text=t,bbox=boxes[n])for n,t in enumerate(texts)],terminations=terms),rechecks=[])
print(encoded(dict(schemaVersion=2,ok=True,observation=ob,anchorRawJson=a,vlRawJson=v)))
"#;
