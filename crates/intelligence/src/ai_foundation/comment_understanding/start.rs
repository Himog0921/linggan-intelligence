//! T0: choose the researchable comments, group them by work and freeze one batch of units.
//!
//! Preview and start build the same plan from the same inputs. Start refuses to run a plan whose
//! fingerprint differs from the one the operator was shown, instead of researching something
//! other than what was previewed.
use super::{
    CONTRACT,
    plan::{PlannedTarget, PlannedUnit, WorkGroup, plan_hash, split},
    target::{self, Target},
};
use crate::{
    ai_foundation::{
        TaskKey,
        batch_store::{self, BatchInput, BatchKind, InputRef},
        content_understanding::core_cards_in,
        store::{Command, FoundationError},
        task_config::lock_setting,
    },
    comment_study_source::StudySourceRole,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// One start is one transaction; a larger selection is split by the caller.
pub const MAX_SELECTIONS: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub work_ref: Uuid,
    pub source_ref: Uuid,
    pub observation_role: StudySourceRole,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub selections: Vec<Selection>,
    pub knowledge_cutoff: String,
    pub expected_plan_hash: String,
    pub task_config_ref: Uuid,
    pub expected_task_revision: i64,
    /// Research comments that already have a current result again.
    #[serde(default)]
    pub rerun: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub batch_ref: Uuid,
    pub unit_count: usize,
    pub target_count: usize,
}

/// Why selected comments are not in the plan, for the preview.
#[derive(Debug, Default)]
struct Report {
    selected: usize,
    excluded: BTreeMap<&'static str, usize>,
    waiting_l1_targets: usize,
    waiting_l1_works: BTreeSet<Uuid>,
    already_researched: usize,
    already_queued: usize,
    dispatch_unresolved: usize,
}

struct Plan {
    units: Vec<PlannedUnit>,
    report: Report,
}

struct CommentState {
    researched: bool,
    queued: bool,
    unresolved: bool,
    truncations: u32,
}

/// What the research history already says about one comment in this domain.
async fn state_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    comment_external_id: &str,
) -> Result<CommentState, FoundationError> {
    let (researched, queued, unresolved, truncations): (bool, bool, bool, i64) = sqlx::query_as(
        "WITH latest AS (SELECT created_at FROM linggan_comment_current_research WHERE domain_ref=$1 AND work_ref=$2 AND comment_external_id=$3),
         mine AS (SELECT r.request_ref,r.created_at FROM linggan_comment_research_request r
          JOIN linggan_material_comment m ON m.material_ref=r.source_ref
          WHERE r.domain_ref=$1 AND r.work_ref=$2 AND m.comment_external_id=$3 AND r.method_version='comment-understanding.v1')
         SELECT EXISTS(SELECT 1 FROM latest),
          EXISTS(SELECT 1 FROM mine JOIN linggan_comment_research_unit_target ut USING(request_ref)
           JOIN linggan_comment_research_unit u ON u.unit_ref=ut.unit_ref
           JOIN linggan_ai_batch_item_control c ON c.item_ref=u.item_ref
           WHERE c.state IN('pending','running') AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_result x WHERE x.request_ref=mine.request_ref)),
          EXISTS(SELECT 1 FROM mine JOIN linggan_comment_research_unit_target ut USING(request_ref)
           JOIN linggan_comment_research_unit u ON u.unit_ref=ut.unit_ref
           JOIN linggan_ai_batch_item_control c ON c.item_ref=u.item_ref
           WHERE c.failure_code='ai_dispatch_unknown' AND mine.created_at>COALESCE((SELECT created_at FROM latest),'-infinity')),
          (SELECT count(*) FROM mine JOIN linggan_comment_research_result x USING(request_ref)
           WHERE x.failure_code='unit_truncated' AND mine.created_at>COALESCE((SELECT created_at FROM latest),'-infinity'))",
    )
    .bind(domain)
    .bind(work)
    .bind(comment_external_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(CommentState {
        researched,
        queued,
        unresolved,
        truncations: truncations.clamp(0, 16) as u32,
    })
}

/// The newest usable L1 Core of the work, as of `as_of`. Its `analysisRef` is the L1 request.
async fn l1_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    as_of: &str,
) -> Result<Option<Uuid>, FoundationError> {
    Ok(core_cards_in(tx, domain, work, as_of)
        .await?
        .first()
        .and_then(|card| card["analysisRef"].as_str())
        .and_then(|reference| Uuid::parse_str(reference).ok()))
}

async fn plan_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    selections: &[Selection],
    rerun: bool,
    as_of: &str,
) -> Result<Plan, FoundationError> {
    let unique: BTreeSet<&Selection> = selections.iter().collect();
    let mut report = Report {
        selected: unique.len(),
        ..Report::default()
    };
    let mut l1: BTreeMap<Uuid, Option<Uuid>> = BTreeMap::new();
    let mut groups: BTreeMap<Uuid, WorkGroup> = BTreeMap::new();
    for selection in unique {
        let target = match target::read_in(
            tx,
            domain,
            selection.work_ref,
            selection.source_ref,
            selection.observation_role,
            as_of,
        )
        .await?
        {
            Ok(target) => target,
            Err(code) => {
                *report.excluded.entry(code).or_default() += 1;
                continue;
            }
        };
        let l1_ref = match l1.get(&selection.work_ref) {
            Some(known) => *known,
            None => {
                let found = l1_in(tx, domain, selection.work_ref, as_of).await?;
                l1.insert(selection.work_ref, found);
                found
            }
        };
        let Some(l1_ref) = l1_ref else {
            report.waiting_l1_targets += 1;
            report.waiting_l1_works.insert(selection.work_ref);
            continue;
        };
        let state = state_in(tx, domain, selection.work_ref, &target.comment_external_id).await?;
        if state.queued {
            report.already_queued += 1;
            continue;
        }
        if state.unresolved {
            report.dispatch_unresolved += 1;
            continue;
        }
        if state.researched && !rerun {
            report.already_researched += 1;
            continue;
        }
        groups
            .entry(selection.work_ref)
            .or_insert_with(|| WorkGroup {
                work_ref: selection.work_ref,
                l1_request_ref: l1_ref,
                targets: Vec::new(),
            })
            .targets
            .push(planned(
                &target,
                selection.observation_role,
                l1_ref,
                state.truncations,
            ));
    }
    Ok(Plan {
        units: split(groups.into_values().collect()),
        report,
    })
}

fn planned(
    target: &Target,
    role: StudySourceRole,
    l1_ref: Uuid,
    truncations: u32,
) -> PlannedTarget {
    PlannedTarget {
        source_ref: target.source_ref,
        role,
        comment_external_id: target.comment_external_id.clone(),
        thread_key: target.thread_key.clone(),
        input_hash: target.input_hash(),
        input_fingerprint: target.fingerprint(l1_ref),
        truncations,
    }
}

async fn read_only_tx(db: &Database) -> Result<Transaction<'static, Postgres>, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// What a start over the same selection would do. Nothing is written.
pub async fn preview(
    db: &Database,
    domain: Uuid,
    selections: &[Selection],
    rerun: bool,
) -> Result<Value, FoundationError> {
    if domain.is_nil() || selections.is_empty() || selections.len() > MAX_SELECTIONS {
        return Err(FoundationError::Invalid);
    }
    let mut tx = read_only_tx(db).await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let (config, revision): (Uuid, i64) = sqlx::query_as(
        "SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='comment_understanding'",
    )
    .fetch_one(&mut *tx)
    .await?;
    let plan = plan_in(&mut tx, domain, selections, rerun, &as_of).await?;
    tx.commit().await?;
    let report = &plan.report;
    let targets: usize = plan.units.iter().map(|unit| unit.targets.len()).sum();
    Ok(json!({
        "domainRef": domain,
        "knowledgeCutoff": as_of,
        "planHash": plan_hash(&as_of, &plan.units),
        "taskConfigRef": config,
        "taskRevision": revision,
        "methodVersion": CONTRACT,
        "selectedCount": report.selected,
        "researchableCount": targets,
        "unitCount": plan.units.len(),
        "expectedModelCalls": plan.units.len(),
        "workCount": plan.units.iter().map(|u| u.work_ref).collect::<BTreeSet<_>>().len(),
        "waitingL1": {"targetCount": report.waiting_l1_targets, "workRefs": report.waiting_l1_works},
        "alreadyResearchedCount": report.already_researched,
        "alreadyQueuedCount": report.already_queued,
        "dispatchUnresolvedCount": report.dispatch_unresolved,
        "excludedCounts": report.excluded,
        "units": plan.units.iter().map(|u| json!({"workRef":u.work_ref,"l1RequestRef":u.l1_request_ref,"targetCount":u.targets.len()})).collect::<Vec<_>>(),
    }))
}

/// Freeze the previewed plan as one batch with one item per unit.
pub async fn start(db: &Database, request: &Start) -> Result<Receipt, FoundationError> {
    if request.domain_ref.is_nil()
        || request.selections.is_empty()
        || request.selections.len() > MAX_SELECTIONS
        || request.expected_task_revision < 0
        || request.expected_plan_hash.len() != 64
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "comment_understanding_start",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let config = lock_setting(
        &mut command.tx,
        TaskKey::CommentUnderstanding,
        request.expected_task_revision,
    )
    .await?;
    if config != request.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let in_the_past: bool = sqlx::query_scalar("SELECT $1::timestamptz<=scope_001_now()")
        .bind(&request.knowledge_cutoff)
        .fetch_one(&mut *command.tx)
        .await
        .map_err(|_| FoundationError::Invalid)?;
    if !in_the_past {
        return Err(FoundationError::Invalid);
    }
    // Two starts over the same comments must not both plan them.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "comment-understanding-start:{}",
            request.domain_ref
        ))
        .execute(&mut *command.tx)
        .await?;
    let plan = plan_in(
        &mut command.tx,
        request.domain_ref,
        &request.selections,
        request.rerun,
        &request.knowledge_cutoff,
    )
    .await?;
    if plan_hash(&request.knowledge_cutoff, &plan.units) != request.expected_plan_hash {
        return Err(FoundationError::Conflict);
    }
    if plan.units.is_empty() {
        return Err(FoundationError::AdmissionDenied(vec![
            "no_researchable_comment".into(),
        ]));
    }
    let inputs: Vec<BatchInput> = plan
        .units
        .iter()
        .map(|unit| BatchInput {
            item_key: unit.unit_ref.to_string(),
            input: InputRef::CommentResearchUnit {
                unit_ref: unit.unit_ref,
            },
        })
        .collect();
    let hash = batch_store::input_hash(TaskKey::CommentUnderstanding, &inputs, false)?;
    let batch_ref = batch_store::insert_batch(
        &mut command.tx,
        request.command_ref,
        TaskKey::CommentUnderstanding,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    let mut target_count = 0;
    for unit in &plan.units {
        sqlx::query("INSERT INTO linggan_comment_research_unit(unit_ref,item_ref,domain_ref,work_ref,l1_request_ref) SELECT $1,item_ref,$3,$4,$5 FROM linggan_ai_batch_item WHERE batch_ref=$2 AND item_key=$6")
            .bind(unit.unit_ref).bind(batch_ref).bind(request.domain_ref).bind(unit.work_ref)
            .bind(unit.l1_request_ref).bind(unit.unit_ref.to_string())
            .execute(&mut *command.tx).await?;
        for (index, target) in unit.targets.iter().enumerate() {
            let request_ref = Uuid::new_v4();
            sqlx::query("INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version) VALUES($1,$2,$3,$4,$5,$6,$7::timestamptz,$8,$9,$10,$11,$12)")
                .bind(request_ref).bind(request.command_ref).bind(request.domain_ref).bind(unit.work_ref)
                .bind(target.source_ref).bind(target.role.as_str()).bind(&request.knowledge_cutoff)
                .bind(&target.input_hash).bind(&target.input_fingerprint).bind(config).bind(batch_ref).bind(CONTRACT)
                .execute(&mut *command.tx).await?;
            sqlx::query("INSERT INTO linggan_comment_research_unit_target(request_ref,unit_ref,position) VALUES($1,$2,$3)")
                .bind(request_ref).bind(unit.unit_ref).bind(index as i32 + 1)
                .execute(&mut *command.tx).await?;
            target_count += 1;
        }
    }
    command
        .finish(Receipt {
            batch_ref,
            unit_count: plan.units.len(),
            target_count,
        })
        .await
}
