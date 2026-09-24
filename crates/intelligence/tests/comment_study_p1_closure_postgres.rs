//! P1 closure proofs use only synthetic material in the disposable proof database.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use linggan_intelligence::comment_study_catalog::{
    CommentDetailQuery, maintain_comment_catalog, read_comment_detail, refresh_clean_cache,
};
use linggan_intelligence::comment_study_source::{ADHD_DOMAIN_REF, eligible_sources_for_works};
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use serde_json::{Value, json};
use uuid::Uuid;

async fn database(name: &str) -> Database {
    let db=fixture::proof_database(name).await;
    sqlx::raw_sql(include_str!("../../../database/bootstrap/comment-study-001.sql"))
        .execute(db.pool()).await.unwrap();
    sqlx::raw_sql(include_str!("../../../database/migrations/0103_comment_study_productization_schema.sql"))
        .execute(db.pool()).await.unwrap();
    db
}
fn domain()->Uuid { Uuid::parse_str(ADHD_DOMAIN_REF).unwrap() }
async fn as_of(db:&Database)->String {
    sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(db.pool()).await.unwrap()
}
async fn work(db:&Database,source:Uuid)->Uuid {
    sqlx::query_scalar("SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1")
        .bind(source).fetch_one(db.pool()).await.unwrap()
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL"]
async fn p1_directory_and_frozen_source_share_parent_and_work_context() {
    let db=database("p1_shared_context").await;
    detail_with_author(&db,"shared-context","SYNTHETIC 作品全文",Some("creator")).await;
    comment_with_author(&db,"shared-context","parent","SYNTHETIC 父语境",Some("creator"),"2026-09-21T08:00:00Z").await;
    let source=reply_with_author(&db,"shared-context","child","parent","我也是",Some("reader"),"2026-09-21T09:00:00Z").await;
    let work=work(&db,source).await;
    refresh_clean_cache(&db,domain(),128).await.unwrap();
    let detail=read_comment_detail(&db,&CommentDetailQuery{domain:domain(),work_ref:work,comment_external_id:"child".into()}).await.unwrap();
    let selected=eligible_sources_for_works(&db,domain(),&as_of(&db).await,&[work],100).await.unwrap();
    assert_eq!(selected.len(),1);
    assert_eq!(selected[0].parent_research_text.as_deref(),Some("SYNTHETIC 父语境"));
    assert_eq!(detail["parentContext"]["researchText"].as_str(),selected[0].parent_research_text.as_deref());
    assert_eq!(detail["work"]["contextManifest"],selected[0].context_manifest);
    assert!(detail["work"]["contextManifest"]["sources"].as_array().unwrap().iter().any(|s|s["kind"]=="body"));
    fixture::submit_package_at(&db,"comments",json!({"contentExternalId":"shared-context"}),json!({
        "kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"shared-context"},
        "payload":{"noteId":"shared-context","commentId":"parent","authorId":"creator"}
    }),"2026-09-22T08:00:00Z").await;
    let selected=eligible_sources_for_works(&db,domain(),&as_of(&db).await,&[work],100).await.unwrap();
    assert_eq!(selected.len(),1);
    assert!(selected[0].parent_research_text.is_none(),"new UNKNOWN must not revive prior parent");
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL"]
async fn p1_maintenance_is_bounded_local_and_completes_late_unindexed_rows() {
    let db=database("p1_cache_tick").await;
    detail_with_author(&db,"cache-tick","SYNTHETIC 缓存",Some("creator")).await;
    for i in 0..130 {
        comment_with_author(&db,"cache-tick",&format!("c{i}"),"SYNTHETIC 有效评论",Some("reader"),"2026-09-21T08:00:00Z").await;
    }
    let first=maintain_comment_catalog(&db).await.unwrap().unwrap();
    assert_eq!(first.examined_count,128);
    let second=maintain_comment_catalog(&db).await.unwrap().unwrap();
    assert_eq!(second.inserted_count,2);
    let third=maintain_comment_catalog(&db).await.unwrap().unwrap();
    assert_eq!(third.examined_count,0);
    let effects:Value=sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_comment_study_run),(SELECT count(*) FROM linggan_model_invocation))")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(effects,json!([0,0]));
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL"]
async fn p1_source_pages_past_ineligible_rows_without_loading_all_contexts() {
    let db=database("p1_source_windows").await;
    detail_with_author(&db,"source-window","SYNTHETIC source",Some("creator")).await;
    let valid=comment_with_author(&db,"source-window","valid","SYNTHETIC 用户",Some("reader"),"2026-09-21T08:00:00Z").await;
    for i in 0..130 {
        comment_with_author(&db,"source-window",&format!("noise{i}"),"😀😀",Some("reader"),"2026-09-22T08:00:00Z").await;
    }
    let sources=eligible_sources_for_works(&db,domain(),&as_of(&db).await,&[work(&db,valid).await],1).await.unwrap();
    assert_eq!(sources.len(),1);assert_eq!(sources[0].source_ref,valid);
}
