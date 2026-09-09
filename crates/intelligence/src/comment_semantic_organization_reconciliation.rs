//! Current-input fencing, split/merge selection, and reconciliation.
//!
//! This is separate from provider reservation and from the durable write
//! primitives so lineage transitions can be reviewed as one state machine.

use super::{
    decision::ReviewDecision,
    persistence::{
        create_group, current_groups, refresh_problem_members, retire_current_memberships,
        supersede_group, update_group_definition, write_current_memberships, write_lineage,
    },
    review::{AcceptedCandidate, ExistingGroup, MaterializedGroup, cluster_is_current_for},
};
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) async fn reconcile_ready_run(db: &Database) -> Result<bool, ModelError> {
    let mut transaction = db.pool().begin().await?;
    let run_ref = sqlx::query_scalar(
        "SELECT run.run_ref FROM linggan_ci_cluster_run run \
         WHERE run.state='reviewing' \
           AND NOT EXISTS(SELECT 1 FROM linggan_ci_cluster_review review \
                          WHERE review.run_ref=run.run_ref AND review.state IN('pending','running')) \
         ORDER BY run.created_at,run.run_ref FOR UPDATE OF run SKIP LOCKED LIMIT 1",
    )
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(run_ref) = run_ref else {
        transaction.rollback().await?;
        return Ok(false);
    };
    reconcile_run_if_complete(&mut transaction, run_ref).await?;
    transaction.commit().await?;
    Ok(true)
}

pub(super) async fn reconcile_run_if_complete(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<(), ModelError> {
    let Some((domain_ref, kind, space_ref)) = sqlx::query(
        "SELECT domain_ref,kind,space_ref FROM linggan_ci_cluster_run run \
         WHERE run.run_ref=$1 AND run.state='reviewing' \
           AND NOT EXISTS(SELECT 1 FROM linggan_ci_cluster_review review \
                          WHERE review.run_ref=run.run_ref AND review.state IN('pending','running')) \
         FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .map(|row| {
        (
            row.get::<Uuid, _>("domain_ref"),
            row.get::<String, _>("kind"),
            row.get::<Uuid, _>("space_ref"),
        )
    }) else {
        return Ok(());
    };
    let candidates = accepted_candidates(transaction, run_ref, space_ref).await?;
    let organized =
        reconcile_candidates(transaction, run_ref, domain_ref, &kind, candidates).await?;
    sqlx::query(
        "UPDATE linggan_ci_cluster_run \
         SET state=CASE WHEN $2 THEN 'accepted' ELSE 'insufficient' END, \
             failure_code=CASE WHEN $2 THEN NULL ELSE 'no_semantic_group_adopted' END, \
             finished_at=scope_001_now(),lease_until=NULL \
         WHERE run_ref=$1 AND state='reviewing'",
    )
    .bind(run_ref)
    .bind(organized)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn accepted_candidates(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    space_ref: Uuid,
) -> Result<Vec<AcceptedCandidate>, ModelError> {
    let rows = sqlx::query(
        "SELECT review_ref,invocation_ref,cluster_key,algorithm,receipt \
         FROM linggan_ci_cluster_review \
         WHERE run_ref=$1 AND state='accepted' ORDER BY cluster_key,review_ref FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_all(&mut **transaction)
    .await?;
    let mut accepted = Vec::new();
    for row in rows {
        let review_ref: Uuid = row.get("review_ref");
        let receipt: Value = row.get("receipt");
        let Some(decision) = receipt
            .get("decision")
            .cloned()
            .and_then(|value| serde_json::from_value::<ReviewDecision>(value).ok())
            .filter(|decision| decision.decision == "same")
        else {
            mark_review_independent(transaction, review_ref, "review_decision_missing").await?;
            continue;
        };
        let algorithm: String = row.get("algorithm");
        let cluster_key: String = row.get("cluster_key");
        let all_atoms = cluster_atoms(transaction, run_ref, &algorithm, &cluster_key).await?;
        let related_atoms = decision
            .related_atom_refs
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let same_scope = decision.same_scope.as_deref().unwrap_or("cluster");
        if all_atoms.is_empty()
            || !related_atoms.is_subset(&all_atoms)
            || !cluster_is_current_for(transaction, run_ref, &algorithm, &cluster_key, space_ref)
                .await?
        {
            mark_review_independent(transaction, review_ref, "current_input_changed").await?;
            continue;
        }
        let admitted = if same_scope == "core" {
            decision
                .core_atom_refs
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
        } else {
            all_atoms
        };
        let same_atoms = admitted
            .difference(&related_atoms)
            .copied()
            .collect::<BTreeSet<_>>();
        if same_atoms.is_empty() {
            mark_review_independent(transaction, review_ref, "no_same_cluster_members").await?;
            continue;
        }
        accepted.push(AcceptedCandidate {
            review_ref,
            invocation_ref: row.get("invocation_ref"),
            cluster_key,
            algorithm,
            name: decision.name.ok_or(ModelError::InvalidOutput)?,
            definition: decision.definition.ok_or(ModelError::InvalidOutput)?,
            same_scope: same_scope.into(),
            same_atoms,
            related_atoms,
        });
    }
    Ok(accepted)
}

async fn cluster_atoms(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    algorithm: &str,
    cluster_key: &str,
) -> Result<BTreeSet<Uuid>, ModelError> {
    Ok(sqlx::query_scalar(
        "SELECT input.atom_ref FROM linggan_ci_cluster_assignment assignment \
         JOIN linggan_ci_cluster_input input ON input.run_ref=assignment.run_ref AND input.atom_ref=assignment.atom_ref \
         WHERE assignment.run_ref=$1 AND assignment.algorithm=$2 AND assignment.cluster_key=$3 \
         ORDER BY input.atom_ref",
    )
    .bind(run_ref)
    .bind(algorithm)
    .bind(cluster_key)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .collect())
}

async fn mark_review_independent(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    review_ref: Uuid,
    reason: &str,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE linggan_ci_cluster_review SET state='independent',lease_until=NULL, \
         receipt=receipt||jsonb_build_object('adoption','preserved_independent_current_input_changed','reason',$2) \
         WHERE review_ref=$1 AND state='accepted'",
    )
    .bind(review_ref)
    .bind(reason)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn reconcile_candidates(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    domain_ref: Uuid,
    kind: &str,
    candidates: Vec<AcceptedCandidate>,
) -> Result<bool, ModelError> {
    if candidates.is_empty() {
        return Ok(false);
    }
    let groups = current_groups(transaction, domain_ref, kind).await?;
    let (candidate_sources, source_candidates) = candidate_sources(&candidates, &groups);
    let superseded = superseded_groups(&candidate_sources, &source_candidates);
    for source in &superseded {
        supersede_group(transaction, *source, run_ref).await?;
    }
    let output = materialize_candidates(
        transaction,
        run_ref,
        domain_ref,
        kind,
        &candidates,
        &groups,
        &candidate_sources,
        &superseded,
    )
    .await?;
    record_reconciliation(
        transaction,
        run_ref,
        &candidates,
        &candidate_sources,
        &source_candidates,
        &superseded,
        &output,
    )
    .await?;
    Ok(true)
}

fn candidate_sources(
    candidates: &[AcceptedCandidate],
    groups: &[ExistingGroup],
) -> (Vec<Vec<Uuid>>, BTreeMap<Uuid, Vec<usize>>) {
    let mut by_candidate = Vec::with_capacity(candidates.len());
    let mut by_source = BTreeMap::<Uuid, Vec<usize>>::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let sources = groups
            .iter()
            .filter(|group| !group.current_same_atoms.is_disjoint(&candidate.same_atoms))
            .map(|group| group.group_ref)
            .collect::<Vec<_>>();
        for source in &sources {
            by_source.entry(*source).or_default().push(index);
        }
        by_candidate.push(sources);
    }
    (by_candidate, by_source)
}

fn superseded_groups(
    candidate_sources: &[Vec<Uuid>],
    source_candidates: &BTreeMap<Uuid, Vec<usize>>,
) -> BTreeSet<Uuid> {
    let mut superseded = source_candidates
        .iter()
        .filter(|(_, indexes)| indexes.len() > 1)
        .map(|(source, _)| *source)
        .collect::<BTreeSet<_>>();
    for sources in candidate_sources {
        if sources.len() > 1 {
            superseded.extend(sources.iter().copied());
        }
    }
    superseded
}

async fn materialize_candidates(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    domain_ref: Uuid,
    kind: &str,
    candidates: &[AcceptedCandidate],
    groups: &[ExistingGroup],
    candidate_sources: &[Vec<Uuid>],
    superseded: &BTreeSet<Uuid>,
) -> Result<Vec<MaterializedGroup>, ModelError> {
    let by_ref = groups
        .iter()
        .map(|group| (group.group_ref, group))
        .collect::<BTreeMap<_, _>>();
    let mut output = Vec::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        let sources = &candidate_sources[index];
        let reusable = sources
            .iter()
            .copied()
            .filter(|source| !superseded.contains(source))
            .collect::<Vec<_>>();
        let materialized = if reusable.len() == 1 && sources.len() == 1 {
            reuse_group(transaction, run_ref, kind, candidate, by_ref[&reusable[0]]).await?
        } else {
            create_materialized_group(transaction, run_ref, domain_ref, kind, candidate).await?
        };
        output.push(MaterializedGroup {
            candidate_index: index,
            ..materialized
        });
    }
    Ok(output)
}

async fn reuse_group(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    kind: &str,
    candidate: &AcceptedCandidate,
    group: &ExistingGroup,
) -> Result<MaterializedGroup, ModelError> {
    if (kind == "problem") != group.problem_ref.is_some() {
        return Err(ModelError::InvalidOutput);
    }
    let renamed = update_group_definition(transaction, group, run_ref, candidate).await?;
    retire_current_memberships(transaction, group.group_ref, run_ref).await?;
    write_current_memberships(transaction, group.group_ref, run_ref, candidate).await?;
    Ok(MaterializedGroup {
        candidate_index: 0,
        group_ref: group.group_ref,
        problem_ref: group.problem_ref,
        reused: true,
        lineage_kind: if renamed { "rename" } else { "membership" },
        lineage_reason: if renamed {
            "bounded semantic review renamed an existing stable group"
        } else {
            "bounded semantic review updated an existing stable group"
        },
    })
}

async fn create_materialized_group(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    domain_ref: Uuid,
    kind: &str,
    candidate: &AcceptedCandidate,
) -> Result<MaterializedGroup, ModelError> {
    let (group_ref, problem_ref) =
        create_group(transaction, run_ref, domain_ref, kind, candidate).await?;
    write_current_memberships(transaction, group_ref, run_ref, candidate).await?;
    Ok(MaterializedGroup {
        candidate_index: 0,
        group_ref,
        problem_ref,
        reused: false,
        lineage_kind: "create",
        lineage_reason: "bounded semantic review accepted a new stable group",
    })
}

async fn record_reconciliation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    candidates: &[AcceptedCandidate],
    candidate_sources: &[Vec<Uuid>],
    source_candidates: &BTreeMap<Uuid, Vec<usize>>,
    superseded: &BTreeSet<Uuid>,
    output: &[MaterializedGroup],
) -> Result<(), ModelError> {
    for materialized in output {
        record_candidate(
            transaction,
            run_ref,
            candidates,
            candidate_sources,
            materialized,
        )
        .await?;
    }
    record_merge_lineage(transaction, run_ref, candidates, candidate_sources, output).await?;
    record_split_lineage(
        transaction,
        run_ref,
        candidates,
        source_candidates,
        superseded,
        output,
    )
    .await
}

async fn record_candidate(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    candidates: &[AcceptedCandidate],
    candidate_sources: &[Vec<Uuid>],
    materialized: &MaterializedGroup,
) -> Result<(), ModelError> {
    if let Some(problem_ref) = materialized.problem_ref {
        refresh_problem_members(transaction, problem_ref, materialized.group_ref).await?;
    }
    let candidate = &candidates[materialized.candidate_index];
    sqlx::query("UPDATE linggan_ci_cluster_review SET receipt=receipt||$2 WHERE review_ref=$1")
        .bind(candidate.review_ref)
        .bind(json!({"organization":{
            "groupRef":materialized.group_ref,
            "sameMemberCount":candidate.same_atoms.len(),
            "relatedMemberCount":candidate.related_atoms.len(),
            "sameScope":&candidate.same_scope,
            "currentGenerationRunRef":run_ref
        }}))
        .execute(&mut **transaction)
        .await?;
    if materialized.reused {
        write_lineage(
            transaction,
            materialized.lineage_kind,
            vec![materialized.group_ref],
            vec![materialized.group_ref],
            materialized.lineage_reason,
            run_ref,
            candidate,
        )
        .await?;
    } else if candidate_sources[materialized.candidate_index].is_empty() {
        write_lineage(
            transaction,
            "create",
            Vec::new(),
            vec![materialized.group_ref],
            materialized.lineage_reason,
            run_ref,
            candidate,
        )
        .await?;
    }
    Ok(())
}

async fn record_merge_lineage(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    candidates: &[AcceptedCandidate],
    candidate_sources: &[Vec<Uuid>],
    output: &[MaterializedGroup],
) -> Result<(), ModelError> {
    for (index, sources) in candidate_sources.iter().enumerate() {
        if sources.len() > 1 {
            write_lineage(
                transaction,
                "merge",
                sources.clone(),
                vec![output[index].group_ref],
                "bounded semantic review merged previously separate current groups",
                run_ref,
                &candidates[index],
            )
            .await?;
        }
    }
    Ok(())
}

async fn record_split_lineage(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    candidates: &[AcceptedCandidate],
    source_candidates: &BTreeMap<Uuid, Vec<usize>>,
    superseded: &BTreeSet<Uuid>,
    output: &[MaterializedGroup],
) -> Result<(), ModelError> {
    for source in superseded {
        let Some(indexes) = source_candidates.get(source) else {
            continue;
        };
        let targets = indexes
            .iter()
            .map(|index| output[*index].group_ref)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if targets.len() > 1 {
            write_lineage(
                transaction,
                "split",
                vec![*source],
                targets,
                "bounded semantic review split a prior current group into conservative clusters",
                run_ref,
                &candidates[indexes[0]],
            )
            .await?;
        }
    }
    Ok(())
}
