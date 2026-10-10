//! Atomic acceptance, immutable result append, and phase recovery.
use super::preparation::catalog_fingerprint;
use super::*;
use sqlx::{Postgres, Transaction};

pub(super) struct Completion<'a> {
    pub invocation: Uuid,
    pub task: Uuid,
    pub run: Uuid,
    pub lease: Uuid,
    pub input: &'a ResearchInput,
    pub response: Option<&'a PiResponse>,
    pub extracted: Option<ResearchOutput>,
    pub resolved: Option<ResolutionOutput>,
    pub rejection: Option<&'static str>,
    pub prepared: &'a Prepared,
    pub source_current: bool,
    pub candidate_sources_current: bool,
    pub eligible_definitions: Vec<Uuid>,
}
struct AcceptedPhase {
    state: &'static str,
    reason: Option<&'static str>,
    output: Option<ResearchOutput>,
    units: Vec<Value>,
}

pub(super) async fn settle(db: &Database, mut c: Completion<'_>) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    // Stop/pause and settlement serialize on policy and run before task ownership.
    let enabled:bool=sqlx::query_scalar("SELECT p.status='active' AND d.status='active' AND r.state='running' FROM linggan_topic_map_research_run r JOIN linggan_topic_map_research_policy p USING(domain_ref) JOIN observation_domain d USING(domain_ref)WHERE r.run_ref=$1 FOR UPDATE OF p,r")
        .bind(c.run).fetch_one(&mut *tx).await?;
    let owner = sqlx::query("SELECT t.*,r.config_ref FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref) WHERE t.task_ref=$1 AND t.lease_token=$2 AND t.state='running' FOR UPDATE OF t")
        .bind(c.task).bind(c.lease).fetch_optional(&mut *tx).await?;
    if owner.is_none() || !enabled {
        let unknown = transport_unknown(c.response);
        finish_invocation_in(
            &mut tx,
            c.invocation,
            c.response,
            false,
            Some(if unknown {
                "unknown_dispatch"
            } else {
                "late_result_rejected"
            }),
            &json!({"accepted":false,"dispatchUnknown":unknown}),
        )
        .await?;
        if owner.is_some() {
            sqlx::query("UPDATE linggan_topic_map_research_task SET state=CASE WHEN $2 THEN 'unknown_dispatch' ELSE 'queued' END,phase_attempt_count=CASE WHEN $2 THEN phase_attempt_count ELSE 0 END,lease_token=NULL,lease_expires_at=NULL,last_reason=CASE WHEN $2 THEN 'unknown_dispatch' ELSE 'paused_before_acceptance' END WHERE task_ref=$1")
                .bind(c.task).bind(unknown).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        return Ok(());
    }
    if let Some(owner) = owner {
        let (current, unavailable) = super::dispatch::source_snapshot(db, &owner).await?;
        c.source_current = current;
        c.candidate_sources_current =
            super::dispatch::candidate_sources_current(c.prepared, &unavailable);
        c.eligible_definitions
            .retain(|id| !unavailable.contains(id));
    }
    let accepted = accept_phase(&mut tx, &mut c).await?;
    if let Some(output) = &accepted.output {
        append_result(&mut tx, &c, output, &accepted.units).await?;
    }
    finish_invocation_in(&mut tx,c.invocation,c.response,accepted.reason.is_none(),accepted.reason,
        &json!({"accepted":accepted.reason.is_none(),"dispatchUnknown":accepted.state=="unknown_dispatch","phaseComplete":accepted.state!="failed" && accepted.state!="unknown_dispatch"})).await?;
    sqlx::query("UPDATE linggan_topic_map_research_task SET state=$3,last_reason=$4,lease_token=NULL,lease_expires_at=NULL,updated_at=scope_001_now()WHERE task_ref=$1 AND lease_token=$2")
        .bind(c.task).bind(c.lease).bind(accepted.state).bind(accepted.reason).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn accept_phase(
    tx: &mut Transaction<'_, Postgres>,
    c: &mut Completion<'_>,
) -> Result<AcceptedPhase, ModelError> {
    let mut accepted = AcceptedPhase {
        state: "failed",
        reason: Some(c.rejection.unwrap_or("invalid_output")),
        output: None,
        units: c.prepared.previous.clone(),
    };
    if let Some((state, reason)) = pre_acceptance_failure(c.response, c.source_current) {
        accepted.state = state;
        accepted.reason = Some(reason);
        return Ok(accepted);
    }
    if let Some(output) = c.extracted.take() {
        accepted.reason = None;
        if c.input.coverage["kind"] == "comparison" || core::units(c.input, &output).is_empty() {
            accepted.state = match output.outcome.as_str() {
                "no_signal" => "no_signal",
                "insufficient" => "insufficient",
                _ => "succeeded",
            };
            accepted.output = Some(output);
        } else {
            accepted.state = "queued";
            sqlx::query("UPDATE linggan_topic_map_research_task SET phase='resolve',phase_attempt_count=0,distilled_json=$2,resolution_cursor=0,resolutions_json='[]'::jsonb WHERE task_ref=$1")
                .bind(c.task).bind(serde_json::to_value(output).map_err(|_|ModelError::InvalidOutput)?).execute(&mut **tx).await?;
        }
    } else if let Some(resolution) = c.resolved.take() {
        return accept_resolution(tx, c, resolution).await;
    } else if c.response.is_some_and(|r| {
        !r.ok
            && r.diagnostic
                .as_ref()
                .is_some_and(|d| d.stage == "request_not_started")
    }) {
        let remains:bool=sqlx::query_scalar("SELECT t.phase_attempt_count<config.max_attempts FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref)JOIN linggan_model_config config ON config.config_ref=r.config_ref WHERE task_ref=$1")
            .bind(c.task).fetch_one(&mut **tx).await?;
        accepted.state = if remains { "queued" } else { "failed" };
        accepted.reason = Some("request_not_started");
    }
    Ok(accepted)
}

fn transport_unknown(response: Option<&PiResponse>) -> bool {
    response.is_none()
        || response.is_some_and(|response| {
            !response.ok
                && response.diagnostic.as_ref().is_none_or(|diagnostic| {
                    diagnostic.response_started.is_none()
                        && diagnostic.stage != "request_not_started"
                })
        })
}

fn pre_acceptance_failure(
    response: Option<&PiResponse>,
    source_current: bool,
) -> Option<(&'static str, &'static str)> {
    // Source withdrawal denies content use, but cannot erase uncertainty about
    // a request already transmitted. Keep that fact available to replay guards.
    if transport_unknown(response) {
        Some(("unknown_dispatch", "unknown_dispatch"))
    } else if !source_current {
        Some(("stale", "source_changed"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transmitted_unknown_survives_source_withdrawal_and_pause() {
        for current in [false, true] {
            assert_eq!(
                pre_acceptance_failure(None, current),
                Some(("unknown_dispatch", "unknown_dispatch"))
            );
        }
        // The disabled/late-result branch uses the identical transport fact.
        assert!(transport_unknown(None));
    }
}

async fn accept_resolution(
    tx: &mut Transaction<'_, Postgres>,
    c: &Completion<'_>,
    resolution: ResolutionOutput,
) -> Result<AcceptedPhase, ModelError> {
    let domain = c.input.domain["domainRef"]
        .as_str()
        .and_then(|s| s.parse::<Uuid>().ok())
        .ok_or(ModelError::Source)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,120))")
        .bind(format!("topic-core:{domain}"))
        .execute(&mut **tx)
        .await?;
    let current = core::catalog_in(tx, domain).await?;
    let compared_current = c
        .prepared
        .candidates
        .as_array()
        .into_iter()
        .flatten()
        .all(|old| {
            current
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| t["definitionRef"] == old["definitionRef"])
        });
    let new_catalog = resolution.decisions.iter().any(|d| d.status == "new")
        && c.prepared.recall["catalogFingerprint"] != catalog_fingerprint(&current);
    if !compared_current || new_catalog || !c.candidate_sources_current {
        sqlx::query(
            "UPDATE linggan_topic_map_research_task SET phase_attempt_count=0 WHERE task_ref=$1",
        )
        .bind(c.task)
        .execute(&mut **tx)
        .await?;
        return Ok(AcceptedPhase {
            state: "queued",
            reason: Some("definition_changed_recalling"),
            output: None,
            units: c.prepared.previous.clone(),
        });
    }
    let mut units = c.prepared.previous.clone();
    units.extend(
        core::accept_in(
            tx,
            core::Acceptance {
                input: c.input,
                task: c.task,
                invocation: c.invocation,
                units: &c.prepared.units,
                resolution: &resolution,
                candidates: &c.prepared.candidates,
                recall: &c.prepared.recall,
                eligible_definitions: &c.eligible_definitions,
            },
        )
        .await?,
    );
    let next = c.prepared.cursor + c.prepared.units.len();
    let draft = c.prepared.draft.as_ref().ok_or(ModelError::InvalidOutput)?;
    let complete = next >= c.prepared.total_units;
    sqlx::query("UPDATE linggan_topic_map_research_task SET resolution_cursor=$2,resolutions_json=$3,recall_manifest=$4,phase_attempt_count=0 WHERE task_ref=$1")
        .bind(c.task).bind(next as i32).bind(json!(units)).bind(&c.prepared.recall).execute(&mut **tx).await?;
    Ok(AcceptedPhase {
        state: if complete { "succeeded" } else { "queued" },
        reason: None,
        output: complete.then(|| draft.clone()),
        units,
    })
}

async fn append_result(
    tx: &mut Transaction<'_, Postgres>,
    c: &Completion<'_>,
    output: &ResearchOutput,
    units: &[Value],
) -> Result<(), ModelError> {
    sqlx::query("INSERT INTO linggan_topic_map_research_result(result_ref,domain_ref,work_public_ref,input_hash,output_json,method_version,invocation_ref,core_json)SELECT $1,domain_ref,work_public_ref,input_hash,$3,$4,$5,$6 FROM linggan_topic_map_research_task WHERE task_ref=$2")
        .bind(Uuid::new_v4()).bind(c.task).bind(serde_json::to_value(output).map_err(|_|ModelError::InvalidOutput)?)
        .bind(analysis::METHOD_VERSION).bind(c.invocation).bind(json!({"methodVersion":core::METHOD_VERSION,"coverage":c.input.coverage,"units":units})).execute(&mut **tx).await?;
    if c.input.coverage["kind"] != "comparison"
        && !output.journey.evidence.is_empty()
        && output.journey.evidence.iter().all(|citation| {
            c.input.fragments.iter().any(|f| {
                f.fragment_id == citation.fragment_id
                    && ["title", "body", "ocr", "transcript"].contains(&f.field.as_str())
            })
        })
    {
        append_annotation(tx, c.invocation, c.input, output).await?;
    }
    Ok(())
}
async fn append_annotation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    invocation: Uuid,
    input: &ResearchInput,
    output: &ResearchOutput,
) -> Result<(), ModelError> {
    let domain: Uuid = sqlx::query_scalar(
        "SELECT domain_ref FROM linggan_topic_map_research_result WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(&mut **tx)
    .await?;
    let citations:Vec<_>=output.journey.evidence.iter().filter_map(|c|input.fragments.iter().find(|f|f.fragment_id==c.fragment_id).map(|f|json!({"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"field":f.field,"start":c.start,"end":c.end}))).collect();
    let request = crate::topic_map::TopicMapAnnotationRequest {
        idempotency_key: format!("topic-map-invocation:{invocation}"),
        domain_ref: domain,
        work_public_ref: input.work.work_ref,
        topic_ref: None,
        definition_ref: None,
        method_version: analysis::METHOD_VERSION.into(),
        main_stage: output.journey.main_stage.clone(),
        involved_stages: output.journey.involved_stages.clone(),
        overlays: output.journey.overlays.clone(),
        path: output.journey.path.clone(),
        rationale: output.journey.rationale.clone(),
        evidence_citations: citations
            .iter()
            .map(|v| crate::topic_map::TopicMapCitation {
                fragment_id: v["fragmentId"].as_str().unwrap_or("").into(),
                source_ref: v["sourceRef"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(Uuid::nil()),
                field: v["field"].as_str().unwrap_or("").into(),
            })
            .collect(),
    };
    crate::topic_map::append_work_annotation_in(tx, &request)
        .await
        .map_err(|e| match e {
            crate::topic_map::TopicMapError::Database(e) => ModelError::Database(e),
            _ => ModelError::InvalidOutput,
        })?;
    Ok(())
}
