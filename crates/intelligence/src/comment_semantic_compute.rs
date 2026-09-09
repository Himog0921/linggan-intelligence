//! Controlled bridge to the local-only CI-AUTO semantic compute package.
//!
//! This module owns process hygiene and artifact identity validation only. It
//! has no database dependency and cannot create a Topic or cluster membership.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

const PROTOCOL_VERSION: &str = "ci-auto-semantics.v1";
const POLICY_VERSION: &str = "ci-auto-cluster.policy.v1";
const WORK_DIR_PREFIX: &str = "ci-auto-semantics-";
const RESULT_FILE: &str = "result.json";
const MANIFEST_FILE: &str = "manifest.json";
const MAX_STDOUT_BYTES: usize = 64 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_RESULT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_DIMENSION: usize = 8192;
const THREADS: &str = "4";
const TIMEOUT: Duration = Duration::from_secs(1800);
const LOCK_MARKER: &str = ".linggan-lock-sha256";
const VENV_CONFIG: &str = "pyvenv.cfg";
const PYTHON_VERSION: &str = ".python-version";

#[derive(Debug, thiserror::Error)]
pub enum SemanticComputeError {
    #[error("semantic_compute_input_invalid")]
    Input,
    #[error("semantic_compute_runtime_unavailable")]
    Runtime,
    #[error("semantic_compute_timeout")]
    Timeout,
    #[error("semantic_compute_stdout_limit")]
    StdoutLimit,
    #[error("semantic_compute_output_invalid")]
    Output,
    #[error("semantic_compute_io")]
    Io(#[source] std::io::Error),
    #[error("semantic_compute_json")]
    Json(#[source] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SemanticAtomKind {
    Problem,
    Need,
    Solution,
    Stance,
    Story,
    Quote,
    Emotion,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SemanticNormalization {
    #[serde(rename = "l2")]
    L2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SemanticComputeMode {
    Compare,
    Hdbscan,
    Leiden,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticVectorSpace {
    pub model: String,
    pub version: String,
    pub dimension: usize,
    pub normalization: SemanticNormalization,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSnapshot {
    pub run_ref: Uuid,
    pub kind: SemanticAtomKind,
    pub space: SemanticVectorSpace,
    /// DB-owned space identity; Python sees only its public model/version/dimension/L2 tuple.
    pub space_hash: [u8; 32],
    pub atom_ids: Vec<Uuid>,
    pub vector_path: PathBuf,
    pub snapshot_sha256: [u8; 32],
    pub mode: SemanticComputeMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticComputeRequest {
    pub work_dir: PathBuf,
    pub snapshot: SemanticSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticComputeReceipt {
    pub run_ref: Uuid,
    pub space_hash: [u8; 32],
    pub snapshot_sha256: [u8; 32],
    pub result_path: PathBuf,
    pub result_sha256: [u8; 32],
    pub bytes: u64,
    pub manifest_path: PathBuf,
    pub manifest_sha256: [u8; 32],
}

/// Fully validated algorithm output. The later organization layer remains
/// responsible for review, lineage, and any durable domain decision.
#[derive(Clone, Debug)]
pub struct SemanticComputeResult {
    pub receipt: SemanticComputeReceipt,
    pub assignments: BTreeMap<String, Vec<SemanticAssignment>>,
    pub neighbors: Vec<SemanticNeighborRecord>,
    pub quality: Value,
    pub artifact: SemanticArtifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticArtifact {
    pub policy_version: String,
    pub resource: Value,
    pub implementation: Value,
    pub checkpoint_recovery: bool,
    pub result_has_topic_definition: bool,
    pub second_embedding_stability: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAssignment {
    pub atom_id: Uuid,
    pub cluster: Option<i64>,
    pub noise: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticNeighborRecord {
    pub atom_id: Uuid,
    pub neighbors: Vec<SemanticNeighbor>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticNeighbor {
    pub atom_id: Uuid,
    pub cosine: f64,
}

#[derive(Clone, Debug)]
pub struct ControlledSemanticCompute {
    python: PathBuf,
    script: PathBuf,
    policy: PathBuf,
}

impl ControlledSemanticCompute {
    /// Uses only repository-controlled paths. There is intentionally no env
    /// override: deployment must prepare this exact venv before invoking Rust.
    pub fn configured() -> Result<Self, SemanticComputeError> {
        let (repository, app) = controlled_app()?;
        let python = app.join(".venv/bin/python");
        let script = app.join("comment_semantics.py");
        let policy = app.join("cluster-policy.json");
        controlled_repo_file(&repository, &script)?;
        controlled_repo_file(&repository, &policy)?;
        validate_venv(&repository, &app, &python)?;
        Ok(Self {
            python,
            script,
            policy,
        })
    }

    pub fn policy_sha256(&self) -> Result<[u8; 32], SemanticComputeError> {
        hash_file(&self.policy)
    }

    /// Reads the policy from the repository-controlled app source without
    /// requiring its venv. This permits a durable `semantic_runtime_unavailable`
    /// receipt when vectors are ready but deployment preparation is missing.
    pub fn controlled_policy_sha256() -> Result<[u8; 32], SemanticComputeError> {
        let (repository, app) = controlled_app()?;
        let policy = app.join("cluster-policy.json");
        controlled_repo_file(&repository, &policy)?;
        hash_file(&policy)
    }

    pub async fn compute(
        &self,
        request: &SemanticComputeRequest,
    ) -> Result<SemanticComputeResult, SemanticComputeError> {
        let validated = ValidatedRequest::from_request(request)?;
        let policy_sha256 = hash_file(&self.policy)?;
        let input = serde_json::to_vec(&PythonRequest::from_validated(&validated, policy_sha256)?)
            .map_err(SemanticComputeError::Json)?;

        let mut child = tokio::process::Command::new(&self.python)
            .arg(&self.script)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LANG", "C")
            .env("OMP_NUM_THREADS", THREADS)
            .env("OPENBLAS_NUM_THREADS", THREADS)
            .env("MKL_NUM_THREADS", THREADS)
            .env("VECLIB_MAXIMUM_THREADS", THREADS)
            .env("NUMEXPR_NUM_THREADS", THREADS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| SemanticComputeError::Runtime)?;
        let mut stdin = child.stdin.take().ok_or(SemanticComputeError::Runtime)?;
        let stdout = child.stdout.take().ok_or(SemanticComputeError::Runtime)?;
        let result = tokio::time::timeout(TIMEOUT, async {
            stdin
                .write_all(&input)
                .await
                .map_err(|_| SemanticComputeError::Runtime)?;
            stdin
                .shutdown()
                .await
                .map_err(|_| SemanticComputeError::Runtime)?;
            drop(stdin);
            let mut stdout_bytes = Vec::new();
            stdout
                .take((MAX_STDOUT_BYTES + 1) as u64)
                .read_to_end(&mut stdout_bytes)
                .await
                .map_err(|_| SemanticComputeError::Runtime)?;
            if stdout_bytes.len() > MAX_STDOUT_BYTES {
                return Err(SemanticComputeError::StdoutLimit);
            }
            let status = child
                .wait()
                .await
                .map_err(|_| SemanticComputeError::Runtime)?;
            if !status.success() {
                return Err(SemanticComputeError::Runtime);
            }
            serde_json::from_slice::<PythonSummary>(&stdout_bytes)
                .map_err(SemanticComputeError::Json)
        })
        .await;
        let summary = match result {
            Ok(value) => value?,
            Err(_) => {
                let _ = child.kill().await;
                return Err(SemanticComputeError::Timeout);
            }
        };
        validate_summary(&summary, &validated)?;
        let manifest_path = validated.work_dir.join(MANIFEST_FILE);
        let manifest_sha256 = hash_file(&manifest_path)?;
        let manifest = read_manifest(&manifest_path)?;
        validate_manifest(&manifest, &summary, &validated, policy_sha256)?;
        let result_path = validated.work_dir.join(RESULT_FILE);
        let result_metadata = fs::metadata(&result_path).map_err(SemanticComputeError::Io)?;
        if result_metadata.len() != summary.bytes || result_metadata.len() > MAX_RESULT_BYTES {
            return Err(SemanticComputeError::Output);
        }
        let actual_result_hash = hash_file(&result_path)?;
        if actual_result_hash != parse_hash(&summary.result_hash)? {
            return Err(SemanticComputeError::Output);
        }
        let receipt = SemanticComputeReceipt {
            run_ref: validated.snapshot.run_ref,
            space_hash: validated.snapshot.space_hash,
            snapshot_sha256: validated.snapshot.snapshot_sha256,
            result_path,
            result_sha256: actual_result_hash,
            bytes: summary.bytes,
            manifest_path,
            manifest_sha256,
        };
        let output = read_result(&receipt.result_path)?;
        validate_result(&output, &manifest, &validated, policy_sha256)?;
        Ok(SemanticComputeResult {
            receipt,
            assignments: output.assignments,
            neighbors: output.neighbors,
            quality: output.quality,
            artifact: output.artifact,
        })
    }
}

fn controlled_app() -> Result<(PathBuf, PathBuf), SemanticComputeError> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(SemanticComputeError::Io)?;
    let app = repository.join("apps/comment-semantics");
    let canonical_app = app.canonicalize().map_err(SemanticComputeError::Io)?;
    ensure_child(&repository, &canonical_app).map_err(|_| SemanticComputeError::Runtime)?;
    Ok((repository, app))
}

fn controlled_repo_file(repository: &Path, path: &Path) -> Result<(), SemanticComputeError> {
    let canonical = path.canonicalize().map_err(SemanticComputeError::Io)?;
    ensure_child(repository, &canonical).map_err(|_| SemanticComputeError::Runtime)
}

fn validate_venv(repository: &Path, app: &Path, python: &Path) -> Result<(), SemanticComputeError> {
    let venv = app.join(".venv");
    let canonical_venv = venv.canonicalize().map_err(SemanticComputeError::Io)?;
    ensure_child(repository, &canonical_venv).map_err(|_| SemanticComputeError::Runtime)?;
    if !python.starts_with(&venv) || !python.is_file() {
        return Err(SemanticComputeError::Runtime);
    }
    let config = venv.join(VENV_CONFIG);
    let lock_marker = venv.join(LOCK_MARKER);
    controlled_venv_file(&canonical_venv, &config)?;
    controlled_venv_file(&canonical_venv, &lock_marker)?;
    let config = fs::read_to_string(config).map_err(SemanticComputeError::Io)?;
    let expected_version =
        fs::read_to_string(app.join(PYTHON_VERSION)).map_err(SemanticComputeError::Io)?;
    let (home, version) = parse_venv_config(&config).ok_or(SemanticComputeError::Runtime)?;
    if version != expected_version.trim() {
        return Err(SemanticComputeError::Runtime);
    }
    let interpreter = python.canonicalize().map_err(SemanticComputeError::Io)?;
    let canonical_home = PathBuf::from(home)
        .canonicalize()
        .map_err(SemanticComputeError::Io)?;
    // Homebrew's venv launcher usually resolves from its `bin` shim into a
    // Framework path under the same Cellar version. Requiring the canonical
    // binary's immediate parent to equal `home` rejects that normal layout.
    // Keep the external interpreter fenced to the one distribution declared
    // by the controlled venv, while the venv path, version and lock marker
    // continue to protect the repository-owned runtime itself.
    let base_distribution = canonical_home
        .parent()
        .ok_or(SemanticComputeError::Runtime)?;
    if !interpreter.is_file() || !interpreter.starts_with(base_distribution) {
        return Err(SemanticComputeError::Runtime);
    }
    let expected_lock = hash_file(&app.join("requirements.lock"))?;
    let lock = fs::read_to_string(lock_marker).map_err(SemanticComputeError::Io)?;
    if lock.trim() != encode_hash(expected_lock) {
        return Err(SemanticComputeError::Runtime);
    }
    Ok(())
}

fn controlled_venv_file(venv: &Path, path: &Path) -> Result<(), SemanticComputeError> {
    let canonical = path.canonicalize().map_err(SemanticComputeError::Io)?;
    ensure_child(venv, &canonical).map_err(|_| SemanticComputeError::Runtime)
}

fn parse_venv_config(config: &str) -> Option<(&str, &str)> {
    let home = config
        .lines()
        .find_map(|line| line.strip_prefix("home = "))?;
    let version = config
        .lines()
        .find_map(|line| line.strip_prefix("version_info = "))?;
    (!home.is_empty() && !version.is_empty()).then_some((home, version))
}

#[derive(Debug)]
struct ValidatedRequest {
    work_dir: PathBuf,
    vector_path: PathBuf,
    snapshot: SemanticSnapshot,
    source_atom_ids_hash: [u8; 32],
}

impl ValidatedRequest {
    fn from_request(request: &SemanticComputeRequest) -> Result<Self, SemanticComputeError> {
        let work_dir = request
            .work_dir
            .canonicalize()
            .map_err(SemanticComputeError::Io)?;
        if !work_dir.is_dir()
            || !work_dir
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(WORK_DIR_PREFIX))
        {
            return Err(SemanticComputeError::Input);
        }
        let vector_path = request
            .snapshot
            .vector_path
            .canonicalize()
            .map_err(SemanticComputeError::Io)?;
        ensure_child(&work_dir, &vector_path)?;
        let metadata = fs::metadata(&vector_path).map_err(SemanticComputeError::Io)?;
        if !metadata.is_file()
            || request.snapshot.atom_ids.is_empty()
            || request.snapshot.space.dimension == 0
            || request.snapshot.space.dimension > MAX_DIMENSION
            || request.snapshot.space.model.trim().is_empty()
            || request.snapshot.space.version.trim().is_empty()
        {
            return Err(SemanticComputeError::Input);
        }
        let expected_bytes = request
            .snapshot
            .atom_ids
            .len()
            .checked_mul(request.snapshot.space.dimension)
            .and_then(|count| count.checked_mul(4))
            .ok_or(SemanticComputeError::Input)?;
        if metadata.len() != expected_bytes as u64
            || hash_file(&vector_path)? != request.snapshot.snapshot_sha256
        {
            return Err(SemanticComputeError::Input);
        }
        let mut sorted_ids = request.snapshot.atom_ids.clone();
        sorted_ids.sort_unstable();
        if sorted_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SemanticComputeError::Input);
        }
        Ok(Self {
            work_dir,
            vector_path,
            source_atom_ids_hash: hash_atom_ids(&request.snapshot.atom_ids),
            snapshot: request.snapshot.clone(),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PythonRequest<'a> {
    protocol_version: &'static str,
    work_dir: &'a Path,
    snapshot: PythonSnapshot<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PythonSnapshot<'a> {
    run_ref: Uuid,
    kind: SemanticAtomKind,
    space: &'a SemanticVectorSpace,
    atom_ids: &'a [Uuid],
    vector_path: String,
    snapshot_hash: String,
    policy: PythonPolicy,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PythonPolicy {
    version: &'static str,
    sha256: String,
    mode: SemanticComputeMode,
}

impl<'a> PythonRequest<'a> {
    fn from_validated(
        request: &'a ValidatedRequest,
        policy_sha256: [u8; 32],
    ) -> Result<Self, SemanticComputeError> {
        Ok(Self {
            protocol_version: PROTOCOL_VERSION,
            work_dir: &request.work_dir,
            snapshot: PythonSnapshot {
                run_ref: request.snapshot.run_ref,
                kind: request.snapshot.kind,
                space: &request.snapshot.space,
                atom_ids: &request.snapshot.atom_ids,
                vector_path: request
                    .vector_path
                    .strip_prefix(&request.work_dir)
                    .map_err(|_| SemanticComputeError::Input)?
                    .to_str()
                    .ok_or(SemanticComputeError::Input)?
                    .to_owned(),
                snapshot_hash: encode_hash(request.snapshot.snapshot_sha256),
                policy: PythonPolicy {
                    version: POLICY_VERSION,
                    sha256: encode_hash(policy_sha256),
                    mode: request.snapshot.mode,
                },
            },
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PythonSummary {
    protocol_version: String,
    run_ref: Uuid,
    snapshot_hash: String,
    result_path: String,
    result_hash: String,
    bytes: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    protocol_version: String,
    run_ref: Uuid,
    snapshot_hash: String,
    policy: ManifestPolicy,
    input: ManifestInput,
    assignments: BTreeMap<String, ManifestAssignment>,
    neighbors: ManifestNeighbors,
    result: ManifestResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestPolicy {
    version: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestInput {
    kind: SemanticAtomKind,
    space: SemanticVectorSpace,
    source_atom_count: usize,
    source_atom_ids_hash: String,
    selected_atom_count: usize,
    selected_atom_ids_hash: String,
    selected_ordinal_hash: String,
    mode: SemanticComputeMode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestAssignment {
    count: usize,
    atom_ids_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestNeighbors {
    count: usize,
    atom_ids_hash: String,
    neighbors_per_atom: usize,
    self_edges: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestResult {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PythonResult {
    protocol_version: String,
    run_ref: Uuid,
    snapshot_hash: String,
    policy: ManifestPolicy,
    input: ResultInput,
    assignments: BTreeMap<String, Vec<SemanticAssignment>>,
    neighbors: Vec<SemanticNeighborRecord>,
    quality: Value,
    artifact: ResultArtifact,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResultInput {
    kind: SemanticAtomKind,
    space: SemanticVectorSpace,
    source_atom_count: usize,
    source_atom_ids_hash: String,
    selected_atom_count: usize,
    selected_ordinal_hash: String,
    atom_ids_hash: String,
    vector_sha256: String,
    mode: SemanticComputeMode,
}

type ResultArtifact = SemanticArtifact;

fn validate_summary(
    summary: &PythonSummary,
    request: &ValidatedRequest,
) -> Result<(), SemanticComputeError> {
    if summary.protocol_version != PROTOCOL_VERSION
        || summary.run_ref != request.snapshot.run_ref
        || parse_hash(&summary.snapshot_hash)? != request.snapshot.snapshot_sha256
        || summary.result_path != RESULT_FILE
        || summary.bytes == 0
        || summary.bytes > MAX_RESULT_BYTES
        || parse_hash(&summary.result_hash).is_err()
    {
        return Err(SemanticComputeError::Output);
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Manifest, SemanticComputeError> {
    let metadata = fs::metadata(path).map_err(SemanticComputeError::Io)?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(SemanticComputeError::Output);
    }
    let bytes = fs::read(path).map_err(SemanticComputeError::Io)?;
    serde_json::from_slice(&bytes).map_err(SemanticComputeError::Json)
}

fn validate_manifest(
    manifest: &Manifest,
    summary: &PythonSummary,
    request: &ValidatedRequest,
    policy_sha256: [u8; 32],
) -> Result<(), SemanticComputeError> {
    let source_hash = encode_hash(request.source_atom_ids_hash);
    let source_count = request.snapshot.atom_ids.len();
    let expected_routes: &[&str] = match request.snapshot.mode {
        SemanticComputeMode::Compare => &["hdbscan", "leiden"],
        SemanticComputeMode::Hdbscan => &["hdbscan"],
        SemanticComputeMode::Leiden => &["leiden"],
    };
    let selected_is_full = !matches!(request.snapshot.mode, SemanticComputeMode::Compare);
    if manifest.protocol_version != PROTOCOL_VERSION
        || manifest.run_ref != request.snapshot.run_ref
        || parse_hash(&manifest.snapshot_hash)? != request.snapshot.snapshot_sha256
        || manifest.policy.version != POLICY_VERSION
        || parse_hash(&manifest.policy.sha256)? != policy_sha256
        || manifest.input.kind != request.snapshot.kind
        || manifest.input.space != request.snapshot.space
        || manifest.input.source_atom_count != source_count
        || manifest.input.source_atom_ids_hash != source_hash
        || manifest.input.mode != request.snapshot.mode
        || manifest.input.selected_atom_count == 0
        || manifest.input.selected_atom_count > source_count
        || parse_hash(&manifest.input.selected_atom_ids_hash).is_err()
        || parse_hash(&manifest.input.selected_ordinal_hash).is_err()
        || manifest.neighbors.count != manifest.input.selected_atom_count
        || manifest.neighbors.atom_ids_hash != manifest.input.selected_atom_ids_hash
        || manifest.neighbors.self_edges != 0
        || manifest.neighbors.neighbors_per_atom != 20.min(source_count.saturating_sub(1))
        || manifest.result.path != RESULT_FILE
        || manifest.result.sha256 != summary.result_hash
        || manifest.result.bytes != summary.bytes
        || manifest.assignments.len() != expected_routes.len()
        || expected_routes.iter().any(|route| {
            manifest.assignments.get(*route).is_none_or(|assignment| {
                assignment.count != manifest.input.selected_atom_count
                    || assignment.atom_ids_hash != manifest.input.selected_atom_ids_hash
            })
        })
        || (selected_is_full
            && (manifest.input.selected_atom_count != source_count
                || manifest.input.selected_atom_ids_hash != source_hash))
    {
        return Err(SemanticComputeError::Output);
    }
    Ok(())
}

fn read_result(path: &Path) -> Result<PythonResult, SemanticComputeError> {
    let bytes = fs::read(path).map_err(SemanticComputeError::Io)?;
    serde_json::from_slice(&bytes).map_err(SemanticComputeError::Json)
}

fn validate_result(
    output: &PythonResult,
    manifest: &Manifest,
    request: &ValidatedRequest,
    policy_sha256: [u8; 32],
) -> Result<(), SemanticComputeError> {
    let expected_routes: &[&str] = match request.snapshot.mode {
        SemanticComputeMode::Compare => &["hdbscan", "leiden"],
        SemanticComputeMode::Hdbscan => &["hdbscan"],
        SemanticComputeMode::Leiden => &["leiden"],
    };
    validate_result_metadata(output, manifest, request, policy_sha256, expected_routes)?;
    let selected_ids = validate_assignments(output, manifest, request, expected_routes)?;
    validate_neighbors(output, &selected_ids, request.snapshot.atom_ids.len())
}

fn validate_result_metadata(
    output: &PythonResult,
    manifest: &Manifest,
    request: &ValidatedRequest,
    policy_sha256: [u8; 32],
    expected_routes: &[&str],
) -> Result<(), SemanticComputeError> {
    let source_count = request.snapshot.atom_ids.len();
    let source_hash = encode_hash(request.source_atom_ids_hash);
    if output.protocol_version != PROTOCOL_VERSION
        || output.run_ref != request.snapshot.run_ref
        || parse_hash(&output.snapshot_hash)? != request.snapshot.snapshot_sha256
        || output.policy.version != POLICY_VERSION
        || parse_hash(&output.policy.sha256)? != policy_sha256
        || output.input.kind != request.snapshot.kind
        || output.input.space != request.snapshot.space
        || output.input.source_atom_count != source_count
        || output.input.source_atom_ids_hash != source_hash
        || output.input.selected_atom_count != manifest.input.selected_atom_count
        || output.input.selected_ordinal_hash != manifest.input.selected_ordinal_hash
        || output.input.atom_ids_hash != manifest.input.selected_atom_ids_hash
        || parse_hash(&output.input.vector_sha256)? != request.snapshot.snapshot_sha256
        || output.input.mode != request.snapshot.mode
        || output.assignments.len() != expected_routes.len()
        || output.artifact.policy_version != POLICY_VERSION
        || output.artifact.result_has_topic_definition
        || !output.artifact.resource.is_object()
        || !output.artifact.implementation.is_object()
        || !output.artifact.second_embedding_stability.is_object()
        || !output.quality.is_object()
    {
        return Err(SemanticComputeError::Output);
    }
    Ok(())
}

fn validate_assignments(
    output: &PythonResult,
    manifest: &Manifest,
    request: &ValidatedRequest,
    expected_routes: &[&str],
) -> Result<Vec<Uuid>, SemanticComputeError> {
    let first_route = expected_routes
        .first()
        .ok_or(SemanticComputeError::Output)?;
    let selected = output
        .assignments
        .get(*first_route)
        .ok_or(SemanticComputeError::Output)?;
    if selected.len() != manifest.input.selected_atom_count
        || hash_atom_ids(
            &selected
                .iter()
                .map(|assignment| assignment.atom_id)
                .collect::<Vec<_>>(),
        ) != parse_hash(&manifest.input.selected_atom_ids_hash)?
    {
        return Err(SemanticComputeError::Output);
    }
    let allowed: std::collections::BTreeSet<_> =
        request.snapshot.atom_ids.iter().copied().collect();
    let selected_ids: Vec<_> = selected
        .iter()
        .map(|assignment| assignment.atom_id)
        .collect();
    let selected_set: std::collections::BTreeSet<_> = selected_ids.iter().copied().collect();
    if selected_set.len() != selected_ids.len() || !selected_set.is_subset(&allowed) {
        return Err(SemanticComputeError::Output);
    }
    if !matches!(request.snapshot.mode, SemanticComputeMode::Compare)
        && selected_ids != request.snapshot.atom_ids
    {
        return Err(SemanticComputeError::Output);
    }
    for route in expected_routes {
        let assignments = output
            .assignments
            .get(*route)
            .ok_or(SemanticComputeError::Output)?;
        if assignments.len() != selected.len()
            || assignments
                .iter()
                .zip(selected)
                .any(|(actual, expected)| actual.atom_id != expected.atom_id)
            || assignments
                .iter()
                .any(|assignment| assignment.noise != assignment.cluster.is_none())
        {
            return Err(SemanticComputeError::Output);
        }
    }
    Ok(selected_ids)
}

fn validate_neighbors(
    output: &PythonResult,
    selected_ids: &[Uuid],
    source_count: usize,
) -> Result<(), SemanticComputeError> {
    let expected_k = 20.min(source_count.saturating_sub(1));
    let selected_set = selected_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if output.neighbors.len() != selected_ids.len()
        || output
            .neighbors
            .iter()
            .zip(selected_ids)
            .any(|(actual, expected)| actual.atom_id != *expected)
    {
        return Err(SemanticComputeError::Output);
    }
    for record in &output.neighbors {
        if record.neighbors.len() != expected_k {
            return Err(SemanticComputeError::Output);
        }
        let mut neighbor_ids = std::collections::BTreeSet::new();
        for neighbor in &record.neighbors {
            if neighbor.atom_id == record.atom_id
                || !selected_set.contains(&neighbor.atom_id)
                || !neighbor_ids.insert(neighbor.atom_id)
                || !neighbor.cosine.is_finite()
                || !(-1.0001..=1.0001).contains(&neighbor.cosine)
            {
                return Err(SemanticComputeError::Output);
            }
        }
    }
    Ok(())
}

fn ensure_child(parent: &Path, child: &Path) -> Result<(), SemanticComputeError> {
    if child.strip_prefix(parent).is_err() {
        return Err(SemanticComputeError::Input);
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<[u8; 32], SemanticComputeError> {
    let mut file = File::open(path).map_err(SemanticComputeError::Io)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(SemanticComputeError::Io)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

fn hash_atom_ids(atom_ids: &[Uuid]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for atom_id in atom_ids {
        let encoded = atom_id.to_string();
        digest.update((encoded.len() as u16).to_be_bytes());
        digest.update(encoded.as_bytes());
    }
    digest.finalize().into()
}

fn encode_hash(hash: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn parse_hash(value: &str) -> Result<[u8; 32], SemanticComputeError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(SemanticComputeError::Output);
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair).map_err(|_| SemanticComputeError::Output)?;
        output[index] = u8::from_str_radix(pair, 16).map_err(|_| SemanticComputeError::Output)?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{encode_hash, hash_atom_ids, parse_hash};
    use uuid::Uuid;

    #[test]
    fn atom_id_hash_matches_python_length_prefixed_contract() {
        let ids = [
            Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap(),
            Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
        ];
        assert_eq!(
            encode_hash(hash_atom_ids(&ids)),
            "f5ffec8c2c69cb59cb83b0ed95c1d38f90715604b27012525be7919563cb9f91"
        );
    }

    #[test]
    fn hashes_reject_uppercase_and_wrong_length() {
        assert!(parse_hash("A".repeat(64).as_str()).is_err());
        assert!(parse_hash("0".repeat(63).as_str()).is_err());
    }
}
