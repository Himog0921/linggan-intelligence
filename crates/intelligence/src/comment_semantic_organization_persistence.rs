//! Durable group, membership, formal-problem projection, and lineage writes.
//!
//! Reconciliation chooses the transition; this module applies it atomically
//! while preserving superseded records for historical inspection.

use super::review::{AcceptedCandidate, ExistingGroup};
use crate::model_settings::ModelError;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;

pub(super) async fn current_groups(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain_ref: Uuid,
    kind: &str,
) -> Result<Vec<ExistingGroup>, ModelError> {
    let rows = sqlx::query(
        "SELECT group_ref,problem_ref,name,definition FROM linggan_ci_semantic_group \
         WHERE domain_ref=$1 AND kind=$2 AND state='active' ORDER BY group_ref FOR UPDATE",
    )
    .bind(domain_ref)
    .bind(kind)
    .fetch_all(&mut **transaction)
    .await?;
    let mut groups = Vec::with_capacity(rows.len());
    for row in rows {
        let group_ref: Uuid = row.get("group_ref");
        let current_same_atoms = sqlx::query_scalar(
            "SELECT atom_ref FROM linggan_ci_atom_membership \
             WHERE group_ref=$1 AND current AND relation='same' ORDER BY atom_ref",
        )
        .bind(group_ref)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
        groups.push(ExistingGroup {
            group_ref,
            problem_ref: row.get("problem_ref"),
            name: row.get("name"),
            definition: row.get("definition"),
            current_same_atoms,
        });
    }
    Ok(groups)
}

pub(super) async fn supersede_group(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_ref: Uuid,
    run_ref: Uuid,
) -> Result<(), ModelError> {
    sqlx::query(
        "DELETE FROM linggan_ci_problem_member member USING linggan_ci_semantic_group group_row \
         WHERE group_row.group_ref=$1 AND member.problem_ref=group_row.problem_ref \
           AND member.origin='model_equivalence'",
    )
    .bind(group_ref)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE linggan_ci_atom_membership \
         SET current=false,superseded_by_run=$2,superseded_reason='reclustered',superseded_at=scope_001_now() \
         WHERE group_ref=$1 AND current",
    )
    .bind(group_ref)
    .bind(run_ref)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE linggan_ci_semantic_group \
         SET state='superseded',superseded_by_run=$2,superseded_reason='reclustered',updated_at=scope_001_now() \
         WHERE group_ref=$1 AND state='active'",
    )
    .bind(group_ref)
    .bind(run_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn retire_current_memberships(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_ref: Uuid,
    run_ref: Uuid,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE linggan_ci_atom_membership \
         SET current=false,superseded_by_run=$2,superseded_reason='reclustered',superseded_at=scope_001_now() \
         WHERE group_ref=$1 AND current",
    )
    .bind(group_ref)
    .bind(run_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn write_current_memberships(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group_ref: Uuid,
    run_ref: Uuid,
    candidate: &AcceptedCandidate,
) -> Result<(), ModelError> {
    for (relation, atoms) in [
        ("same", &candidate.same_atoms),
        ("related", &candidate.related_atoms),
    ] {
        for atom_ref in atoms {
            sqlx::query(
                "INSERT INTO linggan_ci_atom_membership(group_ref,atom_ref,run_ref,relation,decision_ref,current) \
                 VALUES($1,$2,$3,$4,$5,true)",
            )
            .bind(group_ref)
            .bind(atom_ref)
            .bind(run_ref)
            .bind(relation)
            .bind(candidate.review_ref)
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(())
}

pub(super) async fn create_group(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    domain_ref: Uuid,
    kind: &str,
    candidate: &AcceptedCandidate,
) -> Result<(Uuid, Option<Uuid>), ModelError> {
    let group_ref = Uuid::new_v4();
    let problem_ref = if kind == "problem" {
        let problem_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO linggan_ci_problem(problem_ref,domain_ref,name,meaning,definition,origin) \
             VALUES($1,$2,$3,$4,$5,'model_expression')",
        )
        .bind(problem_ref)
        .bind(domain_ref)
        .bind(&candidate.name)
        .bind(&candidate.definition)
        .bind(problem_definition(group_ref, run_ref, candidate))
        .execute(&mut **transaction)
        .await?;
        Some(problem_ref)
    } else {
        None
    };
    sqlx::query(
        "INSERT INTO linggan_ci_semantic_group(group_ref,domain_ref,kind,problem_ref,name,definition) \
         VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(group_ref)
    .bind(domain_ref)
    .bind(kind)
    .bind(problem_ref)
    .bind(&candidate.name)
    .bind(&candidate.definition)
    .execute(&mut **transaction)
    .await?;
    Ok((group_ref, problem_ref))
}

pub(super) async fn update_group_definition(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    group: &ExistingGroup,
    run_ref: Uuid,
    candidate: &AcceptedCandidate,
) -> Result<bool, ModelError> {
    let changed = group.name != candidate.name || group.definition != candidate.definition;
    if !changed {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE linggan_ci_semantic_group \
         SET name=$2,definition=$3,revision=revision+1,updated_at=scope_001_now() \
         WHERE group_ref=$1 AND state='active'",
    )
    .bind(group.group_ref)
    .bind(&candidate.name)
    .bind(&candidate.definition)
    .execute(&mut **transaction)
    .await?;
    if let Some(problem_ref) = group.problem_ref {
        sqlx::query(
            "UPDATE linggan_ci_problem \
             SET name=$2,meaning=$3,definition=definition||$4,revision=revision+1, \
                 definition_revision=definition_revision+1,updated_at=scope_001_now() WHERE problem_ref=$1",
        )
        .bind(problem_ref)
        .bind(&candidate.name)
        .bind(&candidate.definition)
        .bind(problem_definition(group.group_ref, run_ref, candidate))
        .execute(&mut **transaction)
        .await?;
    }
    Ok(true)
}

pub(super) async fn refresh_problem_members(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    problem_ref: Uuid,
    group_ref: Uuid,
) -> Result<(), ModelError> {
    sqlx::query(
        "DELETE FROM linggan_ci_problem_member member \
         WHERE member.problem_ref=$1 AND member.origin='model_equivalence' \
           AND NOT EXISTS(SELECT 1 FROM linggan_ci_atom_membership membership \
                          JOIN linggan_ci_semantic_atom_current atom ON atom.atom_ref=membership.atom_ref \
                          WHERE membership.group_ref=$2 AND membership.current AND membership.relation='same' \
                            AND atom.canonical_ref=member.canonical_ref)",
    )
    .bind(problem_ref)
    .bind(group_ref)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,evidence,analysis_ref) \
         SELECT $1,atom.canonical_ref,'model_equivalence',jsonb_agg(DISTINCT evidence.item), \
                (array_agg(atom.analysis_ref ORDER BY atom.created_at DESC,atom.analysis_ref))[1] \
         FROM linggan_ci_atom_membership membership \
         JOIN linggan_ci_semantic_atom_current atom ON atom.atom_ref=membership.atom_ref \
         CROSS JOIN LATERAL jsonb_array_elements(atom.evidence) evidence(item) \
         WHERE membership.group_ref=$2 AND membership.current AND membership.relation='same' \
         GROUP BY atom.canonical_ref \
         ON CONFLICT(problem_ref,canonical_ref) DO UPDATE \
           SET origin='model_equivalence',evidence=EXCLUDED.evidence,analysis_ref=EXCLUDED.analysis_ref \
           WHERE linggan_ci_problem_member.origin='model_equivalence'",
    )
    .bind(problem_ref)
    .bind(group_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(super) async fn write_lineage(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: &str,
    from_groups: Vec<Uuid>,
    to_groups: Vec<Uuid>,
    reason: &str,
    run_ref: Uuid,
    candidate: &AcceptedCandidate,
) -> Result<(), ModelError> {
    sqlx::query(
        "INSERT INTO linggan_ci_semantic_lineage(lineage_ref,kind,from_groups,to_groups,reason,run_ref,invocation_ref,receipt) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(Uuid::new_v4())
    .bind(kind)
    .bind(from_groups)
    .bind(to_groups)
    .bind(reason)
    .bind(run_ref)
    .bind(candidate.invocation_ref)
    .bind(json!({
        "semanticReviewRef":candidate.review_ref,
        "algorithm":candidate.algorithm,
        "clusterKey":candidate.cluster_key,
        "sameScope":&candidate.same_scope,
        "sameMemberCount":candidate.same_atoms.len(),
        "relatedMemberCount":candidate.related_atoms.len()
    }))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn problem_definition(group_ref: Uuid, run_ref: Uuid, candidate: &AcceptedCandidate) -> Value {
    json!({
        "semanticGroupRef":group_ref,
        "semanticDefinition":candidate.definition,
        "semanticReviewRef":candidate.review_ref,
        "clusterRunRef":run_ref,
        "algorithm":candidate.algorithm,
        "clusterKey":candidate.cluster_key
    })
}
