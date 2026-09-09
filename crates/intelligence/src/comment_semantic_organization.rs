//! Versioned local organization of current semantic atoms.
//!
//! Atom qualification, embedding cache and vector-space identity are owned by
//! sibling modules. This module freezes one domain/kind/space input, invokes
//! the controlled local computer, and persists candidates for bounded review.

#[path = "comment_semantic_organization_decision.rs"]
mod decision;
#[path = "comment_semantic_organization_persistence.rs"]
mod persistence;
#[path = "comment_semantic_organization_reconciliation.rs"]
mod reconciliation;
#[path = "comment_semantic_organization_review.rs"]
mod review;
#[path = "comment_semantic_organization_snapshot.rs"]
mod snapshot;

use crate::{
    comment_semantic_compute::{
        ControlledSemanticCompute, SemanticComputeResult, SemanticNeighbor,
    },
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const REVIEW_MIN_CLUSTER_MEMBERS: usize = 10;
const REVIEW_CORE_MAX: usize = 5;
const REVIEW_BOUNDARY_MAX: usize = 4;
const REVIEW_NEIGHBOR_MAX: usize = 3;

pub async fn run_once(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&ModelWorkerDrain>,
) -> Result<bool, ModelError> {
    if draining(drain) {
        return Ok(false);
    }
    if !schema_ready(db).await? {
        return Err(ModelError::SchemaMissing);
    }
    retire_withdrawn_memberships(db).await?;
    recover_expired_runs(db).await?;
    if crate::embedding_settings::active_config(db)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    if review::run_once(db, store, adapter, drain).await? {
        return Ok(true);
    }
    if draining(drain) {
        return Ok(false);
    }
    if !snapshot::has_current_vector_candidate(db).await? {
        return Ok(false);
    }
    let policy_hash =
        ControlledSemanticCompute::controlled_policy_sha256().map_err(|_| ModelError::Source)?;
    let Some(frozen) = snapshot::freeze_next(db, policy_hash).await? else {
        return Ok(false);
    };
    if draining(drain) {
        snapshot::discard(&frozen);
        return Ok(false);
    }
    if !snapshot::reserve(db, &frozen).await? {
        snapshot::discard(&frozen);
        return Ok(false);
    }
    if draining(drain) {
        mark_compute_failed(db, &frozen, "worker_draining".into()).await?;
        return Ok(false);
    }
    let computer = match ControlledSemanticCompute::configured() {
        Ok(computer) => computer,
        Err(_) => {
            mark_compute_failed(db, &frozen, "semantic_runtime_unavailable".into()).await?;
            snapshot::discard(&frozen);
            return Ok(true);
        }
    };
    // A whole 100k result must not be selected merely because its graph path
    // exists. Compare the exact same frozen input under both algorithms first;
    // Python bounds that compare to its fixed 10k stratified sample.
    let comparison = if matches!(
        frozen.request.snapshot.mode,
        crate::comment_semantic_compute::SemanticComputeMode::Leiden
    ) {
        let comparison_request = snapshot::comparison_request(&frozen)?;
        let compared = computer.compute(&comparison_request).await;
        snapshot::discard_request(&comparison_request);
        match compared {
            Ok(result) => {
                if !hard_gates_pass(&result.quality) {
                    mark_compare_insufficient(db, &frozen, &result).await?;
                    snapshot::discard(&frozen);
                    return Ok(true);
                }
                Some(result)
            }
            Err(error) => {
                mark_compute_failed(db, &frozen, error.to_string()).await?;
                return Ok(true);
            }
        }
    } else {
        None
    };
    let computed = computer.compute(&frozen.request).await;
    let mut result = match computed {
        Ok(result) => result,
        Err(error) => {
            mark_compute_failed(db, &frozen, error.to_string()).await?;
            // Keep a resource/interruption checkpoint for local forensic recovery. No
            // database credential or source text is written into this work directory.
            return Ok(true);
        }
    };
    if draining(drain) {
        mark_compute_failed(db, &frozen, "worker_draining".into()).await?;
        return Ok(false);
    }
    let comparison_quality = comparison.as_ref().map(|result| &result.quality);
    if let Some(quality) = comparison_quality {
        result.quality["comparison"] = quality.clone();
        result.quality["productionSelection"] = json!({
            "algorithm":"leiden",
            "reason":"same_frozen_input_compare_passed; whole_input_leiden_is_the_only_current_capacity-proven route",
            "hdbscanProduction":"not_run_without_current_100k_capacity_proof"
        });
    }
    persist_compute_result(db, &frozen, &result, comparison_quality).await?;
    snapshot::discard(&frozen);
    Ok(true)
}

fn draining(drain: Option<&ModelWorkerDrain>) -> bool {
    drain.is_some_and(ModelWorkerDrain::is_requested)
}

async fn schema_ready(db: &Database) -> Result<bool, ModelError> {
    sqlx::query_scalar(
        "SELECT to_regclass('linggan_ci_semantic_atom_current') IS NOT NULL \
          AND to_regclass('linggan_ci_cluster_run') IS NOT NULL \
          AND to_regclass('linggan_ci_cluster_input') IS NOT NULL \
          AND to_regclass('linggan_ci_cluster_assignment') IS NOT NULL",
    )
    .fetch_one(db.pool())
    .await
    .map_err(ModelError::from)
}

async fn recover_expired_runs(db: &Database) -> Result<(), ModelError> {
    // Inputs and prior accepted groups are immutable. An expired local compute
    // attempt therefore becomes independently visible; it never deletes or
    // rewrites an older usable organization.
    sqlx::query(
        "UPDATE linggan_ci_cluster_run \
         SET state='failed', failure_code='worker_interrupted', finished_at=scope_001_now(), lease_until=NULL \
         WHERE state='running' AND lease_until<=scope_001_now()",
    )
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn retire_withdrawn_memberships(db: &Database) -> Result<(), ModelError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE linggan_ci_atom_membership membership \
         SET current=false,superseded_reason='source_withdrawn',superseded_at=scope_001_now() \
         WHERE membership.current \
           AND NOT EXISTS(SELECT 1 FROM linggan_ci_semantic_atom_current atom \
                          WHERE atom.atom_ref=membership.atom_ref)",
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "DELETE FROM linggan_ci_problem_member member \
         USING linggan_ci_semantic_group group_row \
         WHERE member.problem_ref=group_row.problem_ref AND member.origin='model_equivalence' \
           AND NOT EXISTS(SELECT 1 FROM linggan_ci_atom_membership membership \
                          JOIN linggan_ci_semantic_atom_current atom ON atom.atom_ref=membership.atom_ref \
                          WHERE membership.group_ref=group_row.group_ref \
                            AND membership.current AND membership.relation='same' \
                            AND atom.canonical_ref=member.canonical_ref)",
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE linggan_ci_semantic_group group_row \
         SET state='superseded',superseded_reason='source_withdrawn',updated_at=scope_001_now() \
         WHERE group_row.state='active' \
           AND NOT EXISTS(SELECT 1 FROM linggan_ci_atom_membership membership \
                          WHERE membership.group_ref=group_row.group_ref \
                            AND membership.current AND membership.relation='same')",
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

async fn mark_compute_failed(
    db: &Database,
    frozen: &snapshot::FrozenSnapshot,
    failure: String,
) -> Result<(), ModelError> {
    let code = failure
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
        .take(80)
        .collect::<String>();
    sqlx::query(
        "UPDATE linggan_ci_cluster_run \
         SET state='failed', failure_code=$2, finished_at=scope_001_now(), lease_until=NULL \
         WHERE run_ref=$1 AND state='running'",
    )
    .bind(frozen.run_ref)
    .bind(if code.is_empty() {
        "local_compute_failed"
    } else {
        &code
    })
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn mark_compare_insufficient(
    db: &Database,
    frozen: &snapshot::FrozenSnapshot,
    comparison: &SemanticComputeResult,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE linggan_ci_cluster_run \
         SET state='insufficient',vector_hash=$2,quality=$3,failure_code='comparison_quality_insufficient',finished_at=scope_001_now(),lease_until=NULL \
         WHERE run_ref=$1 AND state='running'",
    )
    .bind(frozen.run_ref)
    .bind(hex_hash(comparison.receipt.snapshot_sha256))
    .bind(json!({
        "policyHash":frozen.policy_hash,
        "spaceHash":frozen.space_hash,
        "runSnapshotHash":frozen.run_snapshot_hash,
        "inputManifestHash":hex_hash(comparison.receipt.manifest_sha256),
        "comparisonQuality":comparison.quality,
        "acceptedForReview":false
    }))
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn persist_compute_result(
    db: &Database,
    frozen: &snapshot::FrozenSnapshot,
    result: &SemanticComputeResult,
    comparison_quality: Option<&Value>,
) -> Result<(), ModelError> {
    let acceptable = hard_gates_pass(&result.quality)
        && comparison_quality.is_none_or(|quality| hard_gates_pass(quality));
    let mut transaction = db.pool().begin().await?;
    let running: bool = sqlx::query_scalar(
        "SELECT state='running' AND snapshot_hash=$2 AND policy_hash=$3 AND vector_hash=$4 \
         FROM linggan_ci_cluster_run WHERE run_ref=$1 FOR UPDATE",
    )
    .bind(frozen.run_ref)
    .bind(&frozen.run_snapshot_hash)
    .bind(&frozen.policy_hash)
    .bind(hex_hash(frozen.request.snapshot.snapshot_sha256))
    .fetch_optional(&mut *transaction)
    .await?
    .unwrap_or(false);
    if !running {
        return Err(ModelError::Conflict);
    }
    for (algorithm, assignments) in &result.assignments {
        if !matches!(algorithm.as_str(), "hdbscan" | "leiden") {
            return Err(ModelError::InvalidOutput);
        }
        let atom_refs: Vec<_> = assignments
            .iter()
            .map(|assignment| assignment.atom_id)
            .collect();
        let clusters: Vec<Option<String>> = assignments
            .iter()
            .map(|assignment| assignment.cluster.map(|cluster| cluster.to_string()))
            .collect();
        if atom_refs != frozen.atom_refs {
            return Err(ModelError::InvalidOutput);
        }
        sqlx::query(
            "INSERT INTO linggan_ci_cluster_assignment(run_ref,atom_ref,algorithm,cluster_key,confidence) \
             SELECT $1,atom_ref,$2,cluster_key,NULL \
             FROM unnest($3::uuid[],$4::text[]) AS input(atom_ref,cluster_key) \
             ON CONFLICT(run_ref,atom_ref,algorithm) DO NOTHING",
        )
        .bind(frozen.run_ref)
        .bind(algorithm)
        .bind(&atom_refs)
        .bind(&clusters)
        .execute(&mut *transaction)
        .await?;
    }
    let review_count = if acceptable {
        queue_bounded_reviews(&mut transaction, frozen, result).await?
    } else {
        0
    };
    // A quality-passing graph without a reviewable group is not an adopted
    // organization. Preserve every atom and assignment, then make the run
    // explicitly insufficient rather than leaving it in `reviewing` forever.
    let accepted_for_review = acceptable && review_count > 0;
    let quality = compute_receipt(
        frozen,
        result,
        comparison_quality,
        accepted_for_review,
        review_count,
    );
    let next_state = if accepted_for_review {
        "reviewing"
    } else {
        "insufficient"
    };
    let changed = sqlx::query(
        "UPDATE linggan_ci_cluster_run \
         SET state=$2, vector_hash=$3, quality=$4, failure_code=$5, finished_at=scope_001_now(), lease_until=NULL \
         WHERE run_ref=$1 AND state='running'",
    )
    .bind(frozen.run_ref)
    .bind(next_state)
    .bind(hex_hash(result.receipt.snapshot_sha256))
    .bind(quality)
    .bind(if accepted_for_review {
        None
    } else if acceptable {
        Some("no_review_eligible_clusters")
    } else {
        Some("quality_gate_failed_or_insufficient")
    })
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(ModelError::Conflict);
    }
    transaction.commit().await?;
    Ok(())
}

async fn queue_bounded_reviews(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    frozen: &snapshot::FrozenSnapshot,
    result: &SemanticComputeResult,
) -> Result<usize, ModelError> {
    let samples = review_samples(result)?;
    let mut inserted = 0;
    for sample in samples {
        inserted += sqlx::query(
            "INSERT INTO linggan_ci_cluster_review(review_ref,run_ref,cluster_key,algorithm,sample_refs,snapshot_hash,state,receipt) \
             VALUES($1,$2,$3,'leiden',$4,$5,'pending',$6) \
             ON CONFLICT(run_ref,cluster_key,algorithm) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(frozen.run_ref)
        .bind(&sample.cluster_key)
        .bind(&sample.sample_refs)
        .bind(&frozen.run_snapshot_hash)
        .bind(json!({
            "sampleRoles": sample.roles,
            "sampleSelection": "knn_density_core_boundary_external_neighbors.v1"
        }))
        .execute(&mut **transaction)
        .await?
        .rows_affected() as usize;
    }
    Ok(inserted)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewSampleRoles {
    core_atom_refs: Vec<Uuid>,
    boundary_atom_refs: Vec<Uuid>,
    neighbor_atom_refs: Vec<Uuid>,
}

struct ReviewSample {
    cluster_key: String,
    sample_refs: Vec<Uuid>,
    roles: ReviewSampleRoles,
}

/// Select exactly the frozen policy's three review roles without materializing
/// a pairwise distance matrix. Core and boundary atoms come from one Leiden
/// cluster; neighbor atoms are nearest atoms belonging to another non-noise
/// Leiden cluster and serve only as counterexamples in the semantic review.
fn review_samples(result: &SemanticComputeResult) -> Result<Vec<ReviewSample>, ModelError> {
    let labels = leiden_labels(result)?;
    let neighborhoods = neighbor_index(result, &labels)?;
    let mut clusters = BTreeMap::<i64, Vec<Uuid>>::new();
    for (atom_ref, label) in &labels {
        if let Some(cluster) = label {
            clusters.entry(*cluster).or_default().push(*atom_ref);
        }
    }
    clusters
        .into_iter()
        .filter(|(_, members)| members.len() >= REVIEW_MIN_CLUSTER_MEMBERS)
        .map(|(cluster, members)| sample_cluster(cluster, members, &labels, &neighborhoods))
        .collect()
}

fn leiden_labels(
    result: &SemanticComputeResult,
) -> Result<BTreeMap<Uuid, Option<i64>>, ModelError> {
    let assignments = result
        .assignments
        .get("leiden")
        .ok_or(ModelError::InvalidOutput)?;
    let mut labels = BTreeMap::<Uuid, Option<i64>>::new();
    for assignment in assignments {
        if assignment.noise != assignment.cluster.is_none()
            || labels
                .insert(assignment.atom_id, assignment.cluster)
                .is_some()
        {
            return Err(ModelError::InvalidOutput);
        }
    }
    Ok(labels)
}

type NeighborIndex<'a> = BTreeMap<Uuid, &'a [SemanticNeighbor]>;

fn neighbor_index<'a>(
    result: &'a SemanticComputeResult,
    labels: &BTreeMap<Uuid, Option<i64>>,
) -> Result<NeighborIndex<'a>, ModelError> {
    let mut neighborhoods = BTreeMap::new();
    for record in &result.neighbors {
        if !labels.contains_key(&record.atom_id)
            || neighborhoods
                .insert(record.atom_id, record.neighbors.as_slice())
                .is_some()
        {
            return Err(ModelError::InvalidOutput);
        }
        let mut seen = BTreeSet::new();
        if record.neighbors.iter().any(|neighbor| {
            !neighbor.cosine.is_finite()
                || !labels.contains_key(&neighbor.atom_id)
                || !seen.insert(neighbor.atom_id)
        }) {
            return Err(ModelError::InvalidOutput);
        }
    }
    if neighborhoods.len() != labels.len() {
        return Err(ModelError::InvalidOutput);
    }
    Ok(neighborhoods)
}

fn sample_cluster(
    cluster: i64,
    members: Vec<Uuid>,
    labels: &BTreeMap<Uuid, Option<i64>>,
    neighborhoods: &NeighborIndex<'_>,
) -> Result<ReviewSample, ModelError> {
    let density = cluster_density(&members, neighborhoods)?;
    let (core_atom_refs, boundary_atom_refs) = role_members(density);
    let neighbor_atom_refs = external_neighbors(cluster, &members, labels, neighborhoods)?;
    let mut sample_refs = core_atom_refs.clone();
    sample_refs.extend(boundary_atom_refs.iter().copied());
    sample_refs.extend(neighbor_atom_refs.iter().copied());
    if sample_refs.len() > REVIEW_CORE_MAX + REVIEW_BOUNDARY_MAX + REVIEW_NEIGHBOR_MAX
        || sample_refs.iter().copied().collect::<BTreeSet<_>>().len() != sample_refs.len()
    {
        return Err(ModelError::InvalidOutput);
    }
    Ok(ReviewSample {
        cluster_key: cluster.to_string(),
        sample_refs,
        roles: ReviewSampleRoles {
            core_atom_refs,
            boundary_atom_refs,
            neighbor_atom_refs,
        },
    })
}

fn cluster_density(
    members: &[Uuid],
    neighborhoods: &NeighborIndex<'_>,
) -> Result<Vec<(Uuid, f64, usize)>, ModelError> {
    let member_set = members.iter().copied().collect::<BTreeSet<_>>();
    let mut density = members
        .iter()
        .map(|atom_ref| {
            let neighbors = neighborhoods
                .get(atom_ref)
                .ok_or(ModelError::InvalidOutput)?;
            let (support, degree) = neighbors
                .iter()
                .filter(|neighbor| member_set.contains(&neighbor.atom_id))
                .fold((0.0_f64, 0_usize), |(support, degree), neighbor| {
                    (support + neighbor.cosine, degree + 1)
                });
            Ok((*atom_ref, support, degree))
        })
        .collect::<Result<Vec<_>, ModelError>>()?;
    density.sort_unstable_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    Ok(density)
}

fn role_members(density: Vec<(Uuid, f64, usize)>) -> (Vec<Uuid>, Vec<Uuid>) {
    let core_atom_refs = density
        .iter()
        .take(REVIEW_CORE_MAX)
        .map(|(atom_ref, _, _)| *atom_ref)
        .collect::<Vec<_>>();
    let core = core_atom_refs.iter().copied().collect::<BTreeSet<_>>();
    let mut boundary = density
        .into_iter()
        .filter(|(atom_ref, _, _)| !core.contains(atom_ref))
        .collect::<Vec<_>>();
    boundary.sort_unstable_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    let boundary_atom_refs = boundary
        .into_iter()
        .take(REVIEW_BOUNDARY_MAX)
        .map(|(atom_ref, _, _)| atom_ref)
        .collect();
    (core_atom_refs, boundary_atom_refs)
}

fn external_neighbors(
    cluster: i64,
    members: &[Uuid],
    labels: &BTreeMap<Uuid, Option<i64>>,
    neighborhoods: &NeighborIndex<'_>,
) -> Result<Vec<Uuid>, ModelError> {
    let mut external = BTreeMap::<Uuid, f64>::new();
    for member in members {
        let neighbors = neighborhoods.get(member).ok_or(ModelError::InvalidOutput)?;
        for neighbor in *neighbors {
            if matches!(labels.get(&neighbor.atom_id), Some(Some(label)) if *label != cluster) {
                external
                    .entry(neighbor.atom_id)
                    .and_modify(|score| *score = score.max(neighbor.cosine))
                    .or_insert(neighbor.cosine);
            }
        }
    }
    let mut external = external.into_iter().collect::<Vec<_>>();
    external.sort_unstable_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    Ok(external
        .into_iter()
        .take(REVIEW_NEIGHBOR_MAX)
        .map(|(atom_ref, _)| atom_ref)
        .collect())
}

fn hard_gates_pass(quality: &Value) -> bool {
    let gates = &quality["gates"];
    let ann = &quality["ann"];
    let routes = match quality["routes"].as_object() {
        Some(routes) if !routes.is_empty() => routes,
        _ => return false,
    };
    gates["inputConservation"] == "passed"
        && gates["annRecall"] == "passed"
        && gates["sameSeed"] == "passed"
        && gates["perturbation"] == "passed"
        && ann["status"] == "measured"
        && ann["k"].as_u64() == Some(20)
        && ann["queries"].as_u64() == Some(1000)
        && ann["recallAt20"]
            .as_f64()
            .is_some_and(|recall| recall >= 0.95)
        && ann["minimum"].as_f64() == Some(0.95)
        && routes.values().all(route_quality_passes)
}

fn route_quality_passes(route: &Value) -> bool {
    let same_seed = &route["seedMetrics"]["sameSeed"];
    let perturbation = &route["seedMetrics"]["perturbation"];
    same_seed["seed"].as_i64() == Some(17)
        && same_seed["repeats"].as_i64() == Some(2)
        && same_seed["labelPermutationEquivalent"].as_bool() == Some(true)
        && same_seed["noiseIdentityEquivalent"].as_bool() == Some(true)
        && same_seed["hardGate"] == "passed"
        && perturbation["seeds"] == json!([17, 29, 43])
        && perturbation["minimumAri"].as_f64() == Some(0.8)
        && perturbation["minimumCommonCore"].as_i64() == Some(30)
        && perturbation["meanAri"]
            .as_f64()
            .is_some_and(|ari| ari >= 0.8)
        && perturbation["commonCore"]
            .as_i64()
            .is_some_and(|count| count >= 30)
        && perturbation["gate"] == "passed"
}

fn compute_receipt(
    frozen: &snapshot::FrozenSnapshot,
    result: &SemanticComputeResult,
    comparison_quality: Option<&Value>,
    acceptable: bool,
    review_count: usize,
) -> Value {
    json!({
        "policyHash": frozen.policy_hash,
        "spaceHash": frozen.space_hash,
        "runSnapshotHash": frozen.run_snapshot_hash,
        "vectorHash": hex_hash(result.receipt.snapshot_sha256),
        "inputManifestHash": hex_hash(result.receipt.manifest_sha256),
        "resultHash": hex_hash(result.receipt.result_sha256),
        "resultBytes": result.receipt.bytes,
        "quality": result.quality,
        "comparisonQuality": comparison_quality,
        "artifact": result.artifact,
        "acceptedForReview": acceptable,
        "reviewCandidateCount": review_count,
    })
}

pub(crate) fn hex_hash(hash: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment_semantic_compute::{
        SemanticArtifact, SemanticAssignment, SemanticComputeReceipt, SemanticNeighbor,
        SemanticNeighborRecord,
    };
    use std::path::PathBuf;

    fn atom(number: u128) -> Uuid {
        Uuid::from_u128(number)
    }

    fn leiden_result() -> SemanticComputeResult {
        let atom_ids = (1..=20).map(atom).collect::<Vec<_>>();
        let assignments = atom_ids
            .iter()
            .map(|atom_id| SemanticAssignment {
                atom_id: *atom_id,
                cluster: Some(if atom_id.as_u128() <= 10 { 1 } else { 2 }),
                noise: false,
            })
            .collect();
        let neighbors = atom_ids
            .iter()
            .map(|atom_id| {
                let in_cluster = atom_ids
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        candidate != atom_id
                            && (candidate.as_u128() <= 10) == (atom_id.as_u128() <= 10)
                    })
                    .take(5)
                    .map(|candidate| SemanticNeighbor {
                        atom_id: candidate,
                        cosine: 0.9,
                    });
                let other_cluster = atom_ids
                    .iter()
                    .copied()
                    .filter(|candidate| (candidate.as_u128() <= 10) != (atom_id.as_u128() <= 10))
                    .take(3)
                    .map(|candidate| SemanticNeighbor {
                        atom_id: candidate,
                        cosine: 0.8,
                    });
                SemanticNeighborRecord {
                    atom_id: *atom_id,
                    neighbors: in_cluster.chain(other_cluster).collect(),
                }
            })
            .collect();
        SemanticComputeResult {
            receipt: SemanticComputeReceipt {
                run_ref: atom(100),
                space_hash: [0; 32],
                snapshot_sha256: [0; 32],
                result_path: PathBuf::new(),
                result_sha256: [0; 32],
                bytes: 1,
                manifest_path: PathBuf::new(),
                manifest_sha256: [0; 32],
            },
            assignments: BTreeMap::from([("leiden".into(), assignments)]),
            neighbors,
            quality: quality_fixture(),
            artifact: SemanticArtifact {
                policy_version: "ci-auto-cluster.policy.v1".into(),
                resource: json!({}),
                implementation: json!({}),
                checkpoint_recovery: false,
                result_has_topic_definition: false,
                second_embedding_stability: json!({"status":"not_configured"}),
            },
        }
    }

    fn quality_fixture() -> Value {
        let route = json!({
            "seedMetrics": {
                "sameSeed": {"seed":17,"repeats":2,"labelPermutationEquivalent":true,"noiseIdentityEquivalent":true,"hardGate":"passed"},
                "perturbation": {"seeds":[17,29,43],"minimumAri":0.8,"minimumCommonCore":30,"meanAri":0.9,"commonCore":30,"gate":"passed"}
            }
        });
        json!({
            "gates":{"inputConservation":"passed","annRecall":"passed","sameSeed":"passed","perturbation":"passed"},
            "ann":{"status":"measured","k":20,"queries":1000,"recallAt20":0.99,"minimum":0.95},
            "routes":{"leiden":route}
        })
    }

    #[test]
    fn samples_core_boundary_and_external_neighbors_without_duplicate_atoms() {
        let samples = review_samples(&leiden_result()).expect("valid leiden result");
        assert_eq!(samples.len(), 2);
        for sample in samples {
            assert_eq!(sample.roles.core_atom_refs.len(), 5);
            assert_eq!(sample.roles.boundary_atom_refs.len(), 4);
            assert_eq!(sample.roles.neighbor_atom_refs.len(), 3);
            assert_eq!(sample.sample_refs.len(), 12);
            assert_eq!(
                sample
                    .sample_refs
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len(),
                sample.sample_refs.len()
            );
            let cluster_one = sample.cluster_key == "1";
            assert!(
                sample
                    .roles
                    .neighbor_atom_refs
                    .iter()
                    .all(|atom_ref| { (atom_ref.as_u128() <= 10) != cluster_one })
            );
        }
    }

    #[test]
    fn hard_gates_reject_a_recall_value_below_the_frozen_minimum() {
        let mut quality = quality_fixture();
        assert!(hard_gates_pass(&quality));
        quality["ann"]["recallAt20"] = json!(0.949);
        assert!(!hard_gates_pass(&quality));
    }
}
