//! Bounded legacy recovery; only random proof databases and synthetic text.
#![recursion_limit = "256"]
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
use fixture::*;
use linggan_intelligence::content_workbench::{self as cw, recovery, *};
use serde_json::json;

use uuid::Uuid;
#[path = "support/legacy_workbench_fixture.rs"]
mod old_fixture;
use old_fixture::{legacy, snapshot};
fn hash(bytes: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn recovery_appends_preserves_all_103_and_history_stays_readable() {
    let db = db("creation_legacy_recovery").await;
    let a = cited_doc(&db).await;
    let b = comment_doc_for_work(&db, a.excerpts[0].citation.work_ref.unwrap()).await;
    let (first, before_a) = legacy(&db, a.clone(), 91).await;
    let (second, before_b) = legacy(&db, b.clone(), 12).await;
    for (work, original, count) in [(first, a, 91), (second, b, 12)] {
        let before = snapshot(&db, work).await;
        assert!(matches!(
            read_work(&db, work).await,
            Err(WorkbenchError::LegacyReferenceUnavailable)
        ));
        let p = recovery::preview(&db, work).await.unwrap();
        assert_eq!(snapshot(&db, work).await, before, "preview must not write");
        assert_eq!(p.base_number, count);
        assert_eq!(p.bindings.len(), 1);
        let request = Uuid::new_v4();
        let digest = p.sha256().unwrap();
        assert!(recovery::apply(&db, &p, "wrong", request).await.is_err());
        let mut tampered = p.clone();
        tampered.title = "altered".into();
        assert!(
            recovery::apply(&db, &tampered, &tampered.sha256().unwrap(), request)
                .await
                .is_err()
        );
        let mut wrong_db = p.clone();
        wrong_db.database_identity["database"] = json!("other");
        assert!(
            recovery::apply(&db, &wrong_db, &wrong_db.sha256().unwrap(), request)
                .await
                .is_err()
        );
        let applied = recovery::apply(&db, &p, &digest, request).await.unwrap();
        assert_eq!(applied.revision.number, count + 1);
        assert_eq!(applied.revision.document.paragraphs, original.paragraphs);
        assert_eq!(applied.revision.document.pages, original.pages);
        assert_eq!(applied.revision.document.canvas, original.canvas);
        let again = recovery::apply(&db, &p, &digest, request).await.unwrap();
        assert!(again.replayed);
        assert_eq!(again.revision.id, applied.revision.id);
        assert!(
            recovery::apply(&db, &p, &digest, Uuid::new_v4())
                .await
                .is_err()
        );
        let all = snapshot(&db, work).await;
        assert_eq!(all.len(), count as usize + 1);
        assert_eq!(&all[..count as usize], before);
        let index = revision_index(&db, work).await.unwrap();
        assert_eq!(index.len(), count as usize + 1);
        assert_eq!(index[0].reference_format, "current");
        assert!(
            index[1..]
                .iter()
                .all(|r| r.reference_format == "legacy_reference")
        );
        let metadata = serde_json::to_value(&index).unwrap();
        assert!(
            metadata
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r.as_object().unwrap().len() == 4)
        );
        assert!(matches!(
            revision(&db, work, p.base_revision).await,
            Err(WorkbenchError::LegacyReferenceUnavailable)
        ));
        assert_eq!(
            read_work(&db, work).await.unwrap().revision.id,
            applied.revision.id
        );
        // Normal save remains usable after explicit recovery.
        let mut edited = applied.revision.document;
        edited.title.push_str("（恢复后编辑）");
        let save = cw::save(
            &db,
            work,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: applied.revision.id,
                document: edited,
            },
        )
        .await
        .unwrap();
        assert_eq!(save.revision.number, count + 2);
    }
    assert_eq!(&snapshot(&db, first).await[..91], before_a);
    assert_eq!(&snapshot(&db, second).await[..12], before_b);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn restriction_after_preview_blocks_apply_and_after_apply_redacts_detail_and_replay() {
    let db = db("creation_legacy_restriction").await;
    let document = comment_doc(&db).await;
    let source_work = document.excerpts[0].citation.work_ref.unwrap();
    let (work, before) = legacy(&db, document, 1).await;
    let p = recovery::preview(&db, work).await.unwrap();
    restrict_comment(&db, source_work).await;
    assert!(
        recovery::apply(&db, &p, &p.sha256().unwrap(), Uuid::new_v4())
            .await
            .is_err()
    );
    assert!(recovery::preview(&db, work).await.is_err());
    assert_eq!(snapshot(&db, work).await, before);
    assert_eq!(revision_index(&db, work).await.unwrap().len(), 1);

    let db = fixture::db("creation_legacy_withdrawal").await;
    let document = comment_doc(&db).await;
    let source_work = document.excerpts[0].citation.work_ref.unwrap();
    let (work, _) = legacy(&db, document, 1).await;
    let p = recovery::preview(&db, work).await.unwrap();
    let hash = p.sha256().unwrap();
    let request = Uuid::new_v4();
    let saved = recovery::apply(&db, &p, &hash, request).await.unwrap();
    restrict_comment(&db, source_work).await;
    let read = revision(&db, work, saved.revision.id).await.unwrap();
    assert!(read.document.excerpts[0].selected_text.is_none());
    let replay = recovery::apply(&db, &p, &hash, request).await.unwrap();
    assert!(replay.revision.document.excerpts[0].selected_text.is_none());
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn corrupt_or_ambiguous_legacy_quotes_fail_without_any_write() {
    let db = db("creation_legacy_invalid").await;
    let document = cited_doc(&db).await;
    let mut corrupt = document.clone();
    corrupt.excerpts[0].text_hash = "a".repeat(64);
    let mut oversized = document.clone();
    oversized.excerpts[0].citation.end += 1;
    let mut ambiguous = document;
    ambiguous.excerpts[0].citation.start = 0;
    ambiguous.excerpts[0].citation.end = 5;
    ambiguous.excerpts[0].selected_text = Some("完整正文。".into());
    ambiguous.excerpts[0].text_hash = hash("完整正文。");
    ambiguous.excerpts[0].citation.text_hash = hash("完整正文。");
    for invalid in [corrupt, oversized, ambiguous] {
        let (work, before) = legacy(&db, invalid, 1).await;
        assert!(recovery::preview(&db, work).await.is_err());
        assert_eq!(snapshot(&db, work).await, before);
    }
}
