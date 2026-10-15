#![recursion_limit = "256"]
use linggan_intelligence::{content_workbench::types::*, content_workbench_sources as sources};
use serde_json::json;
use uuid::Uuid;
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
use fixture::dossier::{self, DOMAIN};

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn exact_sources_keep_domain_version_parent_and_ocr_image_identity() {
    let db = fixture::db("creation_sources").await;
    let doc = fixture::cited_doc(&db).await;
    let c = doc.excerpts[0].citation.clone();
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(
        sources::validate_citation_in_domain(&mut tx, &c, DOMAIN)
            .await
            .unwrap()
            .selected_text,
        doc.excerpts[0].selected_text.clone().unwrap()
    );
    assert!(
        sources::validate_citation_in_domain(&mut tx, &c, Uuid::new_v4())
            .await
            .is_err()
    );
    let mut forged = c.clone();
    forged.text_hash = "0".repeat(64);
    assert!(
        sources::validate_citation_in_domain(&mut tx, &forged, DOMAIN)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let work = c.work_ref.unwrap();
    let derivative = dossier::legacy_ocr(&db).await;
    let before = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    linggan_evidence::research_dossier::relayer_work(&db, work, &before.revision)
        .await
        .unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let (source, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Ocr,
        derivative,
        Some(work),
        &SourceField::Ocr,
    )
    .await
    .unwrap();
    let location = source.location.unwrap();
    assert_eq!(location.derivative_ref, derivative);
    assert!(!location.line_spans.is_empty());
    assert!(location.line_spans[0].bbox[0] > 0.);
    assert!((location.line_spans[0].bbox[2] - 0.76).abs() < 1e-6);
    assert!(
        context["dossier"]["media"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["derivativeRef"] == json!(derivative) && m["originalAssetUrl"].is_string())
    );
    tx.commit().await.unwrap();
    for (kind, id, parent) in [
        ("comments", "parent", None),
        ("replies", "reply", Some("parent")),
        ("replies", "unknown-reply", Some("not-captured")),
    ] {
        let mut payload =
            json!({"commentId":id,"noteId":"dossier-note","text":format!("{id}原声😀\n确切全文")});
        if let Some(parent) = parent {
            payload["rootCommentId"] = json!(parent);
            payload["parentCommentId"] = json!(parent);
        }
        dossier::fixture::submit_package(&db,kind,json!({"contentExternalId":"dossier-note"}),json!({"kind":if kind=="comments"{"comment"}else{"reply"},"sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":payload})).await;
    }
    let reply:Uuid=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE comment_external_id='reply'").fetch_one(db.pool()).await.unwrap();
    let unknown:Uuid=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE comment_external_id='unknown-reply'").fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let (_, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Comment,
        reply,
        Some(work),
        &SourceField::CommentBody,
    )
    .await
    .unwrap();
    assert_eq!(context["parentComment"]["state"], "known");
    assert_eq!(context["parentWork"]["state"], "known");
    assert!(!context.to_string().contains("authorId"));
    let (_, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Comment,
        unknown,
        Some(work),
        &SourceField::CommentBody,
    )
    .await
    .unwrap();
    assert_eq!(context["parentComment"]["state"], "unknown");
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'parent','synthetic restriction')").bind(work).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let (_, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Comment,
        reply,
        Some(work),
        &SourceField::CommentBody,
    )
    .await
    .unwrap();
    assert_eq!(context["parentComment"]["state"], "restricted");
    assert!(context["parentComment"]["body"].is_null());
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'reply','synthetic restriction')").bind(work).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(matches!(
        sources::read_source_in(
            &mut tx,
            &SourceKind::Comment,
            reply,
            Some(work),
            &SourceField::CommentBody
        )
        .await,
        Err(sources::SourceError::Restricted)
    ));
    tx.commit().await.unwrap();
    dossier::fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成新版本","bodyText":"新版本正文","authorId":"synthetic-author"}}),"2026-08-29T10:00:00Z").await;
    let mut tx = db.pool().begin().await.unwrap();
    assert!(matches!(
        sources::validate_citation_in_domain(&mut tx, &c, DOMAIN).await,
        Err(sources::SourceError::Changed)
    ));
    tx.commit().await.unwrap();
    let (_, publication) = fixture::ops_seed(&db).await;
    let query = sources::SourceQuery {
        domain_ref: Some(DOMAIN),
        kind: Some(SourceKind::OpsPublication),
        ..Default::default()
    };
    let page = sources::list_sources(&db, &query).await.unwrap();
    assert_eq!(page["totalBasis"], "source_directory");
    assert!(page["returnedFragmentCount"].as_u64().unwrap() > 0);
    let mut tx = db.pool().begin().await.unwrap();
    assert!(
        sources::read_source_in(
            &mut tx,
            &SourceKind::OpsPublication,
            publication,
            None,
            &SourceField::Body
        )
        .await
        .is_ok()
    );
    tx.commit().await.unwrap();
    sqlx::query("UPDATE linggan_ops_publication SET missing_since_batch_ref=first_batch_ref WHERE publication_ref=$1").bind(publication).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(
        sources::read_source_in(
            &mut tx,
            &SourceKind::OpsPublication,
            publication,
            None,
            &SourceField::Body
        )
        .await
        .is_err()
    );
    tx.commit().await.unwrap();
}
