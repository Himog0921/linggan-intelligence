//! P4 proof: a local algorithm result stays non-authoritative until bounded
//! review, and every resulting group keeps current-membership and lineage
//! boundaries when inputs change.  The only providers here are loopback
//! synthetic fixtures; the Python bridge receives only synthetic vectors.

#[path = "support/comment_daily_fixture.rs"]
mod daily_fixture;
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use linggan_evidence::comment_research_read::restrict_comment_research_source;
use linggan_intelligence::{
    TopicMaterialMemberImport, TopicMaterialRole, TopicWorkspaceImport,
    comment_daily::{
        AutoPolicy, DailySchedule, SelectedBatch, create_selected, run_daily_once, save_schedule,
    },
    comment_intelligence::{ResearchScope, read as read_comment_intelligence},
    comment_intelligence_problems::problem_details,
    comment_research::comment_source_hash,
    comment_semantic_compute::ControlledSemanticCompute,
    comment_semantic_organization,
    embedding_settings::{self, ProbeEmbedding, SaveEmbedding},
    import_topic_workspace,
    model_invocation::{ProbeModel, probe_model},
    model_runner::run_model_work_once,
    model_secrets::SyntheticModelSecrets,
    model_settings::{
        SaveModelConfig, SaveModelConnection, SaveModelEntry, save_model_config,
        save_model_connection, save_model_entry,
    },
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::{path::PathBuf, process::Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;

const SYNTHETIC_POLICY: &str = "synthetic_review_response_not_compute_evidence";

fn hex_hash(hash: [u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct ReviewServer(tokio::process::Child, String);

impl ReviewServer {
    async fn start() -> Self {
        let node = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join(".nvm/versions/node/v24.13.0/bin/node");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_semantic_review_server.mjs");
        let mut child = tokio::process::Command::new(node)
            .arg(script)
            .env_clear()
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .await
            .unwrap();
        Self(child, line.trim().into())
    }

    async fn stop(mut self) {
        self.0.kill().await.unwrap();
    }
}

#[derive(Clone)]
struct Atom {
    atom_ref: Uuid,
    analysis_ref: Uuid,
    domain_ref: Uuid,
    canonical_ref: Uuid,
    source_ref: Uuid,
    evidence: Value,
}

#[derive(Debug)]
struct ComputeProgress {
    progressed: bool,
    atoms: i64,
    vectors: i64,
    runs: i64,
    embedding_work: i64,
    spaces: i64,
    embedding_active: bool,
}

async fn enable_embeddings(db: &Database, model_ref: Uuid) {
    let disabled = embedding_settings::save(
        db,
        &SaveEmbedding {
            expected_revision: 0,
            model_ref,
            enabled: false,
        },
    )
    .await
    .unwrap();
    let config_ref = Uuid::parse_str(disabled["configRef"].as_str().unwrap()).unwrap();
    assert!(
        embedding_settings::probe(
            db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            &ProbeEmbedding {
                invocation_ref: Uuid::new_v4(),
                config_ref,
            },
        )
        .await
        .unwrap()["embeddingQualified"]
            .as_bool()
            .unwrap()
    );
    let revision = disabled["revision"].as_i64().unwrap();
    assert!(
        embedding_settings::save(
            db,
            &SaveEmbedding {
                expected_revision: revision,
                model_ref,
                enabled: true,
            },
        )
        .await
        .unwrap()["enabled"]
            .as_bool()
            .unwrap()
    );
}

async fn current_atoms(db: &Database) -> Vec<Atom> {
    sqlx::query(
        "SELECT atom_ref,analysis_ref,domain_ref,canonical_ref,source_ref,evidence \
         FROM linggan_ci_semantic_atom_current WHERE kind='problem' ORDER BY atom_ref",
    )
    .fetch_all(db.pool())
    .await
    .unwrap()
    .into_iter()
    .map(|row| Atom {
        atom_ref: row.get("atom_ref"),
        analysis_ref: row.get("analysis_ref"),
        domain_ref: row.get("domain_ref"),
        canonical_ref: row.get("canonical_ref"),
        source_ref: row.get("source_ref"),
        evidence: row.get("evidence"),
    })
    .collect()
}

async fn insert_review_run(
    db: &Database,
    domain_ref: Uuid,
    kind: &str,
    space_ref: Uuid,
    atoms: &[Uuid],
    clusters: &[Vec<Uuid>],
    tag: &str,
) -> Uuid {
    let run_ref = Uuid::new_v4();
    let snapshot_hash = comment_source_hash(&format!("semantic-review-fixture:{tag}"));
    let policy_hash = hex_hash(ControlledSemanticCompute::controlled_policy_sha256().unwrap());
    sqlx::query(
        "INSERT INTO linggan_ci_cluster_run(run_ref,domain_ref,kind,space_ref,snapshot_hash,vector_hash,policy_hash,state,member_count,quality) \
         VALUES($1,$2,$3,$4,$5,$6,$7,'reviewing',$8,$9)",
    )
    .bind(run_ref)
    .bind(domain_ref)
    .bind(kind)
    .bind(space_ref)
    .bind(&snapshot_hash)
    .bind(comment_source_hash(&format!("vectors:{tag}")))
    .bind(policy_hash)
    .bind(atoms.len() as i32)
    .bind(json!({"evidenceClass":SYNTHETIC_POLICY}))
    .execute(db.pool())
    .await
    .unwrap();
    for (ordinal, atom_ref) in atoms.iter().enumerate() {
        let definition_hash: String = sqlx::query_scalar(
            "SELECT definition_hash FROM linggan_ci_semantic_atom WHERE atom_ref=$1",
        )
        .bind(atom_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_ci_cluster_input(run_ref,ordinal,atom_ref,definition_hash) \
             VALUES($1,$2,$3,$4)",
        )
        .bind(run_ref)
        .bind(ordinal as i32)
        .bind(atom_ref)
        .bind(definition_hash)
        .execute(db.pool())
        .await
        .unwrap();
    }
    for (cluster, members) in clusters.iter().enumerate() {
        for atom_ref in members {
            sqlx::query(
                "INSERT INTO linggan_ci_cluster_assignment(run_ref,atom_ref,algorithm,cluster_key) \
                 VALUES($1,$2,'leiden',$3)",
            )
            .bind(run_ref)
            .bind(atom_ref)
            .bind((cluster + 1).to_string())
            .execute(db.pool())
            .await
            .unwrap();
        }
        let core = members.iter().take(5).copied().collect::<Vec<_>>();
        let boundary = members.iter().skip(5).take(4).copied().collect::<Vec<_>>();
        let neighbors = atoms
            .iter()
            .copied()
            .filter(|atom_ref| !members.contains(atom_ref))
            .take(3)
            .collect::<Vec<_>>();
        let sample_refs = core
            .iter()
            .chain(&boundary)
            .chain(&neighbors)
            .copied()
            .collect::<Vec<_>>();
        assert!((1..=12).contains(&sample_refs.len()));
        sqlx::query(
            "INSERT INTO linggan_ci_cluster_review(review_ref,run_ref,cluster_key,algorithm,sample_refs,snapshot_hash,state,receipt) \
             VALUES($1,$2,$3,'leiden',$4,$5,'pending',$6)",
        )
        .bind(Uuid::new_v4())
        .bind(run_ref)
        .bind((cluster + 1).to_string())
        .bind(sample_refs)
        .bind(&snapshot_hash)
        .bind(json!({
            "evidenceClass":SYNTHETIC_POLICY,
            "sampleRoles":{"coreAtomRefs":core,"boundaryAtomRefs":boundary,"neighborAtomRefs":neighbors}
        }))
        .execute(db.pool())
        .await
        .unwrap();
    }
    run_ref
}

async fn add_need_atoms(db: &Database, atoms: &[Atom], space_ref: Uuid) -> Vec<Uuid> {
    let hash = comment_source_hash("synthetic-need-definition-v1");
    let vector = sqlx::query(
        "SELECT vector_bytes,dimensions,invocation_ref FROM linggan_ci_atom_vector WHERE space_ref=$1 LIMIT 1",
    )
    .bind(space_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ci_atom_vector(space_ref,definition_hash,vector_bytes,dimensions,invocation_ref) \
         VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",
    )
    .bind(space_ref)
    .bind(&hash)
    .bind(vector.get::<Vec<u8>, _>("vector_bytes"))
    .bind(vector.get::<i32, _>("dimensions"))
    .bind(vector.get::<Uuid, _>("invocation_ref"))
    .execute(db.pool())
    .await
    .unwrap();
    let mut need_atoms = Vec::with_capacity(atoms.len());
    for atom in atoms {
        let atom_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO linggan_ci_semantic_atom(atom_ref,analysis_ref,domain_ref,canonical_ref,source_ref,kind,ordinal,atom_contract_version,definition_hash,meaning,evidence) \
             VALUES($1,$2,$3,$4,$5,'need',1,'comment-atoms.v4-adapter.1',$6,'合成需要稳定的起步支持',$7)",
        )
        .bind(atom_ref)
        .bind(atom.analysis_ref)
        .bind(atom.domain_ref)
        .bind(atom.canonical_ref)
        .bind(atom.source_ref)
        .bind(&hash)
        .bind(&atom.evidence)
        .execute(db.pool())
        .await
        .unwrap();
        need_atoms.push(atom_ref);
    }
    need_atoms
}

async fn run_review_until_terminal(db: &Database) {
    for _ in 0..4 {
        if !comment_semantic_organization::run_once(
            db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            None,
        )
        .await
        .unwrap()
        {
            break;
        }
    }
}

async fn import_derived_topic_fixture(db: &Database, associated_work: Uuid) -> Uuid {
    research_fixture::detail(
        db,
        "semantic-topic-boundary-work",
        "SYNTHETIC / NOT EVIDENCE · unrelated Topic boundary work",
    )
    .await;
    let boundary_work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id='semantic-topic-boundary-work'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let receipt = import_topic_workspace(
        db,
        &TopicWorkspaceImport {
            idempotency_key: "semantic-proof:derived-topic:0001".into(),
            domain_key: "semantic-proof".into(),
            canonical_key: "semantic-proof-associated-topic".into(),
            display_name: "合成关联主题".into(),
            definition_text: "仅用于证明既有 Topic 作品归属会派生为评论语义组的可读出处。".into(),
            expected_version: None,
            adjudication_note: "合成的人为 Topic 裁定，仅作 P4 隔离证明。".into(),
            source_boundary: "一条语义组来源作品和一条不在该组内的边界作品。".into(),
            members: vec![
                TopicMaterialMemberImport {
                    work_public_ref: associated_work,
                    role: TopicMaterialRole::Support,
                    rationale: "该作品是当前合成语义组中一条原子的既有来源。".into(),
                },
                TopicMaterialMemberImport {
                    work_public_ref: boundary_work,
                    role: TopicMaterialRole::Boundary,
                    rationale: "该作品满足正式 Topic 的边界材料要求，但不属于该语义组。".into(),
                },
            ],
        },
    )
    .await
    .unwrap();
    receipt.topic_ref
}

async fn create_actual_sources(db: &Database, daily_config: Uuid) {
    research_fixture::detail(db, "semantic-proof", "SYNTHETIC / NOT EVIDENCE").await;
    let mut sources = Vec::new();
    for index in 0..20 {
        sources.push(
            daily_fixture::source(
                db,
                "semantic-proof",
                &format!("semantic-{index}"),
                "SYNTHETIC / NOT EVIDENCE：每天提醒以后还是很难开始，需要稳定的起步支持。",
            )
            .await,
        );
    }
    daily_fixture::selected(db, daily_config, sources, 100_000).await;
}

async fn await_actual_compute(db: &Database) -> Vec<ComputeProgress> {
    let mut progress = Vec::new();
    for _ in 0..12 {
        let progressed = run_model_work_once(db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap();
        let atoms: i64 =
            sqlx::query_scalar("SELECT count(*) FROM linggan_ci_semantic_atom_current")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let vectors: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_atom_vector")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_cluster_run")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let embedding_work: i64 =
            sqlx::query_scalar("SELECT count(*) FROM linggan_ci_atom_embedding_work")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let spaces: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_semantic_space")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let embedding_active = embedding_settings::active_config(db)
            .await
            .unwrap()
            .is_some();
        progress.push(ComputeProgress {
            progressed,
            atoms,
            vectors,
            runs,
            embedding_work,
            spaces,
            embedding_active,
        });
        if runs > 0 {
            break;
        }
    }
    progress
}

async fn assert_zero_semantic_budget_blocks_dispatch(
    db: &Database,
    progress: &[ComputeProgress],
    embedding_probe_calls: i64,
) {
    let terminal = progress
        .last()
        .expect("daily fixture must record worker ticks");
    assert_eq!(terminal.atoms, 20);
    assert_eq!(terminal.vectors, 0);
    assert_eq!(terminal.runs, 0);
    assert!(terminal.embedding_active);
    assert!(terminal.spaces > 0 && terminal.embedding_work > 0);
    let embed_calls: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE operation='embed'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        embed_calls, embedding_probe_calls,
        "a zero semantic budget may enqueue bounded work but cannot start a vector provider call"
    );
}

async fn configure_semantic_budget(db: &Database, config_ref: Uuid) {
    let revision: i32 =
        sqlx::query_scalar("SELECT revision FROM linggan_comment_daily_schedule WHERE singleton")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let auto_policy = AutoPolicy {
        day_token_limit: 100_000,
        semantic_token_limit: 80_000,
        ..AutoPolicy::default()
    };
    let receipt = save_schedule(
        db,
        &DailySchedule {
            expected_revision: revision,
            enabled: false,
            config_ref,
            source_limit: 100,
            token_limit: 100_000,
            auto_policy,
        },
    )
    .await
    .unwrap();
    assert_eq!(receipt["enabled"], json!(false));
    assert_eq!(receipt["autoPolicy"]["semanticTokenLimit"], json!(80_000));
}

async fn assert_actual_compute_quality_gate(
    db: &Database,
    progress: &[ComputeProgress],
) -> (Vec<Atom>, Uuid) {
    let progressed_ticks = progress.iter().filter(|item| item.progressed).count();
    let final_embedding_state = progress
        .last()
        .map(|last| (last.embedding_active, last.spaces, last.embedding_work));
    assert!(
        progress
            .last()
            .is_some_and(|last| { last.atoms == 20 && last.vectors > 0 && last.runs > 0 }),
        "full daily-to-atom-to-vector-to-local-compute chain did not complete; progressedTicks={progressed_ticks}, finalEmbeddingState={final_embedding_state:?}, progress={progress:?}"
    );
    let automatic_work: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_ci_atom_embedding_work")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let automatic_vectors: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_atom_vector")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let automatic_spaces: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_ci_semantic_space")
            .fetch_one(db.pool())
            .await
            .unwrap();
    println!(
        "P4 automatic-vector observation: currentAtoms=20 embeddingWork={automatic_work} vectors={automatic_vectors} spaces={automatic_spaces} activeEmbedding={}",
        embedding_settings::active_config(db)
            .await
            .unwrap()
            .unwrap()
    );
    let actual = sqlx::query(
        "SELECT state,failure_code,quality FROM linggan_ci_cluster_run ORDER BY created_at LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        actual.get::<String, _>("state"),
        "insufficient",
        "controlled local compute must report a valid hard-gate miss, failureCode={:?}, quality={}",
        actual.get::<Option<String>, _>("failure_code"),
        actual.get::<Value, _>("quality")
    );
    assert_eq!(
        actual.get::<Value, _>("quality")["acceptedForReview"],
        json!(false)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_semantic_group")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "real local compute cannot create a formal problem before its frozen gates and review"
    );
    let atoms = current_atoms(db).await;
    assert_eq!(atoms.len(), 20);
    let space_ref: Uuid =
        sqlx::query_scalar("SELECT space_ref FROM linggan_ci_atom_vector LIMIT 1")
            .fetch_one(db.pool())
            .await
            .unwrap();
    (atoms, space_ref)
}

async fn accept_initial_problem_group(db: &Database, atoms: &[Atom], space_ref: Uuid) -> Uuid {
    let all_problem_atoms = atoms.iter().map(|atom| atom.atom_ref).collect::<Vec<_>>();
    let initial = insert_review_run(
        db,
        atoms[0].domain_ref,
        "problem",
        space_ref,
        &all_problem_atoms,
        std::slice::from_ref(&all_problem_atoms),
        "initial",
    )
    .await;
    run_review_until_terminal(db).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_ci_cluster_run WHERE run_ref=$1"
        )
        .bind(initial)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "accepted"
    );
    let first_group: Uuid = sqlx::query_scalar(
        "SELECT group_ref FROM linggan_ci_semantic_group WHERE kind='problem' AND state='active'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_atom_membership WHERE group_ref=$1 AND current AND relation='same'",
        )
        .bind(first_group)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        19
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_atom_membership WHERE group_ref=$1 AND current AND relation='related'",
        )
        .bind(first_group)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    first_group
}

async fn prove_split(db: &Database, atoms: &[Atom], space_ref: Uuid, first_group: Uuid) {
    let all_problem_atoms = atoms.iter().map(|atom| atom.atom_ref).collect::<Vec<_>>();
    let split = insert_review_run(
        db,
        atoms[0].domain_ref,
        "problem",
        space_ref,
        &all_problem_atoms,
        &[
            all_problem_atoms[..10].to_vec(),
            all_problem_atoms[10..].to_vec(),
        ],
        "split",
    )
    .await;
    run_review_until_terminal(db).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_ci_cluster_run WHERE run_ref=$1"
        )
        .bind(split)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "accepted"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_ci_semantic_group WHERE group_ref=$1"
        )
        .bind(first_group)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "superseded"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_semantic_group WHERE kind='problem' AND state='active'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_semantic_lineage WHERE kind='split' AND run_ref=$1",
        )
        .bind(split)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

async fn prove_merge(db: &Database, atoms: &[Atom], space_ref: Uuid) -> (Uuid, Uuid) {
    let all_problem_atoms = atoms.iter().map(|atom| atom.atom_ref).collect::<Vec<_>>();
    let merge = insert_review_run(
        db,
        atoms[0].domain_ref,
        "problem",
        space_ref,
        &all_problem_atoms,
        std::slice::from_ref(&all_problem_atoms),
        "merge",
    )
    .await;
    run_review_until_terminal(db).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_semantic_group WHERE kind='problem' AND state='active'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_semantic_lineage WHERE kind='merge' AND run_ref=$1",
        )
        .bind(merge)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    let merged = sqlx::query(
        "SELECT group_ref,problem_ref FROM linggan_ci_semantic_group \
         WHERE kind='problem' AND state='active'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let merged_group: Uuid = merged.get("group_ref");
    let merged_problem: Uuid = merged.get("problem_ref");
    (merged_group, merged_problem)
}

async fn assert_organization_change_is_visible_but_not_interpreted(
    db: &Database,
    domain_ref: Uuid,
    problem_ref: Uuid,
) {
    // The read query deliberately excludes the unfinished current day. Advance
    // only the isolated test clock so the fixture's real admission day is a
    // closed current comparison window rather than a partial day.
    daily_fixture::clock(db, "2026-09-10T01:00:00Z").await;
    let value = read_comment_intelligence(
        db,
        &ResearchScope {
            domain: Some(domain_ref),
            from: Some("2026-08-01T00:00:00Z".into()),
            to: Some("2026-09-30T00:00:00Z".into()),
            ..ResearchScope::default()
        },
    )
    .await
    .unwrap();
    let comparison = value["comparisons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["problemRef"] == json!(problem_ref))
        .expect("the merged semantic problem has comparison windows");
    let current = &comparison["currentWindow"];
    assert!(current["organizationChanges"].as_i64().unwrap_or_default() > 0);
    assert!(
        current["semanticSignature"]
            .as_str()
            .is_some_and(|signature| !signature.is_empty())
    );
    assert!(current["organizationEligible"].is_i64());
    assert!(current["organized"].is_i64());
    assert_eq!(
        comparison["comparison"]["reason"], "样本不足：每个窗口至少需要30条有效评论",
        "organization changes are visible, but a 20-atom synthetic fixture cannot claim a discussion change"
    );
}

async fn prove_topic_derivation(
    db: &Database,
    atom: &Atom,
    merged_group: Uuid,
    merged_problem: Uuid,
) -> Uuid {
    let topic_work: Uuid = sqlx::query_scalar(
        "SELECT work_ref FROM linggan_ci_source WHERE canonical_ref=$1 AND domain_ref=$2",
    )
    .bind(atom.canonical_ref)
    .bind(atom.domain_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let before_topic = problem_details(db, atom.domain_ref, merged_problem)
        .await
        .unwrap();
    let formal_problem_before: (String, String, Value, i64) = (
        before_topic["problem"]["name"].as_str().unwrap().into(),
        before_topic["problem"]["meaning"].as_str().unwrap().into(),
        before_topic["problem"]["definition"].clone(),
        sqlx::query_scalar("SELECT count(*) FROM linggan_ci_problem_member WHERE problem_ref=$1")
            .bind(merged_problem)
            .fetch_one(db.pool())
            .await
            .unwrap(),
    );
    let topic_ref = import_derived_topic_fixture(db, topic_work).await;
    let _ = run_model_work_once(db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let after_topic = problem_details(db, atom.domain_ref, merged_problem)
        .await
        .unwrap();
    assert_eq!(after_topic["problem"]["name"], formal_problem_before.0);
    assert_eq!(after_topic["problem"]["meaning"], formal_problem_before.1);
    assert_eq!(
        after_topic["problem"]["definition"],
        formal_problem_before.2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_problem_member WHERE problem_ref=$1",
        )
        .bind(merged_problem)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        formal_problem_before.3,
        "derived Topic provenance must not mutate the formal problem projection"
    );
    assert_eq!(
        after_topic["relatedTopics"][0]["topicRef"],
        json!(topic_ref)
    );
    let association_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ci_comment_topic_association \
             WHERE group_ref=$1 AND topic_ref=$2",
    )
    .bind(merged_group)
    .bind(topic_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let current_same_members: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ci_atom_membership \
         WHERE group_ref=$1 AND current AND relation='same'",
    )
    .bind(merged_group)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        association_count, current_same_members,
        "every current same member whose source work belongs to the Topic retains provenance"
    );
    topic_ref
}

async fn assert_repeat_is_idempotent(db: &Database) {
    let problems_before_repeat: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_problem")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(
        !comment_semantic_organization::run_once(
            db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            None,
        )
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_problem")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        problems_before_repeat,
        "replaying an accepted frozen snapshot cannot duplicate formal problems"
    );
}

async fn prove_kind_isolation(db: &Database, atoms: &[Atom], space_ref: Uuid) {
    let need_atoms = add_need_atoms(db, atoms, space_ref).await;
    let need = insert_review_run(
        db,
        atoms[0].domain_ref,
        "need",
        space_ref,
        &need_atoms,
        std::slice::from_ref(&need_atoms),
        "need-isolation",
    )
    .await;
    run_review_until_terminal(db).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_ci_cluster_run WHERE run_ref=$1"
        )
        .bind(need)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "accepted"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_semantic_group WHERE kind='need' AND state='active' AND problem_ref IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1,
        "same text in a different atom kind cannot reuse a problem group"
    );
}

async fn withdraw_topic_sources(
    db: &Database,
    merged_group: Uuid,
    topic_ref: Uuid,
    expected_source: Uuid,
) -> i64 {
    let topic_sources: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT source.source_ref \
         FROM linggan_ci_comment_topic_association link \
         JOIN linggan_ci_semantic_atom atom ON atom.atom_ref=link.atom_ref AND atom.analysis_ref=link.analysis_ref \
         JOIN linggan_ci_source source ON source.canonical_ref=atom.canonical_ref \
         WHERE link.group_ref=$1 AND link.topic_ref=$2",
    )
    .bind(merged_group)
    .bind(topic_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(topic_sources.contains(&expected_source));
    assert!(!topic_sources.is_empty());
    for source_ref in &topic_sources {
        restrict_comment_research_source(db, *source_ref, "synthetic Topic source withdrawal")
            .await
            .unwrap();
    }
    topic_sources.len() as i64
}

async fn assert_withdrawal_retires_current_projections(
    db: &Database,
    model_ref: Uuid,
    withdrawn: &Atom,
    merged_group: Uuid,
    merged_problem: Uuid,
    topic_ref: Uuid,
) {
    let settings = embedding_settings::read(db).await.unwrap();
    embedding_settings::save(
        db,
        &SaveEmbedding {
            expected_revision: settings["revision"].as_i64().unwrap(),
            model_ref,
            enabled: false,
        },
    )
    .await
    .unwrap();
    let receipt_count =
        withdraw_topic_sources(db, merged_group, topic_ref, withdrawn.source_ref).await;
    assert!(
        !comment_semantic_organization::run_once(
            db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            None,
        )
        .await
        .unwrap()
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT current FROM linggan_ci_atom_membership WHERE atom_ref=$1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(withdrawn.atom_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT superseded_reason FROM linggan_ci_atom_membership WHERE atom_ref=$1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(withdrawn.atom_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "source_withdrawn"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_problem_member member \
             JOIN linggan_ci_semantic_group group_row ON group_row.problem_ref=member.problem_ref \
             WHERE group_row.group_ref=$1 AND member.origin='model_equivalence'",
        )
        .bind(merged_group)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0,
        "withdrawn source evidence is not retained in the current formal-problem projection"
    );
    let unavailable = problem_details(db, withdrawn.domain_ref, merged_problem)
        .await
        .unwrap_err();
    assert!(matches!(
        unavailable,
        linggan_storage_postgres::StorageError::Statement(sqlx::Error::Protocol(code))
            if code == "CI_PROBLEM_UNAVAILABLE"
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_comment_topic_association \
             WHERE group_ref=$1 AND topic_ref=$2",
        )
        .bind(merged_group)
        .bind(topic_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        receipt_count,
        "the immutable Topic source-work association receipts remain after all associated sources are withdrawn"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL, controlled local Python, and loopback synthetic review"]
async fn local_compute_quality_gate_and_review_lineage_keep_current_memberships_sound() {
    let db = fixture::proof_database("semantic_organization").await;
    let (mut daily_server, daily_url) = daily_fixture::fixture_server().await;
    let (daily_config, _, _) =
        daily_fixture::configured(&db, &daily_url, "synthetic-good", None).await;
    let model_ref: Uuid =
        sqlx::query_scalar("SELECT model_ref FROM linggan_model_config WHERE config_ref=$1")
            .bind(daily_config)
            .fetch_one(db.pool())
            .await
            .unwrap();
    enable_embeddings(&db, model_ref).await;
    assert!(
        embedding_settings::active_config(&db)
            .await
            .unwrap()
            .is_some()
    );
    let embedding_probe_calls: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE operation='embed'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    create_actual_sources(&db, daily_config).await;
    let zero_budget_progress = await_actual_compute(&db).await;
    assert_zero_semantic_budget_blocks_dispatch(&db, &zero_budget_progress, embedding_probe_calls)
        .await;
    configure_semantic_budget(&db, daily_config).await;
    let progress = await_actual_compute(&db).await;
    let (atoms, space_ref) = assert_actual_compute_quality_gate(&db, &progress).await;
    let review_server = ReviewServer::start().await;
    daily_fixture::configured(
        &db,
        &review_server.1,
        "synthetic-semantic-review",
        Some(daily_config),
    )
    .await;
    let first_group = accept_initial_problem_group(&db, &atoms, space_ref).await;
    prove_split(&db, &atoms, space_ref, first_group).await;
    let (merged_group, merged_problem) = prove_merge(&db, &atoms, space_ref).await;
    assert_organization_change_is_visible_but_not_interpreted(
        &db,
        atoms[0].domain_ref,
        merged_problem,
    )
    .await;
    let topic_ref = prove_topic_derivation(&db, &atoms[0], merged_group, merged_problem).await;
    assert_repeat_is_idempotent(&db).await;
    prove_kind_isolation(&db, &atoms, space_ref).await;
    assert_withdrawal_retires_current_projections(
        &db,
        model_ref,
        &atoms[0],
        merged_group,
        merged_problem,
        topic_ref,
    )
    .await;
    review_server.stop().await;
    daily_server.kill().await.unwrap();
}
