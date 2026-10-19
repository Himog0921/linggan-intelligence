//! Opt-in private copies for a human review. Never used by CI or shared runtimes.
//! Source identities are mapped explicitly; importing these copies is not collection proof.
use super::*;
use std::collections::BTreeMap;

pub(super) async fn seed(db: &Database, path: &str) -> serde_json::Value {
    let sample: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(sample["domainRef"], fixture::dossier::DOMAIN.to_string());
    let (synthetic_account, publication) = fixture::ops_seed(db).await;
    let account = &sample["accounts"][0];
    assert_eq!(account["platform"], "xhs");
    let account_ref = Uuid::parse_str(account["ref"].as_str().unwrap()).unwrap();
    // The authentic account identity/name are copied only into this disposable schema.
    sqlx::query("INSERT INTO linggan_ops_account(account_ref,source_system,source_record_id,domain_ref,ledger_state,platform,account_name,status,is_retained,review_status,profile_link_state,first_batch_ref,last_changed_batch_ref,source_created_at,source_updated_at) SELECT $1,source_system,$2,domain_ref,ledger_state,'xhs',$3,'active',true,'confirmed','missing',first_batch_ref,last_changed_batch_ref,source_created_at,source_updated_at FROM linggan_ops_account WHERE account_ref=$4")
        .bind(account_ref).bind(format!("private-review-{}", account_ref)).bind(account["name"].as_str().unwrap())
        .bind(synthetic_account).execute(db.pool()).await.unwrap();
    sqlx::query(
        "UPDATE linggan_ops_account SET ledger_state='absent_from_export' WHERE account_ref<>$1",
    )
    .bind(account_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let mut notes = BTreeMap::<String, (String, String)>::new();
    for note in sample["notes"].as_array().unwrap() {
        notes.insert(
            note["workRef"].as_str().unwrap().to_owned(),
            (
                note["title"].as_str().unwrap().to_owned(),
                note["text"].as_str().unwrap().to_owned(),
            ),
        );
    }
    // Comments keep their own real parent note; never attach them to a convenient sample.
    for entry in sample["commentContexts"].as_array().unwrap() {
        let parent = &entry["context"]["parentWork"];
        let original = parent["workRef"].as_str().unwrap().to_owned();
        let fragments = parent["fragments"].as_array().unwrap();
        let text = |field: &str| {
            fragments
                .iter()
                .filter(|f| f["field"] == field)
                .filter_map(|f| f["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        notes
            .entry(original)
            .or_insert_with(|| (text("title"), text("body")));
    }
    let mut mapping = BTreeMap::new();
    let mut note_refs = Vec::new();
    for (original, (title, body)) in notes {
        let external = format!("private-review-{original}");
        let package = fixture::dossier::fixture::submit_package(db, "content_detail", json!({"contentExternalId":external}),
            json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":external},
                "payload":{"title":title,"bodyText":body,"authorId":format!("private-copy-{original}")}})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(&external)
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
            .bind(Uuid::new_v4()).bind(work).bind(fixture::dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        mapping.insert(original, json!({"candidateRef":work,"external":external}));
        note_refs.push(work);
    }
    let mut voice_refs = Vec::new();
    let mut comment_mapping = Vec::new();
    for entry in sample["commentContexts"].as_array().unwrap() {
        let item = &entry["item"];
        let context = &entry["context"];
        let original = item["sourceRef"].as_str().unwrap();
        let parent = &mapping[item["workRef"].as_str().unwrap()];
        let external = format!("private-review-{original}");
        let parent_comment = &context["parentComment"];
        let mut payload = json!({"commentId":external,"noteId":parent["external"],"text":item["text"]});
        let reply = parent_comment["state"] == "known";
        if reply {
            let parent_id = format!("private-review-{}", parent_comment["sourceRef"].as_str().unwrap());
            fixture::dossier::fixture::submit_package(db, "comments", json!({"contentExternalId":parent["external"]}),
                json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":parent["external"]},
                    "payload":{"commentId":parent_id,"noteId":parent["external"],"text":parent_comment["body"]}})).await;
            payload["parentCommentId"] = json!(parent_id);
            payload["rootCommentId"] = json!(parent_id);
        } else {
            assert_eq!(parent_comment["state"], "not_applicable", "private review requires complete original context");
        }
        fixture::dossier::fixture::submit_package(db, if reply {"replies"} else {"comments"}, json!({"contentExternalId":parent["external"]}),
            json!({"kind":if reply {"reply"} else {"comment"},"sourceObject":{"platform":"xhs","type":"content","externalId":parent["external"]},"payload":payload})).await;
        let reference: Uuid = sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE comment_external_id=$1")
            .bind(&external).fetch_one(db.pool()).await.unwrap();
        voice_refs.push(reference);
        comment_mapping.push(json!({"originalRef":original,"candidateRef":reference,"originalWorkRef":item["workRef"]}));
    }
    json!({"accountRef":account_ref,"publicationRef":publication,"noteRefs":note_refs,"voiceRefs":voice_refs,
        "replyRef":null,"evidenceKind":"private-real-text-copy","sourceMapping":mapping,"commentMapping":comment_mapping})
}
