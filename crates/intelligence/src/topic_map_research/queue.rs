//! Incremental source work, explicit retry rules and one frozen run budget.
use super::windows::{is_research_evidence, origin_id};
use super::{
    INPUT_CONTRACT, RESEARCH_WINDOW_CHARS, ResearchError, ResearchInput, load_inputs,
    reference_manifest, research_windows,
};
use crate::topic_map_research_analysis::METHOD_VERSION;
use linggan_evidence::creator_discovery;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) fn should_queue_input(states: &[String], trigger: &str) -> bool {
    if states.is_empty() {
        return true;
    }
    if states.iter().any(|state| {
        matches!(
            state.as_str(),
            "queued" | "running" | "succeeded" | "no_signal" | "insufficient" | "unknown_dispatch"
        )
    }) {
        return false;
    }
    states.iter().all(|state| state == "stale")
        || (trigger == "on_demand"
            && states
                .iter()
                .any(|state| matches!(state.as_str(), "failed" | "stopped")))
}

/// A different config, method or window size cannot authorize replaying a source
/// range whose provider dispatch is still unknown. New qualified text can advance.
pub(crate) fn overlaps_unknown_dispatch(
    window: &ResearchInput,
    full: &ResearchInput,
    refs: &[Value],
) -> bool {
    fn same_field(a: &str, b: &str) -> bool {
        a == b
            || (["studied_comment", "unresearched_comment"].contains(&a)
                && ["studied_comment", "unresearched_comment"].contains(&b))
    }
    window
        .fragments
        .iter()
        .filter(|f| is_research_evidence(f))
        .any(|fragment| {
            let origin = origin_id(window, fragment);
            let Some(source) = full
                .fragments
                .iter()
                .find(|source| source.fragment_id == origin)
            else {
                return false;
            };
            refs.iter().any(|reference| {
                let Some(field) = reference["field"].as_str() else {
                    return false;
                };
                let owner = reference["workRef"]
                    .as_str()
                    .or_else(|| reference["fragmentId"].as_str()?.split('.').next());
                if owner != origin.split('.').next() {
                    return false;
                }
                if !same_field(field, &source.field)
                    || !(reference["sourceRef"] == json!(source.source_ref)
                        || reference["sourceFragmentId"].as_str() == Some(origin.as_str())
                        || reference["fragmentId"].as_str() == Some(origin.as_str())
                        || reference["semanticSourceAliases"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|id| id == &origin)
                        || (matches!(field, "body" | "title")
                            && reference["workRef"].as_str() == origin.split('.').next()))
                {
                    return false;
                }
                let Some(start) = reference["start"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                else {
                    return false;
                };
                let Some(end) = reference["end"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                else {
                    return false;
                };
                if start >= end
                    || start < source.start
                    || end > source.end
                    || fragment.start >= end
                    || fragment.end <= start
                {
                    return false;
                }
                let text: String = source
                    .text
                    .chars()
                    .skip(start - source.start)
                    .take(end - start)
                    .collect();
                reference["textHash"].as_str() == Some(creator_discovery::hash(&text).as_str())
            })
        })
}

pub(crate) async fn attach_legacy_media_aliases(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    refs: &mut [Value],
) -> Result<(), sqlx::Error> {
    let jobs: Vec<Uuid> = refs
        .iter()
        .filter(|r| matches!(r["field"].as_str(), Some("ocr" | "transcript")))
        .filter_map(|r| r["sourceRef"].as_str()?.parse().ok())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    for jobs in jobs.chunks(500) {
        let rows = sqlx::query("SELECT job.job_ref,job.blob_sha256,job.slot_key,job.input_scope,job.processor_kind,origin.content_public_ref,derived.source_location FROM linggan_media_processing_job job JOIN linggan_material_media_origin origin USING(slot_key) JOIN linggan_media_derivative derivative USING(job_ref) JOIN linggan_material_derived_text derived USING(derivative_ref) WHERE job.job_ref=ANY($1)")
            .bind(jobs).fetch_all(&mut **tx).await?;
        for reference in refs.iter_mut() {
            let owner = reference["workRef"]
                .as_str()
                .or_else(|| reference["fragmentId"].as_str()?.split('.').next());
            let Some(owner) = owner.and_then(|owner| owner.parse::<Uuid>().ok()) else {
                continue;
            };
            let aliases: Vec<_> = rows
                .iter()
                .filter(|row| {
                    reference["sourceRef"] == json!(row.get::<Uuid, _>("job_ref"))
                        && owner == row.get::<Uuid, _>("content_public_ref")
                        && (reference["field"] == "transcript")
                            == (row.get::<String, _>("processor_kind") == "asr")
                })
                .map(|row| {
                    let semantic = creator_discovery::topic_media_source_identity(
                        &row.get::<String, _>("blob_sha256"),
                        &row.get::<String, _>("slot_key"),
                        &row.get::<String, _>("input_scope"),
                        row.get::<Option<Value>, _>("source_location")
                            .unwrap_or(json!({})),
                    );
                    format!(
                        "{owner}.{}.{}",
                        reference["field"].as_str().unwrap_or(""),
                        creator_discovery::hash(&semantic.to_string())
                    )
                })
                .collect();
            if !aliases.is_empty() {
                reference["semanticSourceAliases"] = json!(aliases);
            }
        }
    }
    Ok(())
}

pub(crate) async fn queue_research_run(
    db: &Database,
    domain: Uuid,
    request: Uuid,
    trigger: &str,
    works: &[Uuid],
    topic: Option<Uuid>,
) -> Result<Option<Uuid>, ResearchError> {
    queue_run(db, domain, request, trigger, works, topic, false).await
}

/// Automatic admission never grows the backlog while an earlier batch remains
/// unfinished, including batches waiting for a human or a budget reset.
pub(crate) async fn queue_automatic_run(
    db: &Database,
    domain: Uuid,
) -> Result<Option<Uuid>, ResearchError> {
    queue_run(db, domain, Uuid::new_v4(), "incremental", &[], None, true).await
}

async fn queue_run(
    db: &Database,
    domain: Uuid,
    request: Uuid,
    trigger: &str,
    works: &[Uuid],
    topic: Option<Uuid>,
    automatic: bool,
) -> Result<Option<Uuid>, ResearchError> {
    let p = sqlx::query("SELECT * FROM linggan_topic_map_research_policy WHERE domain_ref=$1")
        .bind(domain)
        .fetch_optional(db.pool())
        .await?
        .ok_or(ResearchError::Invalid("research_not_configured"))?;
    let config: Uuid = p.get("model_config_ref");
    let input = load_inputs(db, domain, config).await?;
    let scope = select_scope(db, domain, trigger, works, topic, &input).await?;
    if scope.is_empty() {
        return Ok(None);
    }
    // This reader acquires its own pooled connection, so call it before the
    // policy transaction. The comparison queue repeats the check under its lock.
    let current_comparison = if trigger == "on_demand" && !scope.is_empty() {
        crate::topic_map_core::comparison::has_current_comparison(
            db, &input, domain, config, &scope,
        )
        .await
        .map_err(|error| match error {
            crate::model_settings::ModelError::Database(error) => ResearchError::Database(error),
            _ => ResearchError::Invalid("comparison_source_unavailable"),
        })?
    } else {
        false
    };
    let windows = prepare_windows(&input, &scope, trigger);
    let mut tx = db.pool().begin().await?;
    let locked = sqlx::query(
        "SELECT status,automatic_enabled,model_config_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(domain)
    .fetch_one(&mut *tx)
    .await?;
    if automatic {
        let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_run WHERE domain_ref=$1 AND method_version=$2 AND state NOT IN ('completed','stopped','failed'))")
            .bind(domain).bind(METHOD_VERSION).fetch_one(&mut *tx).await?;
        if blocked
            || locked.get::<String, _>("status") != "active"
            || !locked.get::<bool, _>("automatic_enabled")
            || locked.get::<Uuid, _>("model_config_ref") != config
        {
            tx.commit().await?;
            return Ok(None);
        }
    }
    if let Some(run) = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_topic_map_research_run WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        return Ok(Some(run));
    }
    let (selected, remaining) =
        select_windows(&mut tx, domain, &scope, &input, windows, trigger).await?;
    let reassignment = if trigger == "on_demand" {
        crate::topic_map_core::backfill::authorize_scope(&mut tx, domain, request, &scope)
            .await
            .map_err(|error| match error {
                crate::model_settings::ModelError::Database(error) => {
                    ResearchError::Database(error)
                }
                _ => ResearchError::Invalid("reassignment_source_unavailable"),
            })?
    } else {
        None
    };
    if selected.is_empty() && (trigger != "on_demand" || current_comparison) {
        tx.commit().await?;
        return Ok(reassignment);
    }
    let run = Uuid::new_v4();
    let frozen_works: Vec<_> = if trigger == "on_demand" {
        scope.clone()
    } else {
        selected
            .iter()
            .map(|input| input.work.work_ref)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let input_scope = json!({"workRefs":frozen_works,"inputContract":INPUT_CONTRACT,"maxWindowChars":RESEARCH_WINDOW_CHARS});
    sqlx::query("INSERT INTO linggan_topic_map_research_run(run_ref,domain_ref,request_ref,trigger,topic_ref,config_ref,method_version,token_limit,state,last_reason,input_scope) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'queued',$9,$10)")
        .bind(run).bind(domain).bind(request).bind(trigger).bind(topic).bind(config).bind(METHOD_VERSION)
        .bind(p.get::<i64,_>("run_token_limit")).bind(if remaining{Some("additional_source_windows_pending")}else{None}).bind(input_scope)
        .execute(&mut *tx).await?;
    for input in selected {
        sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4()).bind(run).bind(domain).bind(input.work.work_ref).bind(&input.hash)
            .bind(reference_manifest(&input)).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Some(run))
}

async fn select_scope(
    db: &Database,
    domain: Uuid,
    trigger: &str,
    works: &[Uuid],
    topic: Option<Uuid>,
    input: &[ResearchInput],
) -> Result<Vec<Uuid>, ResearchError> {
    let scope: Vec<Uuid> = if trigger == "on_demand" {
        if !works.is_empty() {
            let unique: BTreeSet<_> = works.iter().copied().collect();
            if unique.len() != works.len() || works.len() > 10 {
                return Err(ResearchError::Invalid("invalid_comparison_scope"));
            }
            if works
                .iter()
                .any(|work| !input.iter().any(|i| i.work.work_ref == *work))
            {
                return Err(ResearchError::Invalid("research_source_unavailable"));
            }
            works.to_vec()
        } else if let Some(topic) = topic {
            let bound: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_binding b WHERE b.topic_ref=$1 AND b.domain_ref=$2 AND b.version=(SELECT max(version) FROM linggan_topic_map_binding WHERE topic_ref=$1))")
                .bind(topic).bind(domain).fetch_one(db.pool()).await?;
            if !bound {
                return Err(ResearchError::Invalid("topic_scope_unavailable"));
            }
            let members: Vec<Uuid> = sqlx::query_scalar("SELECT member.work_public_ref FROM linggan_topic_material_member member JOIN linggan_topic_classification_run run USING(classification_run_ref) JOIN linggan_topic_definition def USING(definition_ref) WHERE def.topic_ref=$1 AND def.version=(SELECT max(version) FROM linggan_topic_definition WHERE topic_ref=$1) ORDER BY member.ordinal LIMIT 10")
                .bind(topic).fetch_all(db.pool()).await?;
            members
                .into_iter()
                .filter(|work| input.iter().any(|i| i.work.work_ref == *work))
                .collect()
        } else {
            return Err(ResearchError::Invalid("on_demand_scope_required"));
        }
    } else {
        input
            .iter()
            .filter(|i| works.is_empty() || works.contains(&i.work.work_ref))
            .map(|i| i.work.work_ref)
            .collect()
    };
    Ok(scope)
}

fn prepare_windows(input: &[ResearchInput], scope: &[Uuid], trigger: &str) -> Vec<ResearchInput> {
    let mut windows = Vec::new();
    for input in input.iter().filter(|i| scope.contains(&i.work.work_ref)) {
        let mut input = input.clone();
        if trigger == "on_demand" {
            // These IDs authorize later comparison of distilled results; they do not
            // turn another author's text into this work's source or hash dependency.
            input.context_work_refs = scope
                .iter()
                .filter(|work| **work != input.work.work_ref)
                .copied()
                .collect();
        }
        windows.extend(research_windows(&input, RESEARCH_WINDOW_CHARS));
    }
    windows
}

async fn select_windows(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
    scope: &[Uuid],
    input: &[ResearchInput],
    windows: Vec<ResearchInput>,
    trigger: &str,
) -> Result<(Vec<ResearchInput>, bool), sqlx::Error> {
    let hashes: Vec<_> = windows.iter().map(|input| input.hash.clone()).collect();
    let mut history = BTreeMap::<String, Vec<String>>::new();
    for hashes in hashes.chunks(500) {
        let rows = sqlx::query("SELECT input_hash,state FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND input_hash=ANY($2)")
            .bind(domain).bind(hashes).fetch_all(&mut **tx).await?;
        for row in rows {
            history
                .entry(row.get("input_hash"))
                .or_default()
                .push(row.get("state"));
        }
    }
    let scope_strings: Vec<_> = scope.iter().map(Uuid::to_string).collect();
    let unknown = sqlx::query("SELECT input_refs FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND state='unknown_dispatch' AND (work_public_ref=ANY($2) OR (COALESCE(input_refs->'source',input_refs)->'contextWorkRefs') ?| $3 OR (COALESCE(input_refs->'source',input_refs)->'coverage'->'scopeWorkRefs') ?| $3)")
        .bind(domain).bind(&scope).bind(&scope_strings).fetch_all(&mut **tx).await?;
    let mut unknown_refs: Vec<Value> = unknown
        .into_iter()
        .flat_map(|row| {
            let manifest: Value = row.get("input_refs");
            manifest.get("source").unwrap_or(&manifest)["fragments"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .collect();
    attach_legacy_media_aliases(tx, &mut unknown_refs).await?;
    let mut selected = Vec::new();
    let mut remaining = false;
    for window in windows {
        if should_queue_input(
            history.get(&window.hash).map_or(&[], Vec::as_slice),
            trigger,
        ) && !input
            .iter()
            .find(|full| full.work.work_ref == window.work.work_ref)
            .is_some_and(|full| overlaps_unknown_dispatch(&window, full, &unknown_refs))
        {
            if trigger != "on_demand" && selected.len() == 100 {
                remaining = true;
                break;
            }
            selected.push(window);
        }
    }
    Ok((selected, remaining))
}
