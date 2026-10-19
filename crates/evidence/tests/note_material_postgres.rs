use linggan_evidence::{note_material_input as input, research_dossier as dossier, *};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/research_dossier_fixture.rs"]
mod source;
use source::{DOMAIN, fixture};

async fn seed(db: &Database, id: &str, payload: Value, observed: &str) -> Uuid {
    let package=fixture::submit_package_at(db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":payload}),observed).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4) ON CONFLICT DO NOTHING").bind(Uuid::new_v4()).bind(work).bind(DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    work
}
fn column<'a>(m: &'a input::Material, field: &str) -> &'a input::Column {
    m.columns.iter().find(|c| c.field == field).unwrap()
}
async fn counts(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_runtime_capture_package),(SELECT count(*) FROM linggan_material_content_detail),(SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_media_ocr_line),(SELECT count(*) FROM linggan_model_invocation))").fetch_one(db.pool()).await.unwrap()
}
async fn clock(db: &Database, time: &str) {
    // Only this synthetic schema's clock changes; SQL is selected from fixed literals.
    let sql = match time {
        "start" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT timestamptz '2026-10-17T00:00:00Z' $$"
        }
        "before" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT timestamptz '2026-10-17T23:59:59Z' $$"
        }
        "boundary" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT timestamptz '2026-10-18T00:00:00Z' $$"
        }
        "later" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT timestamptz '2026-10-18T01:00:00Z' $$"
        }
        _ => panic!("unknown proof clock"),
    };
    sqlx::query(sql).execute(db.pool()).await.unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn tags_upgrade_existing_detail_is_narrow_idempotent_and_atomic() {
    let db = fixture::proof_database("note_material_tags").await;
    let work=seed(&db,"tags",json!({"title":"合成标题","bodyText":"合成正文🙂","hashtags":["#A","#B","#A"],"keywords":["不是标签"]}),"2026-10-17T00:00:00Z").await;
    let old = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(
        input::read(&db, DOMAIN, work)
            .await
            .unwrap()
            .fragments
            .iter()
            .filter(|f| f.field == "tags")
            .map(|f| f.text.as_str())
            .collect::<Vec<_>>(),
        vec!["#A", "#B", "#A"]
    );
    // Reconstruct the real upgrade condition: accepted raw/detail facts exist but the
    // newly introduced derived tables are empty. Only this disposable fixture is altered.
    sqlx::query("TRUNCATE linggan_material_content_hashtag,linggan_material_hashtag_projection")
        .execute(db.pool())
        .await
        .unwrap();
    let material: Uuid =
        sqlx::query_scalar("SELECT material_ref FROM linggan_material_content_detail LIMIT 1")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_hashtag_projection(material_ref,projection_contract,state,tag_count,source_hash) VALUES($1,'content-hashtags.v1','provided',1,$2)").bind(material).bind("a".repeat(64)).execute(&mut *tx).await.unwrap();
    assert!(
        tx.commit().await.is_err(),
        "deferred count check rolls back incomplete receipts"
    );
    let baseline = counts(&db).await;
    let waiting = input::read(&db, DOMAIN, work).await.unwrap();
    assert!(
        waiting
            .not_ready_reasons
            .contains(&"tags_projection_pending".into())
    );
    let plan = input::plan(&db, DOMAIN, &[work], None, input::BatchOperation::Hashtags)
        .await
        .unwrap();
    assert_eq!(plan.entries[0].pending_tag_details, 1);
    assert_eq!(counts(&db).await, baseline);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_material_hashtag_projection")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "plan is read only"
    );
    let receipt = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(receipt.updated, 1);
    assert!(receipt.complete());
    let second = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(second.already_current, 1);
    assert_eq!(
        counts(&db).await,
        baseline,
        "no detail, package, media job, OCR line or model added"
    );
    assert_eq!(
        dossier::read(&db, work).await.unwrap().unwrap().revision,
        old.revision,
        "old dossier identity unchanged by tags"
    );
    assert!(
        input::read_at(&db, DOMAIN, &[work], Some(&plan.as_of))
            .await
            .unwrap()[0]
            .not_ready_reasons
            .contains(&"tags_projection_pending".into()),
        "future projection does not leak into frozen reads"
    );
    assert_eq!(
        column(&input::read(&db, DOMAIN, work).await.unwrap(), "tags").state,
        input::ColumnState::Available
    );
    let mut tampered = plan.clone();
    tampered.entries[0].work_ref = Uuid::new_v4();
    assert!(input::apply_plan(&db, &tampered).await.is_err());
    assert!(
        input::plan(
            &db,
            Uuid::new_v4(),
            &[work],
            None,
            input::BatchOperation::Hashtags
        )
        .await
        .is_err()
    );
    assert!(
        input::plan(
            &db,
            DOMAIN,
            &vec![work; 201],
            None,
            input::BatchOperation::Hashtags
        )
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE linggan_material_content_hashtag SET tag_text='changed' WHERE material_ref=$1"
        )
        .bind(material)
        .execute(db.pool())
        .await
        .is_err()
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn fingerprints_and_snapshots_separate_content_sources_time_and_missing_states() {
    let db = fixture::proof_database("note_material_identity").await;
    let work = seed(
        &db,
        "identity",
        json!({"title":"标题","bodyText":"原文🙂","likedCount":1,"hashtags":["#一","#二"]}),
        "2026-10-17T00:00:00Z",
    )
    .await;
    let first = input::read(&db, DOMAIN, work).await.unwrap();
    assert!(first.ready);
    let a = input::save_snapshot(&db, DOMAIN, work, &first.as_of, &first.dossier_revision)
        .await
        .unwrap();
    let again = input::save_snapshot(&db, DOMAIN, work, &first.as_of, &first.dossier_revision)
        .await
        .unwrap();
    assert!(a.inserted);
    assert!(!again.inserted);
    assert_eq!(a.snapshot_ref, again.snapshot_ref);
    seed(
        &db,
        "identity",
        json!({"title":"标题","bodyText":"原文🙂","likedCount":999,"hashtags":["#一","#二"]}),
        "2026-10-18T00:00:00Z",
    )
    .await;
    let second = input::read(&db, DOMAIN, work).await.unwrap();
    assert_eq!(
        first.material_fingerprint, second.material_fingerprint,
        "likes, observation and new UUIDs excluded"
    );
    assert_ne!(first.provenance_hash, second.provenance_hash);
    assert_ne!(first.snapshot_identity, second.snapshot_identity);
    assert!(
        input::save_snapshot(&db, DOMAIN, work, &second.as_of, &first.dossier_revision)
            .await
            .is_err(),
        "stale revision"
    );
    let b = input::save_snapshot(&db, DOMAIN, work, &second.as_of, &second.dossier_revision)
        .await
        .unwrap();
    assert_ne!(a.snapshot_ref, b.snapshot_ref);
    let frozen = input::read_at(&db, DOMAIN, &[work], Some(&first.as_of))
        .await
        .unwrap()
        .remove(0);
    assert_eq!(frozen.provenance_hash, first.provenance_hash);
    seed(
        &db,
        "identity",
        json!({"title":"标题","bodyText":"修改正文🙂","hashtags":["#一","#二"]}),
        "2026-10-19T00:00:00Z",
    )
    .await;
    assert_ne!(
        first.material_fingerprint,
        input::read(&db, DOMAIN, work)
            .await
            .unwrap()
            .material_fingerprint
    );
    for (id, payload, expected) in [
        (
            "empty",
            json!({"hashtags":[]}),
            input::ColumnState::KnownEmpty,
        ),
        ("absent", json!({}), input::ColumnState::NotProvided),
        (
            "invalid",
            json!({"hashtags":["x",12]}),
            input::ColumnState::InvalidSource,
        ),
    ] {
        let work = seed(&db, id, payload, "2026-10-17T00:00:00Z").await;
        assert_eq!(
            column(&input::read(&db, DOMAIN, work).await.unwrap(), "tags").state,
            expected
        );
    }
    assert!(
        sqlx::query("DELETE FROM linggan_note_material_snapshot WHERE snapshot_ref=$1")
            .bind(a.snapshot_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_note_material_snapshot SELECT $1,domain_ref,content_public_ref,$2,material_fingerprint,provenance_hash,input_contract,fingerprint_contract,dossier_contract,layer_version,as_of,ready,not_ready_reasons,columns,1,created_at FROM linggan_note_material_snapshot WHERE snapshot_ref=$3").bind(Uuid::new_v4()).bind("f".repeat(64)).bind(a.snapshot_ref).execute(&mut *tx).await.unwrap();
    assert!(
        tx.commit().await.is_err(),
        "deferred fragment-count constraint rejects incomplete snapshots"
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn grace_uses_first_domain_membership_and_media_wait_is_not_waived() {
    let db = fixture::proof_database("note_material_readiness").await;
    clock(&db, "start").await;
    let package=fixture::submit_package(&db,"profile_discovery",json!({"authorExternalId":"synthetic-author"}),json!({"kind":"profile_discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":"discovery-only"},"payload":{"title":"仅发现标题"}})).await;
    let work: Uuid = sqlx::query_scalar("SELECT public_ref FROM linggan_material_content LIMIT 1")
        .fetch_one(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    clock(&db, "before").await;
    assert!(!input::read(&db, DOMAIN, work).await.unwrap().ready);
    clock(&db, "boundary").await;
    assert!(
        input::read(&db, DOMAIN, work).await.unwrap().ready,
        "24h inclusive, missing detail still visible"
    );
    let observation = Uuid::new_v4();
    fixture::submit_package(&db,"media_slots",json!({"contentExternalId":"discovery-only"}),json!({"kind":"media_slot","slotKey":"xhs:discovery-only:image:1","observationRef":observation,"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/synthetic.jpg","candidateUris":["https://media.example/synthetic.jpg"],"observedAt":"2026-10-17T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"discovery-only"}})).await;
    let waiting = input::read(&db, DOMAIN, work).await.unwrap();
    assert!(!waiting.ready);
    assert_eq!(
        column(&waiting, "ocr").state,
        input::ColumnState::Waiting,
        "missing job/work is unknown, never terminal"
    );
    admit_media_blob(
        &db,
        observation,
        &"d".repeat(64),
        "image/jpeg",
        12,
        "blobs/synthetic.jpg",
    )
    .await
    .unwrap();
    ensure_media_processing_work(&db).await.unwrap();
    let job: Uuid = sqlx::query_scalar(
        "SELECT job_ref FROM linggan_media_processing_job WHERE processor_kind='image_ocr' LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) VALUES($1,$2,'failed','synthetic failure')").bind(Uuid::new_v4()).bind(job).execute(db.pool()).await.unwrap();
    let frozen_plan = input::plan(&db, DOMAIN, &[work], None, input::BatchOperation::Snapshot)
        .await
        .unwrap();
    clock(&db, "later").await;
    sqlx::query("UPDATE linggan_media_processing_work SET state='retry_wait',attempt_count=1,updated_at=scope_001_now() WHERE job_ref=$1").bind(job).execute(db.pool()).await.unwrap();
    assert_eq!(
        dossier::read(&db, work).await.unwrap().unwrap().revision,
        frozen_plan.entries[0].expected_revision,
        "work-only drift does not change the old dossier revision"
    );
    let drift = input::apply_plan(&db, &frozen_plan).await.unwrap();
    assert_eq!(drift.failed, 1);
    assert_eq!(drift.results[0].result["reason"], "material_changed");
    for state in ["retry_wait", "leased", "terminal"] {
        let sql = match state {
            "retry_wait" => {
                "UPDATE linggan_media_processing_work SET state='retry_wait',attempt_count=1,next_attempt_at=scope_001_now()+interval '1 hour',worker_instance_ref=NULL,lease_expires_at=NULL,updated_at=scope_001_now() WHERE job_ref=$1"
            }
            "leased" => {
                "UPDATE linggan_media_processing_work SET state='leased',worker_instance_ref=gen_random_uuid(),lease_expires_at=scope_001_now()-interval '1 minute',next_attempt_at=scope_001_now(),updated_at=scope_001_now() WHERE job_ref=$1"
            }
            _ => {
                "UPDATE linggan_media_processing_work SET state='terminal',attempt_count=3,worker_instance_ref=NULL,lease_expires_at=NULL,next_attempt_at=scope_001_now(),updated_at=scope_001_now() WHERE job_ref=$1"
            }
        };
        sqlx::query(sql).bind(job).execute(db.pool()).await.unwrap();
        let m = input::read(&db, DOMAIN, work).await.unwrap();
        assert_eq!(m.ready, state == "terminal", "{state}");
    }
    clock(&db, "later").await;
    sqlx::query("UPDATE linggan_media_processing_work SET state='completed',completed_at=scope_001_now(),updated_at=scope_001_now() WHERE job_ref=$1").bind(job).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) VALUES($1,$2,'succeeded','no_text_observed')").bind(Uuid::new_v4()).bind(job).execute(db.pool()).await.unwrap();
    let empty = input::read(&db, DOMAIN, work).await.unwrap();
    assert!(empty.ready);
    assert_eq!(column(&empty, "ocr").state, input::ColumnState::KnownEmpty);

    assert!(
        input::read_at(&db, DOMAIN, &[work], Some("2099-01-01T00:00:00Z"))
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn bounded_relayer_replay_and_restriction_preserve_raw_facts() {
    let db = fixture::proof_database("note_material_relayer").await;
    let work = source::work(&db).await;
    let derivative = source::legacy_ocr(&db).await;
    let initial = counts(&db).await;
    let before = input::read(&db, DOMAIN, work).await.unwrap();
    assert!(!before.ready);
    assert_eq!(
        column(&before, "ocr").state,
        input::ColumnState::NeedsRelayer
    );
    let plan = input::plan(&db, DOMAIN, &[work], None, input::BatchOperation::Relayer)
        .await
        .unwrap();
    let result = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(result.layout_updated, 1);
    assert_eq!(result.updated, 1);
    let second = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(second.already_current, 1);
    assert_eq!(second.layout_updated, 0);
    assert_eq!(
        counts(&db).await,
        initial,
        "first relayer and replay create no source facts, media jobs, OCR lines or model calls"
    );
    // A completed replay is accepted only while the underlying source is unchanged.
    seed(
        &db,
        "dossier-note",
        json!({"title":"来源已变化","bodyText":"新的合成正文"}),
        "2026-10-19T00:00:00Z",
    )
    .await;
    let drift = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(drift.failed, 1);
    assert!(!drift.complete());
    let initial = counts(&db).await;
    let material = input::read(&db, DOMAIN, work).await.unwrap();
    assert!(
        material
            .fragments
            .iter()
            .any(|f| f.field == "ocr" && f.text.contains("第3行"))
    );
    assert!(material.ready);
    assert_eq!(counts(&db).await, initial);
    input::save_snapshot(
        &db,
        DOMAIN,
        work,
        &material.as_of,
        &material.dossier_revision,
    )
    .await
    .unwrap();
    record_derivative_disposition(
        &db,
        derivative,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "withdraw synthetic",
    )
    .await
    .unwrap();
    let frozen = input::read_at(&db, DOMAIN, &[work], Some(&material.as_of))
        .await
        .unwrap()
        .remove(0);
    assert!(
        !frozen
            .fragments
            .iter()
            .any(|f| f.field == "ocr" || f.field == "cover_headline")
    );
    assert_eq!(column(&frozen, "ocr").state, input::ColumnState::Restricted);
    assert_eq!(counts(&db).await, initial);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn batch_partial_receipts_and_reads_scale_by_batch_not_work() {
    let db = fixture::proof_database("note_material_batch").await;
    let mut works = vec![];
    for n in 0..12 {
        works.push(
            seed(
                &db,
                &format!("batch-{n}"),
                json!({"title":"合成标题","bodyText":"正文"}),
                "2026-10-17T00:00:00Z",
            )
            .await,
        );
    }
    let measure = |works: Vec<Uuid>, db: Database| async move {
        sqlx::query("SELECT public.pg_stat_statements_reset()")
            .execute(db.pool())
            .await
            .unwrap();
        let values = input::read_at(&db, DOMAIN, &works, None).await.unwrap();
        assert_eq!(values.len(), works.len());
        sqlx::query_scalar::<_,f64>("SELECT COALESCE(sum(calls),0)::float8 FROM public.pg_stat_statements WHERE query NOT LIKE '%pg_stat_statements%'").fetch_one(db.pool()).await.unwrap()
    };
    let one = measure(vec![works[0]], db.clone()).await;
    let twelve = measure(works.clone(), db.clone()).await;
    assert!(
        twelve <= one + 2.0,
        "batched read SQL: one={one}, twelve={twelve}"
    );
    let plan = input::plan(&db, DOMAIN, &works, None, input::BatchOperation::Snapshot)
        .await
        .unwrap();
    seed(
        &db,
        "batch-0",
        json!({"title":"新标题"}),
        "2026-10-18T00:00:00Z",
    )
    .await;
    let receipt = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(receipt.failed, 1);
    assert_eq!(receipt.updated, 11);
    assert_eq!(
        receipt.updated + receipt.already_current + receipt.partial + receipt.failed,
        12
    );
    let replay = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(replay.failed, 1);
    assert_eq!(replay.already_current, 11);
    let observation = Uuid::new_v4();
    fixture::submit_package(&db,"media_slots",json!({"contentExternalId":"batch-1"}),json!({"kind":"media_slot","slotKey":"xhs:batch-1:image:1","observationRef":observation,"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/missing.jpg","candidateUris":["https://media.example/missing.jpg"],"observedAt":"2026-10-17T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"batch-1"}})).await;
    let plan = input::plan(
        &db,
        DOMAIN,
        &[works[1]],
        None,
        input::BatchOperation::Relayer,
    )
    .await
    .unwrap();
    let partial = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(partial.partial, 1);
    assert_eq!(partial.needs_ocr, 1);
    assert!(!partial.complete());
    let raw = counts(&db).await;
    input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(counts(&db).await, raw);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn a_restricted_slot_without_a_job_is_not_waiting_and_cannot_hide_other_pending_slots() {
    let db = fixture::proof_database("note_material_restricted_slots").await;
    let work = seed(
        &db,
        "restricted-slots",
        json!({"title":"合成","bodyText":"合成正文"}),
        "2026-10-17T00:00:00Z",
    )
    .await;
    for n in 1..=2 {
        fixture::submit_package(&db,"media_slots",json!({"contentExternalId":"restricted-slots"}),json!({"kind":"media_slot","slotKey":format!("xhs:restricted-slots:image:{n}"),"observationRef":Uuid::new_v4(),"slot":{"role":"image","ordinal":n},"observation":{"externalUri":format!("https://media.example/{n}.jpg"),"candidateUris":[format!("https://media.example/{n}.jpg")],"observedAt":"2026-10-17T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"restricted-slots"}})).await;
        if n == 1 {
            let before = input::read(&db, DOMAIN, work).await.unwrap();
            assert!(!before.ready);
            record_slot_disposition(
                &db,
                "xhs:restricted-slots:image:1",
                MaterialMediaDisposition::WithdrawnOrRestricted,
                "synthetic-proof",
                "restricted before processing",
            )
            .await
            .unwrap();
            let restricted = input::read(&db, DOMAIN, work).await.unwrap();
            assert!(restricted.ready);
            assert_eq!(
                column(&restricted, "ocr").state,
                input::ColumnState::Restricted
            );
            let frozen = input::read_at(&db, DOMAIN, &[work], Some(&before.as_of))
                .await
                .unwrap()
                .remove(0);
            assert!(frozen.ready);
            assert_eq!(
                column(&frozen, "ocr").state,
                input::ColumnState::Restricted,
                "current restriction overrides historical cutoff without a job"
            );
        } else {
            let mixed = input::read(&db, DOMAIN, work).await.unwrap();
            assert!(!mixed.ready);
            assert_eq!(column(&mixed, "ocr").state, input::ColumnState::Waiting);
        }
    }
}
