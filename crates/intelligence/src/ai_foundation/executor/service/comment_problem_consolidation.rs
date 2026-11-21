//! Prepare one scoped consolidation call and settle its accepted decisions with its receipt.
use super::{
    super::{
        ExecutorError,
        queue::ClaimedItem,
        receipt::{self, ItemResult},
    },
    research_call,
};
use crate::{
    ai_foundation::{
        admission,
        batch_store::InputRef,
        comment_understanding::{
            CONSOLIDATION_CONTRACT, CONSOLIDATION_RESPONSE_REQUIREMENTS, Verdict,
            accept_consolidation, catalog, consolidation_schema, redirect,
        },
        store::FoundationError,
        transport::ResponseSchema,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

const CONSOLIDATE_FULL_MAX: usize = 150;

struct Prepared {
    call: research_call::Call,
    domain: Uuid,
}

enum Preparation {
    Ready(Box<Prepared>),
    Skip(&'static str),
}

async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Preparation, ExecutorError> {
    let InputRef::CommentProblemConsolidation { domain_ref: domain } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Ok(Preparation::Skip("batch_cancelled"));
    }
    let linked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_problem_consolidation_batch
         WHERE batch_ref=$1 AND domain_ref=$2)",
    )
    .bind(item.batch_ref)
    .bind(domain)
    .fetch_one(&mut *tx)
    .await?;
    if !linked {
        return Err(ExecutorError::Invalid);
    }
    admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
        .await?;
    admission::check_queue_input_scope(&mut tx, item.task_key, item.task_config_ref, &item.input)
        .await?;
    redirect::lock_in(&mut tx, domain).await?;
    let states: BTreeMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT current.problem_ref,current.state FROM linggan_comment_problem_current current
         JOIN linggan_comment_problem_definition definition USING(problem_ref)
         WHERE current.domain_ref=$1 AND definition.origin_invocation_ref IS NOT NULL
         AND NOT EXISTS (SELECT 1 FROM linggan_comment_problem_withdrawn_source x WHERE x.problem_ref=current.problem_ref)",
    )
    .bind(domain)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .collect();
    let candidate_count = states
        .values()
        .filter(|state| *state == "candidate")
        .count();
    if candidate_count > CONSOLIDATE_FULL_MAX || states.len() > catalog::CATALOG_FULL_MAX {
        return Ok(Preparation::Skip("comment_catalog_too_large"));
    }
    if candidate_count == 0 || states.len() < 2 {
        return Ok(Preparation::Skip("comment_consolidation_no_pairs"));
    }
    let problems: Vec<_> = catalog::read_in(&mut tx, domain)
        .await?
        .into_iter()
        .filter(|problem| states.contains_key(&problem.problem_ref))
        .collect();
    let mut pairs = BTreeMap::new();
    for (left_index, left) in problems.iter().enumerate() {
        for (right_index, right) in problems.iter().enumerate().skip(left_index + 1) {
            if states[&left.problem_ref] == "candidate" || states[&right.problem_ref] == "candidate"
            {
                let key = format!("q{}-{}", left_index + 1, right_index + 1);
                pairs.insert(key, [left.problem_ref, right.problem_ref]);
            }
        }
    }
    let entries: Vec<_> = catalog::entries(&problems)
        .into_iter()
        .zip(&problems)
        .map(|(entry, problem)| json!({
            "ref":entry.alias, "state":states[&problem.problem_ref], "label":entry.label,
            "actor":entry.actor, "goal":entry.goal, "barrier":entry.barrier, "context":entry.context,
        }))
        .collect();
    let fingerprint = content_hash(&json!([entries, pairs]).to_string());
    let call = research_call::prepare_in(
        &mut tx,
        item,
        secrets,
        research_call::Input {
            contract: CONSOLIDATION_CONTRACT,
            reference: item.batch_ref,
            fingerprint: &fingerprint,
            payload: json!({"contract":CONSOLIDATION_CONTRACT,"catalog":entries,
                "responseRequirements":CONSOLIDATION_RESPONSE_REQUIREMENTS}),
            response_schema: ResponseSchema {
                name: "comment_problem_consolidation".into(),
                schema: consolidation_schema(),
            },
            material_class: "comment_material",
        },
    )
    .await?;
    sqlx::query(
        "INSERT INTO linggan_comment_problem_consolidation_dispatch(invocation_ref,item_ref,pairs)
         VALUES($1,$2,$3)",
    )
    .bind(call.invocation)
    .bind(item.item_ref)
    .bind(sqlx::types::Json(&pairs))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Preparation::Ready(Box::new(Prepared { call, domain })))
}

pub(super) async fn execute(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let prepared = match prepare(db, item, secrets).await {
        Ok(Preparation::Ready(prepared)) => prepared,
        Ok(Preparation::Skip(reason)) => return item.skip_unattempted(db, reason).await,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons))) => {
            let reason = reasons.first().map_or("admission_denied", String::as_str);
            return item.defer(db, reason).await;
        }
        Err(error) => return super::super::preparation::handle(db, item, error).await,
    };
    let response = super::super::dispatch::call(db, item, adapter, &prepared.call.request).await;
    receipt::finalize(
        db,
        item,
        prepared.call.invocation,
        response.as_ref(),
        || settle(db, item, &prepared, response.as_ref()),
    )
    .await
}

async fn settle(
    db: &Database,
    item: &ClaimedItem,
    prepared: &Prepared,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let invocation = prepared.call.invocation;
    let (mut tx, cancelled) = receipt::lock_in(db, item, invocation, response).await?;
    let allowed = if cancelled {
        false
    } else {
        match async {
            admission::check_queue_configuration(
                &mut tx,
                item.task_key,
                item.task_config_ref,
                true,
            )
            .await?;
            admission::check_queue_input_scope(
                &mut tx,
                item.task_key,
                item.task_config_ref,
                &item.input,
            )
            .await
        }
        .await
        {
            Ok(()) => true,
            Err(FoundationError::AdmissionDenied(_)) => false,
            Err(error) => return Err(error.into()),
        }
    };
    let pairs: sqlx::types::Json<BTreeMap<String, [Uuid; 2]>> = sqlx::query_scalar(
        "SELECT pairs FROM linggan_comment_problem_consolidation_dispatch
         WHERE invocation_ref=$1 AND item_ref=$2",
    )
    .bind(invocation)
    .bind(item.item_ref)
    .fetch_one(&mut *tx)
    .await?;
    let decisions = response
        .filter(|response| allowed && response.ok)
        .and_then(|response| response.text.as_deref())
        .and_then(|text| accept_consolidation(text, &pairs.keys().cloned().collect()).ok());
    let decided = decisions.as_deref().unwrap_or_default();
    let redirects = if decisions.is_some() {
        redirect::apply_in(&mut tx, prepared.domain, invocation, &pairs, decided).await?
    } else {
        Default::default()
    };
    let success = decisions.is_some();
    let failure = research_call::failure(response, allowed, success, "ai_source_unavailable");
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    result["promptVersion"] = json!(prepared.call.prompt_version_ref);
    if success {
        // The frozen dispatch already maps pair keys to definitions. Store only the verdict
        // and apply outcome here, without retaining model-generated reason text.
        result["pairDecisions"] = json!(decided.iter().map(|decision| {
            let application = if redirects.contains(&decision.pair_key) {
                "redirect_applied"
            } else if decision.verdict == Verdict::Same {
                "pair_no_longer_current"
            } else {
                "kept_separate"
            };
            json!({"pairKey":decision.pair_key,"verdict":decision.verdict,"application":application})
        }).collect::<Vec<_>>());
    }
    receipt::observe_in(&mut tx, invocation, response).await?;
    finish_invocation_in(&mut tx, invocation, response, success, failure, &result).await?;
    let output: Value = json!({
        "methodVersion":CONSOLIDATION_CONTRACT,
        "decidedPairs":decided.len(),
        "redirects":redirects.len(),
    });
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success,
            cancelled,
            output: success.then_some(&output),
            failure,
            counts_as_failure: !success && allowed && !receipt::not_dispatched(response),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
