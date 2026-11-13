//! Synthetic bindings prove the saved-parent reader cannot resurrect a superseded parent.
use super::saved_parent;
#[path = "../../../../evidence/tests/support/material_fixture.rs"]
mod fixture;
use uuid::Uuid;

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic bindings only"]
async fn a_latest_binding_without_a_parent_never_falls_back_to_an_older_parent() {
    let db = fixture::proof_database("topic_saved_parent").await;
    let domain: Uuid = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    let (child, parent, parent_definition) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key) VALUES($1,'domain-synthetic','saved-parent-child'),($2,'domain-synthetic','saved-parent-upper')")
        .bind(child).bind(parent).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) VALUES($1,$2,1,'合成上层主题','合成上层主题定义','provisional')")
        .bind(parent_definition).bind(parent).execute(&mut *tx).await.unwrap();
    // Version 1 still names a parent; version 2 deliberately clears it.
    sqlx::query("INSERT INTO linggan_topic_map_receipt(receipt_ref,idempotency_key,request_sha256,action,subject_ref,revision) VALUES($1,'synthetic-saved-parent-v1',$2,'synthetic',$3,1),($4,'synthetic-saved-parent-v2',$2,'synthetic',$3,2)")
        .bind(Uuid::new_v4()).bind("0".repeat(64)).bind(child).bind(Uuid::new_v4())
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref) SELECT $1,$2,$3,$4,1,receipt_ref FROM linggan_topic_map_receipt WHERE idempotency_key='synthetic-saved-parent-v1'")
        .bind(Uuid::new_v4()).bind(child).bind(domain).bind(parent)
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref) SELECT $1,$2,$3,NULL,2,receipt_ref FROM linggan_topic_map_receipt WHERE idempotency_key='synthetic-saved-parent-v2'")
        .bind(Uuid::new_v4()).bind(child).bind(domain)
        .execute(&mut *tx).await.unwrap();
    assert_eq!(saved_parent(&mut tx, child).await.unwrap(), None);
    tx.rollback().await.unwrap();
}
