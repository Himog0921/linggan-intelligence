use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use crate::{
    comment_research::comment_source_hash,
    comment_semantic_compute::{
        SemanticAtomKind, SemanticComputeMode, SemanticComputeRequest, SemanticNormalization,
        SemanticSnapshot, SemanticVectorSpace,
    },
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

pub(crate) struct FrozenSnapshot {
    pub(crate) run_ref: Uuid,
    pub(crate) domain_ref: Uuid,
    pub(crate) kind: String,
    pub(crate) space_ref: Uuid,
    pub(crate) request: SemanticComputeRequest,
    pub(crate) atom_refs: Vec<Uuid>,
    definition_hashes: Vec<String>,
    pub(crate) run_snapshot_hash: String,
    pub(crate) policy_hash: String,
    pub(crate) space_hash: String,
    work_dir: PathBuf,
}

struct Candidate {
    domain_ref: Uuid,
    kind: String,
    space_ref: Uuid,
    model_id: String,
    model_version: String,
    dimensions: i32,
    normalization: String,
    space_hash: String,
}

struct AtomVector {
    atom_ref: Uuid,
    definition_hash: String,
    bytes: Vec<u8>,
}

pub(crate) async fn freeze_next(
    db: &Database,
    policy_hash: [u8; 32],
) -> Result<Option<FrozenSnapshot>, ModelError> {
    let policy_hash = hex_hash(policy_hash);
    for candidate in next_candidates(db).await? {
        let atoms = atom_vectors(db, &candidate).await?;
        if atoms.is_empty() {
            continue;
        }
        let expected_bytes = usize::try_from(candidate.dimensions)
            .ok()
            .and_then(|dimensions| dimensions.checked_mul(4))
            .ok_or(ModelError::Invalid)?;
        if !(2..=8192).contains(&candidate.dimensions)
            || candidate.normalization != "l2"
            || atoms.iter().any(|atom| {
                atom.bytes.len() != expected_bytes || !valid_hash(&atom.definition_hash)
            })
        {
            return Err(ModelError::InvalidOutput);
        }
        let run_snapshot_hash = run_hash(&candidate, &atoms);
        if completed_snapshot_exists(db, &candidate, &run_snapshot_hash, &policy_hash).await?
            || recently_failed(db, &candidate, &run_snapshot_hash, &policy_hash).await?
        {
            continue;
        }
        return freeze_candidate(candidate, atoms, run_snapshot_hash, policy_hash);
    }
    Ok(None)
}

fn freeze_candidate(
    candidate: Candidate,
    atoms: Vec<AtomVector>,
    run_snapshot_hash: String,
    policy_hash: String,
) -> Result<Option<FrozenSnapshot>, ModelError> {
    let kind = atom_kind(&candidate.kind)?;
    let space_hash = parse_hash(&candidate.space_hash)?;
    let run_ref = Uuid::new_v4();
    let work_dir = std::env::temp_dir().join(format!("ci-auto-semantics-{run_ref}"));
    fs::create_dir(&work_dir).map_err(|_| ModelError::Source)?;
    let vector_path = work_dir.join("vectors.f32");
    if let Err(error) = write_vectors(&vector_path, &atoms) {
        let _ = fs::remove_dir_all(&work_dir);
        return Err(error);
    }
    let vector_hash = sha256_file(&vector_path)?;
    let atom_refs = atoms.iter().map(|atom| atom.atom_ref).collect::<Vec<_>>();
    let definition_hashes = atoms
        .iter()
        .map(|atom| atom.definition_hash.clone())
        .collect::<Vec<_>>();
    let request = SemanticComputeRequest {
        work_dir: work_dir.clone(),
        snapshot: SemanticSnapshot {
            run_ref,
            kind,
            space: SemanticVectorSpace {
                model: candidate.model_id,
                version: candidate.model_version,
                dimension: candidate.dimensions as usize,
                normalization: SemanticNormalization::L2,
            },
            space_hash,
            atom_ids: atom_refs.clone(),
            vector_path,
            snapshot_sha256: vector_hash,
            // A whole small input can be compared directly. Larger inputs get
            // a separate frozen 10k compare run before their whole-input
            // Leiden candidate is admitted below.
            mode: if atom_refs.len() <= 10_000 {
                SemanticComputeMode::Compare
            } else {
                SemanticComputeMode::Leiden
            },
        },
    };
    Ok(Some(FrozenSnapshot {
        run_ref,
        domain_ref: candidate.domain_ref,
        kind: candidate.kind,
        space_ref: candidate.space_ref,
        request,
        atom_refs,
        definition_hashes,
        run_snapshot_hash,
        policy_hash,
        space_hash: candidate.space_hash,
        work_dir,
    }))
}

/// Avoid requiring the local Python runtime when there is no current complete
/// vector set to organize. The full freeze below still rechecks every selected
/// atom and definition hash transactionally before any run is reserved.
pub(crate) async fn has_current_vector_candidate(db: &Database) -> Result<bool, ModelError> {
    Ok(!next_candidates(db).await?.is_empty())
}

pub(crate) async fn reserve(db: &Database, frozen: &FrozenSnapshot) -> Result<bool, ModelError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *transaction)
        .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_ci_cluster_run \
         WHERE domain_ref=$1 AND kind=$2 AND space_ref=$3 AND snapshot_hash=$4 AND policy_hash=$5 \
           AND state<>'failed')",
    )
    .bind(frozen.domain_ref)
    .bind(&frozen.kind)
    .bind(frozen.space_ref)
    .bind(&frozen.run_snapshot_hash)
    .bind(&frozen.policy_hash)
    .fetch_one(&mut *transaction)
    .await?;
    if exists {
        transaction.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO linggan_ci_cluster_run(run_ref,domain_ref,kind,space_ref,snapshot_hash,vector_hash,policy_hash,state,member_count,lease_until,attempts,quality) \
         VALUES($1,$2,$3,$4,$5,$6,$7,'running',$8,scope_001_now()+interval '30 minutes',1,$9)",
    )
    .bind(frozen.run_ref)
    .bind(frozen.domain_ref)
    .bind(&frozen.kind)
    .bind(frozen.space_ref)
    .bind(&frozen.run_snapshot_hash)
    .bind(hex_hash(frozen.request.snapshot.snapshot_sha256))
    .bind(&frozen.policy_hash)
    .bind(frozen.atom_refs.len() as i32)
    .bind(serde_json::json!({"spaceHash": frozen.space_hash, "runSnapshotHash": frozen.run_snapshot_hash, "status":"reserved"}))
    .execute(&mut *transaction)
    .await?;
    let current: Vec<(Uuid, String)> = sqlx::query(
        "SELECT atom_ref,definition_hash FROM linggan_ci_semantic_atom_current \
         WHERE atom_ref=ANY($1) ORDER BY atom_ref",
    )
    .bind(&frozen.atom_refs)
    .fetch_all(&mut *transaction)
    .await?
    .into_iter()
    .map(|row| (row.get("atom_ref"), row.get("definition_hash")))
    .collect();
    let expected_current = frozen
        .atom_refs
        .iter()
        .copied()
        .zip(frozen.definition_hashes.iter().cloned())
        .collect::<Vec<_>>();
    let mut expected_sorted = expected_current;
    expected_sorted.sort_unstable_by_key(|(atom_ref, _)| *atom_ref);
    if current != expected_sorted {
        transaction.rollback().await?;
        return Ok(false);
    }
    let ordinals = (0..frozen.atom_refs.len() as i32).collect::<Vec<_>>();
    sqlx::query(
        "INSERT INTO linggan_ci_cluster_input(run_ref,ordinal,atom_ref,definition_hash) \
         SELECT $1,ordinal,atom_ref,definition_hash \
         FROM unnest($2::integer[],$3::uuid[],$4::text[]) AS input(ordinal,atom_ref,definition_hash)",
    )
    .bind(frozen.run_ref)
    .bind(&ordinals)
    .bind(&frozen.atom_refs)
    .bind(&frozen.definition_hashes)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

pub(crate) fn discard(frozen: &FrozenSnapshot) {
    let _ = fs::remove_dir_all(&frozen.work_dir);
}

pub(crate) fn comparison_request(
    frozen: &FrozenSnapshot,
) -> Result<SemanticComputeRequest, ModelError> {
    let work_dir = std::env::temp_dir().join(format!("ci-auto-semantics-{}", Uuid::new_v4()));
    fs::create_dir(&work_dir).map_err(|_| ModelError::Source)?;
    let vector_path = work_dir.join("vectors.f32");
    if fs::hard_link(&frozen.request.snapshot.vector_path, &vector_path).is_err()
        && fs::copy(&frozen.request.snapshot.vector_path, &vector_path).is_err()
    {
        let _ = fs::remove_dir_all(&work_dir);
        return Err(ModelError::Source);
    }
    let mut snapshot = frozen.request.snapshot.clone();
    snapshot.run_ref = Uuid::new_v4();
    snapshot.vector_path = vector_path;
    snapshot.mode = SemanticComputeMode::Compare;
    Ok(SemanticComputeRequest { work_dir, snapshot })
}

pub(crate) fn discard_request(request: &SemanticComputeRequest) {
    let _ = fs::remove_dir_all(&request.work_dir);
}

async fn next_candidates(db: &Database) -> Result<Vec<Candidate>, ModelError> {
    let Some(active) = crate::embedding_settings::active_config(db).await? else {
        return Ok(Vec::new());
    };
    let api = active["api"].as_str().ok_or(ModelError::Invalid)?;
    let endpoint = active["baseUrl"].as_str().ok_or(ModelError::Invalid)?;
    let model_id = active["modelId"].as_str().ok_or(ModelError::Invalid)?;
    let dimensions = active["dimensions"]
        .as_i64()
        .filter(|value| (2..=8192).contains(value))
        .ok_or(ModelError::Invalid)? as i32;
    let rows = sqlx::query(
        "SELECT a.domain_ref,a.kind,v.space_ref,s.model_id,s.model_version,s.dimensions,s.normalization,s.space_hash, \
                min(a.created_at) AS oldest_atom \
         FROM linggan_ci_semantic_atom_current a \
         JOIN linggan_ci_atom_vector v ON v.definition_hash=a.definition_hash \
         JOIN linggan_ci_semantic_space s ON s.space_ref=v.space_ref AND s.dimensions=v.dimensions \
         WHERE s.normalization='l2' AND s.provider_api=$1 AND s.endpoint=$2 \
           AND s.model_id=$3 AND s.dimensions=$4 \
         GROUP BY a.domain_ref,a.kind,v.space_ref,s.model_id,s.model_version,s.dimensions,s.normalization,s.space_hash \
         ORDER BY oldest_atom,a.domain_ref,a.kind,s.space_hash",
    )
    .bind(api)
    .bind(endpoint)
    .bind(model_id)
    .bind(dimensions)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| Candidate {
            domain_ref: row.get("domain_ref"),
            kind: row.get("kind"),
            space_ref: row.get("space_ref"),
            model_id: row.get("model_id"),
            model_version: row.get("model_version"),
            dimensions: row.get("dimensions"),
            normalization: row.get("normalization"),
            space_hash: row.get("space_hash"),
        })
        .collect())
}

async fn completed_snapshot_exists(
    db: &Database,
    candidate: &Candidate,
    snapshot_hash: &str,
    policy_hash: &str,
) -> Result<bool, ModelError> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_ci_cluster_run \
         WHERE domain_ref=$1 AND kind=$2 AND space_ref=$3 AND snapshot_hash=$4 AND policy_hash=$5 \
           AND state IN('running','reviewing','accepted','insufficient','superseded'))",
    )
    .bind(candidate.domain_ref)
    .bind(&candidate.kind)
    .bind(candidate.space_ref)
    .bind(snapshot_hash)
    .bind(policy_hash)
    .fetch_one(db.pool())
    .await
    .map_err(ModelError::from)
}

async fn recently_failed(
    db: &Database,
    candidate: &Candidate,
    snapshot_hash: &str,
    policy_hash: &str,
) -> Result<bool, ModelError> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_ci_cluster_run \
         WHERE domain_ref=$1 AND kind=$2 AND space_ref=$3 AND snapshot_hash=$4 AND policy_hash=$5 \
           AND state='failed' AND finished_at>scope_001_now()-interval '5 minutes')",
    )
    .bind(candidate.domain_ref)
    .bind(&candidate.kind)
    .bind(candidate.space_ref)
    .bind(snapshot_hash)
    .bind(policy_hash)
    .fetch_one(db.pool())
    .await
    .map_err(ModelError::from)
}

async fn atom_vectors(db: &Database, candidate: &Candidate) -> Result<Vec<AtomVector>, ModelError> {
    let rows = sqlx::query(
        "SELECT a.atom_ref,a.definition_hash,v.vector_bytes \
         FROM linggan_ci_semantic_atom_current a \
         JOIN linggan_ci_atom_vector v ON v.definition_hash=a.definition_hash AND v.space_ref=$3 \
         WHERE a.domain_ref=$1 AND a.kind=$2 ORDER BY a.atom_ref",
    )
    .bind(candidate.domain_ref)
    .bind(&candidate.kind)
    .bind(candidate.space_ref)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| AtomVector {
            atom_ref: row.get("atom_ref"),
            definition_hash: row.get("definition_hash"),
            bytes: row.get("vector_bytes"),
        })
        .collect())
}

fn write_vectors(path: &PathBuf, atoms: &[AtomVector]) -> Result<(), ModelError> {
    let mut file = fs::File::create(path).map_err(|_| ModelError::Source)?;
    for atom in atoms {
        file.write_all(&atom.bytes)
            .map_err(|_| ModelError::Source)?;
    }
    file.sync_all().map_err(|_| ModelError::Source)
}

fn run_hash(candidate: &Candidate, atoms: &[AtomVector]) -> String {
    // This is deliberately the complete ordered atom identity frozen by P4:
    // no vector bytes, source text, likes, or credentials change this run key.
    comment_source_hash(
        &serde_json::json!({
            "kind":candidate.kind,
            "spaceHash":candidate.space_hash,
            "atoms":atoms.iter().map(|atom|serde_json::json!({
                "atomRef":atom.atom_ref,
                "definitionHash":atom.definition_hash
            })).collect::<Vec<_>>()
        })
        .to_string(),
    )
}

fn atom_kind(value: &str) -> Result<SemanticAtomKind, ModelError> {
    match value {
        "problem" => Ok(SemanticAtomKind::Problem),
        "need" => Ok(SemanticAtomKind::Need),
        "solution" => Ok(SemanticAtomKind::Solution),
        "stance" => Ok(SemanticAtomKind::Stance),
        "story" => Ok(SemanticAtomKind::Story),
        "quote" => Ok(SemanticAtomKind::Quote),
        "emotion" => Ok(SemanticAtomKind::Emotion),
        _ => Err(ModelError::Invalid),
    }
}

fn sha256_file(path: &PathBuf) -> Result<[u8; 32], ModelError> {
    let mut file = fs::File::open(path).map_err(|_| ModelError::Source)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| ModelError::Source)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_hash(value: &str) -> Result<[u8; 32], ModelError> {
    if !valid_hash(value) {
        return Err(ModelError::Invalid);
    }
    let mut hash = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        hash[index] = u8::from_str_radix(
            std::str::from_utf8(pair).map_err(|_| ModelError::Invalid)?,
            16,
        )
        .map_err(|_| ModelError::Invalid)?;
    }
    Ok(hash)
}

fn hex_hash(hash: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
