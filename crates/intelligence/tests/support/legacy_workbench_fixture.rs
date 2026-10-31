//! Historical DTO fixtures; synthetic data only, owned isolated databases.
use linggan_intelligence::content_workbench::{WorkDocument, recovery};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn hash(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub async fn legacy(db: &Database, document: WorkDocument, count: i64) -> (Uuid, Vec<Value>) {
    let mut raw = serde_json::to_value(&document).unwrap();
    for excerpt in raw["excerpts"].as_array_mut().unwrap() {
        let citation = excerpt["citation"].as_object_mut().unwrap();
        citation.remove("blockRef");
        citation.remove("asOf");
        if citation["sourceKind"] == "note" {
            let source = Uuid::parse_str(citation["sourceRef"].as_str().unwrap()).unwrap();
            let (created, body): (String, String) = sqlx::query_as("SELECT linggan_human_moment(created_at),body_text FROM linggan_material_content_detail WHERE material_ref=$1").bind(source).fetch_one(db.pool()).await.unwrap();
            citation.insert(
                "sourceVersion".into(),
                json!(hash(
                    serde_json::to_vec(&json!([created, hash(body)])).unwrap()
                )),
            );
        }
    }
    let digest = recovery::legacy_document_sha256(raw.clone()).unwrap();
    let work = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_creation_work(work_ref,domain_ref,saved_by) VALUES($1,$2,'local_machine')").bind(work).bind(Uuid::from_u128(0x00000000000040008000000000000001)).execute(db.pool()).await.unwrap();
    let mut previous: Option<Uuid> = None;
    for number in 1..=count {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_creation_revision(revision_ref,work_ref,revision_number,previous_revision_ref,document,document_sha256) VALUES($1,$2,$3,$4,$5,$6)").bind(id).bind(work).bind(number).bind(previous).bind(&raw).bind(&digest).execute(db.pool()).await.unwrap();
        sqlx::query("UPDATE linggan_creation_work SET current_revision_ref=$2 WHERE work_ref=$1")
            .bind(work)
            .bind(id)
            .execute(db.pool())
            .await
            .unwrap();
        previous = Some(id);
    }
    (work, snapshot(db, work).await)
}
pub async fn snapshot(db: &Database, work: Uuid) -> Vec<Value> {
    sqlx::query_scalar("SELECT to_jsonb(r) FROM linggan_creation_revision r WHERE work_ref=$1 ORDER BY revision_number").bind(work).fetch_all(db.pool()).await.unwrap()
}
