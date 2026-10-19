//! #425 isolated PostgreSQL + filesystem proof. Synthetic authorship, no external calls.
#![recursion_limit = "256"]
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
use fixture::*;
use linggan_intelligence::content_workbench::*;
use std::{
    fs,
    io::{Cursor, Read},
};
use uuid::Uuid;
#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn save_replay_stale_and_restore_keep_linear_history() {
    let db = db("creation_revision").await;
    let first = create(&db, doc()).await;
    let mut newer = first.revision.document.clone();
    newer.paragraphs[0].text = "第二版🙂\n保留换行".into();
    let r = SaveRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        document: newer.clone(),
    };
    let saved = save(&db, first.work_id, &r).await.unwrap();
    assert_eq!(saved.revision.number, 2);
    let receipt = read_request(&db, r.request_id).await.unwrap();
    assert_eq!(receipt["kind"], "work");
    assert_eq!(
        receipt["receipt"]["revision"]["id"],
        serde_json::json!(saved.revision.id)
    );
    assert!(matches!(
        read_request(&db, Uuid::new_v4()).await,
        Err(WorkbenchError::NotFound)
    ));
    let retry = save(&db, first.work_id, &r).await.unwrap();
    assert!(retry.replayed);
    assert_eq!(retry.revision.id, saved.revision.id);
    let mut changed = r.clone();
    changed.document.title = "异体命令".into();
    assert!(matches!(
        save(&db, first.work_id, &changed).await,
        Err(WorkbenchError::RequestConflict)
    ));
    let stale = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            document: doc(),
        },
    )
    .await
    .unwrap();
    assert_eq!(stale.outcome, "stale");
    assert_eq!(stale.revision.id, saved.revision.id);
    let restored = restore(
        &db,
        first.work_id,
        &RestoreRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            restore_revision: first.revision.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(restored.revision.number, 3);
    assert_eq!(restored.revision.document, first.revision.document);
    assert_eq!(
        read_work(&db, first.work_id).await.unwrap().revision.id,
        restored.revision.id
    );
    assert_eq!(revisions(&db, first.work_id).await.unwrap().len(), 3);
    assert!(
        sqlx::query("UPDATE linggan_creation_revision SET document='{}' WHERE revision_ref=$1")
            .bind(first.revision.id)
            .execute(db.pool())
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn same_revision_two_tabs_allow_exactly_one_writer() {
    let db = db("creation_tabs").await;
    let first = create(&db, doc()).await;
    let mut a = first.revision.document.clone();
    a.title = "标签页甲".into();
    let mut b = a.clone();
    b.title = "标签页乙".into();
    let ra = SaveRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        document: a,
    };
    let rb = SaveRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        document: b,
    };
    let (a, b) = tokio::join!(save(&db, first.work_id, &ra), save(&db, first.work_id, &rb));
    let results = [a.unwrap(), b.unwrap()];
    assert_eq!(
        results.iter().filter(|r| r.outcome == "accepted").count(),
        1
    );
    assert_eq!(results.iter().filter(|r| r.outcome == "stale").count(), 1);
    assert_eq!(revisions(&db, first.work_id).await.unwrap().len(), 2);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn exact_excerpt_atomic_multitarget_place_and_deleted_target_preserve_identity() {
    let db = db("creation_place").await;
    let first = create(&db, cited_doc(&db).await).await;
    let d = &first.revision.document;
    assert!(d.excerpts[0].citation.start > 4000);
    let request = PlaceRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        canvas_id: d.canvas[0].id,
        targets: vec![
            UseTarget::Paragraph {
                paragraph_id: d.paragraphs[0].id,
            },
            UseTarget::PageBlock {
                page_id: d.pages[1].id,
                block_id: d.pages[1].blocks[0].id,
            },
        ],
    };
    let placed = place(&db, first.work_id, &request).await.unwrap();
    assert_eq!(placed.revision.document.uses.len(), 2);
    assert!(
        placed.revision.document.paragraphs[0]
            .text
            .contains("结尾不能丢失🙂")
    );
    assert_eq!(
        place(&db, first.work_id, &request)
            .await
            .unwrap()
            .revision
            .id,
        placed.revision.id
    );
    let invalid = PlaceRequest {
        request_id: Uuid::new_v4(),
        expected_revision: placed.revision.id,
        canvas_id: request.canvas_id,
        targets: vec![
            request.targets[0].clone(),
            UseTarget::Paragraph {
                paragraph_id: Uuid::new_v4(),
            },
        ],
    };
    assert!(place(&db, first.work_id, &invalid).await.is_err());
    assert_eq!(
        read_work(&db, first.work_id).await.unwrap().revision.id,
        placed.revision.id
    );
    let mut doc = placed.revision.document.clone();
    doc.pages.reverse();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: placed.revision.id,
            document: doc.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.revision.document.uses, placed.revision.document.uses);
    doc.pages.retain(|p| p.kind != PageKind::Voice);
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: doc,
        },
    )
    .await
    .unwrap();
    assert!(
        saved
            .revision
            .document
            .uses
            .iter()
            .any(|u| u.state == UseState::NeedsReview)
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn withdrawn_reference_redacts_history_replay_and_allows_free_edit() {
    let db = db("creation_restriction").await;
    let create_doc = comment_doc(&db).await;
    let request = CreateRequest {
        request_id: Uuid::new_v4(),
        domain_ref: fixture::dossier::DOMAIN,
        document: create_doc,
    };
    let first = create_work(&db, &request).await.unwrap();
    let citation = &first.revision.document.excerpts[0].citation;
    restrict_comment(&db, citation.work_ref.unwrap()).await;
    let read = read_work(&db, first.work_id).await.unwrap();
    assert!(read.revision.document.excerpts[0].selected_text.is_none());
    assert!(
        create_work(&db, &request)
            .await
            .unwrap()
            .revision
            .document
            .excerpts[0]
            .selected_text
            .is_none()
    );
    assert!(
        revisions(&db, first.work_id).await.unwrap()[0]
            .document
            .excerpts[0]
            .selected_text
            .is_none()
    );
    let mut doc = read.revision.document;
    doc.paragraphs[0].text = "作者仍可继续自由创作".into();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            document: doc,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.outcome, "accepted");
    assert_eq!(
        saved.revision.document.paragraphs[0].text,
        "作者仍可继续自由创作"
    );
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM linggan_creation_revision WHERE revision_ref=$1")
            .bind(saved.revision.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(stored["excerpts"][0]["selectedText"].is_string());
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn domain_and_asset_owner_integrity_are_not_client_claims() {
    let db = db("creation_assets").await;
    assert!(
        create_work(
            &db,
            &CreateRequest {
                request_id: Uuid::new_v4(),
                domain_ref: Uuid::new_v4(),
                document: doc()
            }
        )
        .await
        .is_err()
    );
    let a = create(&db, doc()).await;
    let b = create(&db, doc()).await;
    let bytes = png(40, 30, 80);
    let request = Uuid::new_v4();
    let asset = upload_asset(&db, a.work_id, request, &bytes).await.unwrap();
    assert_eq!((asset.width, asset.height), (40, 30));
    assert_eq!(
        upload_asset(&db, a.work_id, request, &bytes)
            .await
            .unwrap()
            .id,
        asset.id
    );
    assert!(
        upload_asset(&db, a.work_id, request, &png(40, 30, 81))
            .await
            .is_err()
    );
    assert!(
        upload_asset(&db, a.work_id, Uuid::new_v4(), b"<svg/>")
            .await
            .is_err()
    );
    assert!(asset_file(&db, b.work_id, asset.id).await.is_err());
    let mut document = b.revision.document;
    document.pages[0].asset_id = Some(asset.id);
    assert!(
        save(
            &db,
            b.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: b.revision.id,
                document
            }
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn frozen_export_png_zip_replay_failure_and_immutable_package() {
    let db = db("creation_export").await;
    let mut document = doc();
    document.tags = vec!["作者确认".into(), "#合成标签".into()];
    document.account_ref = Some(account(&db).await);
    document.pages.extend([
        page(PageKind::Opinion),
        page(PageKind::Steps),
        page(PageKind::Opinion),
    ]);
    let serialized = serde_json::to_value(&document).unwrap();
    assert!(serialized.get("materials").is_none());
    assert!(serialized.get("materialPreparation").is_none());
    assert_eq!(
        serde_json::from_value::<WorkDocument>(serialized).unwrap(),
        document
    );
    let first = create(&db, document).await;
    let r = FreezeRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        renderer_version: "creation-renderer.v1".into(),
        font_version: "synthetic-system-cjk-fixture".into(),
    };
    let (export, revision) = freeze_export(&db, first.work_id, &r).await.unwrap();
    assert_eq!(export.pages.len(), 5);
    assert!(
        export
            .created_at
            .as_ref()
            .is_some_and(|time| time.ends_with('Z'))
    );
    let mut historical = serde_json::to_value(&export).unwrap();
    historical.as_object_mut().unwrap().remove("createdAt");
    let old_receipt: ExportRecord = serde_json::from_value(historical.clone()).unwrap();
    assert!(old_receipt.created_at.is_none());
    assert_eq!(serde_json::to_value(old_receipt).unwrap(), historical);
    assert_eq!(
        freeze_export(&db, first.work_id, &r).await.unwrap().0.id,
        export.id
    );
    assert!(matches!(
        finalize_export(
            &db,
            export.id,
            &FinalizeRequest {
                request_id: Uuid::new_v4()
            }
        )
        .await,
        Err(WorkbenchError::Incomplete)
    ));
    mark_export_failed(
        &db,
        export.id,
        &ExportFailureRequest {
            request_id: Uuid::new_v4(),
            code: "render_failure".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(read_export(&db, export.id).await.unwrap().state, "failed");
    retry_export(
        &db,
        export.id,
        &FinalizeRequest {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert!(
        upload_export_page(&db, export.id, export.pages[0].page_id, &png(1, 1, 0))
            .await
            .is_err()
    );
    let good = png(1080, 1440, 120);
    let mut bad = good.clone();
    let len = bad.len();
    bad[len - 6] ^= 1;
    assert!(
        upload_export_page(&db, export.id, export.pages[0].page_id, &bad)
            .await
            .is_err()
    );
    for p in &export.pages {
        upload_export_page(&db, export.id, p.page_id, &good)
            .await
            .unwrap();
    }
    assert!(
        upload_export_page(&db, export.id, Uuid::new_v4(), &good)
            .await
            .is_err()
    );
    let complete = finalize_export(
        &db,
        export.id,
        &FinalizeRequest {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(complete.state, "complete");
    let file = export_file(&db, export.id).await.unwrap();
    let bytes = fs::read(&file).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    assert_eq!(archive.len(), 6);
    let mut body = String::new();
    archive
        .by_name("正文.txt")
        .unwrap()
        .read_to_string(&mut body)
        .unwrap();
    assert!(body.contains("手写正文，标点。"));
    assert!(body.ends_with("#作者确认 #合成标签"));
    assert!(archive.by_name("manifest.json").is_err());
    for i in 1..=5 {
        let mut image = Vec::new();
        archive
            .by_name(&format!(
                "{i:02}-{}.png",
                if i == 1 { "封面" } else { "内页" }
            ))
            .unwrap()
            .read_to_end(&mut image)
            .unwrap();
        assert_eq!(image, good);
    }
    let again = finalize_export(
        &db,
        export.id,
        &FinalizeRequest {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(again.package_sha256, complete.package_sha256);
    let mut next = revision.document;
    next.title = "导出后新编辑".into();
    save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            document: next,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        fs::read(export_file(&db, export.id).await.unwrap()).unwrap(),
        bytes
    );
    assert!(
        sqlx::query(
            "UPDATE linggan_creation_export SET renderer_version='forged' WHERE export_ref=$1"
        )
        .bind(export.id)
        .execute(db.pool())
        .await
        .is_err()
    );
    assert!(
        sqlx::query("UPDATE linggan_creation_export_page SET sha256=$2 WHERE export_ref=$1")
            .bind(export.id)
            .bind("a".repeat(64))
            .execute(db.pool())
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn unused_withdrawn_excerpt_does_not_block_independent_export() {
    let db = db("creation_unused_quote").await;
    let mut document = comment_doc(&db).await;
    document.account_ref = Some(account(&db).await);
    let source_work = document.excerpts[0].citation.work_ref.unwrap();
    let first = create(&db, document).await;
    restrict_comment(&db, source_work).await;
    let (export, snapshot) = freeze_export(
        &db,
        first.work_id,
        &FreezeRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            renderer_version: "creation-renderer.v1".into(),
            font_version: "synthetic-fixture".into(),
        },
    )
    .await
    .unwrap();
    assert!(snapshot.document.excerpts[0].selected_text.is_none());
    assert_eq!(snapshot.document.paragraphs[0].text, "手写正文，标点。");
    assert_eq!(export.state, "rendering");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn restricted_export_download_blocked_but_independent_rewrite_is_preserved() {
    let db = db("creation_export_revocation").await;
    let mut document = comment_doc(&db).await;
    document.account_ref = Some(account(&db).await);
    document.pages[0].subtitle = "作者图页副题".into();
    let source_work = document.excerpts[0].citation.work_ref.unwrap();
    let first = create(&db, document).await;
    let placed = place(
        &db,
        first.work_id,
        &PlaceRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            canvas_id: first.revision.document.canvas[0].id,
            targets: vec![
                UseTarget::Paragraph {
                    paragraph_id: first.revision.document.paragraphs[0].id,
                },
                UseTarget::PageSubtitle {
                    page_id: first.revision.document.pages[0].id,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        placed.revision.document.uses[0].content_origin,
        ContentOrigin::Excerpt
    );
    let (export, _) = freeze_export(
        &db,
        first.work_id,
        &FreezeRequest {
            request_id: Uuid::new_v4(),
            expected_revision: placed.revision.id,
            renderer_version: "creation-renderer.v1".into(),
            font_version: "synthetic-fixture".into(),
        },
    )
    .await
    .unwrap();
    let png = png(1080, 1440, 70);
    for p in &export.pages {
        upload_export_page(&db, export.id, p.page_id, &png)
            .await
            .unwrap();
    }
    let finalize = FinalizeRequest {
        request_id: Uuid::new_v4(),
    };
    finalize_export(&db, export.id, &finalize).await.unwrap();
    assert!(export_file(&db, export.id).await.is_ok());
    restrict_comment(&db, source_work).await;
    assert!(matches!(
        export_file(&db, export.id).await,
        Err(WorkbenchError::Source)
    ));
    assert!(matches!(
        export_page_file(&db, export.id, export.pages[0].page_id).await,
        Err(WorkbenchError::Source)
    ));
    assert!(matches!(
        finalize_export(&db, export.id, &finalize).await,
        Err(WorkbenchError::Source)
    ));
    let read = read_work(&db, first.work_id).await.unwrap();
    assert_eq!(
        read.revision.document.paragraphs[0].text,
        "手写正文，标点。\n\n[来源当前不可使用]"
    );
    assert_eq!(
        read.revision.document.pages[0].subtitle,
        "作者图页副题\n\n[来源当前不可使用]"
    );
    let mut own = read.revision.document;
    own.pages[0].subtitle = "作者独立副题".into();
    own.paragraphs[0].text = "作者独立改写：把复杂目标拆成一次能完成的小动作。".into();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: placed.revision.id,
            document: own,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        saved.revision.document.uses[0].content_origin,
        ContentOrigin::Expression
    );
    assert!(
        saved.revision.document.paragraphs[0]
            .text
            .starts_with("作者独立改写")
    );
    assert!(
        freeze_export(
            &db,
            first.work_id,
            &FreezeRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                renderer_version: "creation-renderer.v1".into(),
                font_version: "synthetic-fixture".into()
            }
        )
        .await
        .is_ok()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn confirmed_tags_and_page_style_are_validated_and_revisioned() {
    let db = db("creation_tags").await;
    let mut document = doc();
    document.tags = vec!["作者确认".into(), "保留😀".into()];
    document.pages[0].template_version = "creation-v1-editorial".into();
    document.pages[0].style.alignment = Alignment::Center;
    let first = create(&db, document).await;
    assert_eq!(
        read_work(&db, first.work_id)
            .await
            .unwrap()
            .revision
            .document
            .tags,
        vec!["作者确认", "保留😀"]
    );
    for tags in [
        vec!["".into()],
        vec![" 含首空格".into()],
        vec!["换行\n标签".into()],
        vec!["重复".into(), "重复".into()],
        vec!["长".repeat(51)],
        vec!["标签".into(); 31],
    ] {
        let mut document = first.revision.document.clone();
        document.tags = tags;
        assert!(matches!(
            save(
                &db,
                first.work_id,
                &SaveRequest {
                    request_id: Uuid::new_v4(),
                    expected_revision: first.revision.id,
                    document
                }
            )
            .await,
            Err(WorkbenchError::Invalid(_))
        ));
    }
    let mut historical = serde_json::to_value(&first.revision.document).unwrap();
    historical.as_object_mut().unwrap().remove("tags");
    historical["pages"][0]["style"]
        .as_object_mut()
        .unwrap()
        .remove("alignment");
    let historical: WorkDocument = serde_json::from_value(historical).unwrap();
    assert!(historical.tags.is_empty());
    assert_eq!(historical.pages[0].style.alignment, Alignment::Start);
    assert_eq!(revisions(&db, first.work_id).await.unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn request_uuid_is_exclusive_across_work_asset_and_export_writers() {
    let db = db("creation_request_owner").await;
    let mut document = doc();
    document.account_ref = Some(account(&db).await);
    let first = create(&db, document).await;
    let bytes = png(1080, 1440, 70);
    assert!(matches!(
        upload_asset(&db, first.work_id, first.request_id, &bytes).await,
        Err(WorkbenchError::RequestConflict)
    ));
    let asset_request = Uuid::new_v4();
    let asset = upload_asset(&db, first.work_id, asset_request, &bytes)
        .await
        .unwrap();
    assert!(matches!(
        save(
            &db,
            first.work_id,
            &SaveRequest {
                request_id: asset_request,
                expected_revision: first.revision.id,
                document: first.revision.document.clone()
            }
        )
        .await,
        Err(WorkbenchError::RequestConflict)
    ));
    let freeze = FreezeRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        renderer_version: "creation-renderer.v1".into(),
        font_version: "synthetic-fixture".into(),
    };
    let mut collision = freeze.clone();
    collision.request_id = asset_request;
    assert!(matches!(
        freeze_export(&db, first.work_id, &collision).await,
        Err(WorkbenchError::RequestConflict)
    ));
    let (export, _) = freeze_export(&db, first.work_id, &freeze).await.unwrap();
    assert!(matches!(
        upload_asset(&db, first.work_id, freeze.request_id, &bytes).await,
        Err(WorkbenchError::RequestConflict)
    ));
    assert!(matches!(
        mark_export_failed(
            &db,
            export.id,
            &ExportFailureRequest {
                request_id: freeze.request_id,
                code: "overflow".into()
            }
        )
        .await,
        Err(WorkbenchError::RequestConflict)
    ));
    let asset_receipt = read_request(&db, asset_request).await.unwrap();
    assert_eq!(asset_receipt["kind"], "asset");
    assert_eq!(asset_receipt["receipt"]["id"], serde_json::json!(asset.id));
    let freeze_receipt = read_request(&db, freeze.request_id).await.unwrap();
    assert_eq!(freeze_receipt["kind"], "export");
    assert_eq!(
        freeze_receipt["receipt"]["revision"]["id"],
        serde_json::json!(first.revision.id)
    );
    let failed = ExportFailureRequest {
        request_id: Uuid::new_v4(),
        code: "overflow".into(),
    };
    mark_export_failed(&db, export.id, &failed).await.unwrap();
    let retried = FinalizeRequest {
        request_id: Uuid::new_v4(),
    };
    retry_export(&db, export.id, &retried).await.unwrap();
    let retry_receipt = read_request(&db, retried.request_id).await.unwrap();
    assert_eq!(
        retry_receipt["receipt"]["export"]["id"],
        serde_json::json!(export.id)
    );
    assert_eq!(
        retry_receipt["receipt"]["revision"]["id"],
        serde_json::json!(first.revision.id)
    );
    // Different writer owners racing with a fresh UUID produce exactly one receipt.
    let concurrent = Uuid::new_v4();
    let save_request = SaveRequest {
        request_id: concurrent,
        expected_revision: first.revision.id,
        document: first.revision.document.clone(),
    };
    let (save_result, asset_result) = tokio::join!(
        save(&db, first.work_id, &save_request),
        upload_asset(&db, first.work_id, concurrent, &bytes)
    );
    assert_eq!(
        usize::from(save_result.is_ok()) + usize::from(asset_result.is_ok()),
        1
    );
    assert!(
        matches!(save_result, Err(WorkbenchError::RequestConflict))
            || matches!(asset_result, Err(WorkbenchError::RequestConflict))
    );
    assert!(read_request(&db, concurrent).await.is_ok());
}
