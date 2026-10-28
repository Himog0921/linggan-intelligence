use super::fixture;
use base64::{Engine, engine::general_purpose::STANDARD};
use linggan_intelligence::{
    ai_foundation::{
        settings_read::{read_settings, read_skill_version},
        skill_package::{Encoding, SkillPackage, UploadFile},
        skill_store::{ImportSkill, import_skill},
        store::FoundationError,
    },
    pi_adapter::PiAdapter,
};
use serde_json::json;
use uuid::Uuid;

pub(super) fn package() -> SkillPackage {
    SkillPackage {directory:"example-skill".into(),files:vec![
        UploadFile {path:"SKILL.md".into(),encoding:Encoding::Utf8,content:"---\nname: example-skill\ndescription: A synthetic skill\nallowed-tools: [shell]\n---\nRead references/café.md.\n".into()},
        UploadFile {path:"references/café.md".into(),encoding:Encoding::Utf8,content:"Café\n".into()},
        UploadFile {path:"assets/example.bin".into(),encoding:Encoding::Base64,content:STANDARD.encode([0xff,0x00,0xfe])},
    ]}
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_skill_import_uses_pinned_loader_canonical_versions_and_sealed_files() {
    let db = fixture::proof_database("ai_foundation_skills").await;
    let adapter = PiAdapter::configured();
    let first = ImportSkill {
        command_ref: Uuid::new_v4(),
        expected_version_ref: None,
        package: package(),
    };
    let one = import_skill(&db, &adapter, &first).await.unwrap();
    assert_eq!(one.version_no, 1);
    assert!(!one.reused);
    assert_eq!(import_skill(&db, &adapter, &first).await.unwrap(), one);
    let snapshot = read_skill_version(&db, one.version_ref).await.unwrap();
    assert!(
        snapshot["frontmatter"]
            .as_str()
            .unwrap()
            .contains("allowed-tools")
    );
    assert!(
        snapshot.get("tools").is_none(),
        "frontmatter is archived data"
    );
    let invocation: String =
        sqlx::query_scalar("SELECT invocation FROM linggan_skill_version WHERE version_ref=$1")
            .bind(one.version_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(
        invocation.contains("<reference path=\"references/café.md\">\nCafé\n"),
        "batch invocation includes fixed text references"
    );
    assert!(
        !invocation.contains("assets/example.bin"),
        "binary bytes are retained but not interpreted as text"
    );
    assert_eq!(snapshot["files"].as_array().unwrap().len(), 3);
    let binary = snapshot["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "assets/example.bin")
        .unwrap();
    assert_eq!(binary["kind"], "binary");
    assert!(binary["content"].is_null());
    assert_eq!(binary["contentBase64"], STANDARD.encode([0xff, 0x00, 0xfe]));
    let mut normalized = ImportSkill {
        command_ref: Uuid::new_v4(),
        expected_version_ref: Some(one.version_ref),
        package: package(),
    };
    normalized.package.files.reverse();
    for file in &mut normalized.package.files {
        if matches!(file.encoding, Encoding::Utf8) {
            file.content = file.content.replace('é', "e\u{301}").replace('\n', "\r\n");
            file.path = file.path.replace('é', "e\u{301}");
        }
    }
    let reused = import_skill(&db, &adapter, &normalized).await.unwrap();
    assert_eq!(reused.version_ref, one.version_ref);
    assert!(reused.reused);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_skill_import")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(DISTINCT source_sha256) FROM linggan_skill_import"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
    );
    let mut second = ImportSkill {
        command_ref: Uuid::new_v4(),
        expected_version_ref: Some(one.version_ref),
        package: package(),
    };
    second.package.files[0]
        .content
        .push_str("A changed method.\n");
    let two = import_skill(&db, &adapter, &second).await.unwrap();
    assert_eq!(two.version_no, 2);
    assert_ne!(two.package_sha256, one.package_sha256);
    assert_eq!(
        read_skill_version(&db, one.version_ref).await.unwrap(),
        snapshot
    );
    let mut stale = second.clone();
    stale.command_ref = Uuid::new_v4();
    assert!(matches!(
        import_skill(&db, &adapter, &stale).await,
        Err(FoundationError::Conflict)
    ));
    let mut invalid = second.clone();
    invalid.command_ref = Uuid::new_v4();
    invalid.expected_version_ref = Some(two.version_ref);
    invalid.package.files[0].content = invalid.package.files[0]
        .content
        .replace("name: example-skill", "name: other");
    assert!(matches!(
        import_skill(&db, &adapter, &invalid).await,
        Err(FoundationError::Invalid)
    ));
    assert!(
        sqlx::query("UPDATE linggan_skill_version SET body='changed' WHERE version_ref=$1")
            .bind(one.version_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE linggan_skill_file SET path='changed' WHERE version_ref=$1")
            .bind(one.version_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(sqlx::query("INSERT INTO linggan_skill_file(version_ref,path,bytes,text_content,sha256) SELECT version_ref,'extra.md',bytes,text_content,sha256 FROM linggan_skill_file WHERE version_ref=$1 AND path='SKILL.md'").bind(one.version_ref).execute(db.pool()).await.is_err(),"files cannot be appended after sealing");
    let incomplete = Uuid::new_v4();
    assert!(sqlx::query("INSERT INTO linggan_skill_version(version_ref,skill_ref,directory_name,version_no,description,body,frontmatter,disable_model_invocation,invocation,loader_version,normalization_version,package_sha256,source_sha256) SELECT $1,skill_ref,directory_name,3,description,body,frontmatter,disable_model_invocation,invocation,loader_version,normalization_version,repeat('0',64),source_sha256 FROM linggan_skill_version WHERE version_ref=$2")
        .bind(incomplete).bind(two.version_ref).execute(db.pool()).await.is_err(),"incomplete versions cannot commit");
    let settings = read_settings(&db, true).await.unwrap();
    for oversized_count in [true, false] {
        let mut tx = db.pool().begin().await.unwrap();
        let version = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_skill_version(version_ref,skill_ref,directory_name,version_no,description,body,frontmatter,disable_model_invocation,invocation,loader_version,normalization_version,package_sha256,source_sha256) SELECT $1,skill_ref,directory_name,3,description,body,frontmatter,disable_model_invocation,invocation,loader_version,normalization_version,repeat('0',64),source_sha256 FROM linggan_skill_version WHERE version_ref=$2")
            .bind(version).bind(two.version_ref).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO linggan_skill_file(version_ref,path,bytes,text_content,sha256) SELECT $1,path,convert_to(body,'UTF8'),body,encode(sha256(convert_to(body,'UTF8')),'hex') FROM (
            SELECT CASE WHEN n=1 THEN 'SKILL.md' ELSE 'ref-'||n||'.md' END AS path,
            CASE WHEN $2 OR n=1 THEN 'x' ELSE repeat('x',262144) END AS body
            FROM generate_series(1,CASE WHEN $2 THEN 65 ELSE 5 END)n)files")
            .bind(version).bind(oversized_count).execute(&mut *tx).await.unwrap();
        assert!(
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await
                .is_err(),
            "aggregate file count/bytes must be checked before a version can commit"
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(settings["skills"].as_array().unwrap().len(), 1);
    assert_eq!(
        settings["skills"][0]["versions"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        settings["skills"][0]["versions"][0]["versionRef"],
        json!(two.version_ref)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}
