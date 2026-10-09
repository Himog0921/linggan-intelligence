//! Temporary keyword search and frozen detail identities share the existing bounded round.
use crate::topic_map_research::ResearchError;
use linggan_evidence::{
    AcquisitionChainError, MaterialDeepeningTarget, missing_qualified_detail_refs_in,
    request_and_admit_keyword_search_for_domain_in_transaction,
    request_and_admit_material_targets_for_domain_in_transaction,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SearchCommand {
    StartSearch {
        request_ref: Uuid,
        domain_ref: Uuid,
        topic_ref: Option<Uuid>,
        authorization_ref: Uuid,
        keywords: Vec<String>,
        purpose: String,
        #[serde(default)]
        new_round: bool,
    },
    FreezeSearchDetails {
        request_ref: Uuid,
        domain_ref: Uuid,
        round_ref: Uuid,
        work_refs: Vec<Uuid>,
    },
    StopSearch {
        request_ref: Uuid,
        domain_ref: Uuid,
        round_ref: Uuid,
    },
}
impl SearchCommand {
    fn identity(&self) -> (Uuid, Uuid) {
        match self {
            Self::StartSearch {
                request_ref,
                domain_ref,
                ..
            }
            | Self::FreezeSearchDetails {
                request_ref,
                domain_ref,
                ..
            }
            | Self::StopSearch {
                request_ref,
                domain_ref,
                ..
            } => (*request_ref, *domain_ref),
        }
    }
}
fn collection_error(e: AcquisitionChainError) -> ResearchError {
    match e {
        AcquisitionChainError::Database(e) => ResearchError::Database(e),
        _ => ResearchError::Invalid("collection_admission_rejected"),
    }
}
async fn cancel_searches(
    tx: &mut Transaction<'_, Postgres>,
    round: Uuid,
) -> Result<(), ResearchError> {
    sqlx::query("UPDATE collection_work_order_lease SET released_at=COALESCE(released_at,scope_001_now()) WHERE work_order_ref IN(SELECT work_order_ref FROM linggan_topic_map_search_target WHERE round_ref=$1)").bind(round).execute(&mut **tx).await?;
    sqlx::query("UPDATE collection_work_order SET queue_state='cancelled' WHERE work_order_ref IN(SELECT work_order_ref FROM linggan_topic_map_search_target WHERE round_ref=$1) AND queue_state NOT IN('completed','cancelled')").bind(round).execute(&mut **tx).await?;
    Ok(())
}
async fn candidate_refs(
    tx: &mut Transaction<'_, Postgres>,
    round: Uuid,
) -> Result<Vec<Uuid>, ResearchError> {
    Ok(sqlx::query_scalar("SELECT DISTINCT finding.content_public_ref FROM linggan_topic_map_search_target search JOIN collection_work_order_lease lease ON lease.work_order_ref=search.work_order_ref JOIN collection_work_order_lease_task task USING(lease_ref) JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id JOIN linggan_runtime_submission_receipt receipt ON receipt.attempt_id=package.attempt_id JOIN linggan_material_discovery_finding finding ON finding.package_ref=package.package_ref JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=finding.package_ref AND disposition.record_ordinal=finding.record_ordinal WHERE search.round_ref=$1 AND receipt.material_admission='ACCEPTED' AND disposition.disposition='accepted_for_library_discovery' ORDER BY finding.content_public_ref LIMIT 200").bind(round).fetch_all(&mut **tx).await?)
}
pub async fn apply_search_command(
    db: &Database,
    command: &SearchCommand,
) -> Result<Value, ResearchError> {
    let (request, domain) = command.identity();
    let hash = linggan_evidence::creator_discovery::hash(
        &serde_json::to_string(command).map_err(|_| ResearchError::Invalid("invalid_command"))?,
    );
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,43))")
        .bind(domain.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(r) = sqlx::query(
        "SELECT command_hash,receipt FROM linggan_topic_map_research_command WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        return if r.get::<String, _>("command_hash") == hash {
            Ok(r.get("receipt"))
        } else {
            Err(ResearchError::Conflict)
        };
    }
    let receipt = match command {
        SearchCommand::StartSearch {
            topic_ref,
            authorization_ref,
            keywords,
            purpose,
            new_round,
            ..
        } => {
            let mut words: Vec<_> = keywords.iter().map(|s| s.trim().to_owned()).collect();
            words.sort();
            words.dedup();
            if words.is_empty()
                || words.len() > 3
                || words.len() != keywords.len()
                || words
                    .iter()
                    .any(|w| w.chars().count() > 80 || w.contains("::") || w.is_empty())
                || purpose.trim().is_empty()
                || purpose.chars().count() > 120
            {
                return Err(ResearchError::Invalid("invalid_search_scope"));
            }
            let enabled:Option<bool>=sqlx::query_scalar("SELECT p.collection_enabled AND p.status='active' AND d.status='active' FROM linggan_topic_map_research_policy p JOIN observation_domain d USING(domain_ref)WHERE p.domain_ref=$1 FOR UPDATE OF p").bind(domain).fetch_optional(&mut *tx).await?;
            if enabled != Some(true) {
                return Err(ResearchError::Invalid("collection_not_enabled"));
            }
            if let Some(topic) = topic_ref {
                let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=$1 ORDER BY version DESC LIMIT 1)b WHERE domain_ref=$2)").bind(topic).bind(domain).fetch_one(&mut *tx).await?;
                if !valid {
                    return Err(ResearchError::Invalid("topic_outside_domain"));
                }
            }
            let scope = linggan_evidence::creator_discovery::hash(
                &json!({"domain":domain,"topic":topic_ref,"keywords":words,"purpose":purpose})
                    .to_string(),
            );
            let previous=sqlx::query("SELECT round_ref,state FROM linggan_topic_map_collection_round WHERE domain_ref=$1 AND kind='search' AND scope_hash=$2 ORDER BY created_at DESC LIMIT 1 FOR UPDATE").bind(domain).bind(&scope).fetch_optional(&mut *tx).await?;
            if let Some(p) =
                previous.filter(|p| p.get::<String, _>("state") == "active" || !*new_round)
            {
                json!({"requestRef":request,"roundRef":p.get::<Uuid,_>("round_ref"),"state":p.get::<String,_>("state"),"reused":true})
            } else {
                let round = Uuid::new_v4();
                sqlx::query("INSERT INTO linggan_topic_map_collection_round(round_ref,domain_ref,topic_ref,scope_hash,purpose,kind,state,request_ref)VALUES($1,$2,$3,$4,$5,'search','active',$6)").bind(round).bind(domain).bind(topic_ref).bind(scope).bind(purpose).bind(request).execute(&mut *tx).await?;
                let count = words.len();
                let mut orders = Vec::new();
                for (index, word) in words.iter().enumerate() {
                    let target = Uuid::new_v4();
                    let quota = 200 / count + usize::from(index < 200 % count);
                    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source,purpose_kind,purpose_round_ref)VALUES($1,'xhs','keyword',$2,$3,'manual','research_round',$4)").bind(target).bind(format!("{word}::{round}")).bind(word).bind(round).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO observation_domain_target(domain_ref,target_ref,role)VALUES($1,$2,'primary')").bind(domain).bind(target).execute(&mut *tx).await?;
                    let outcome = request_and_admit_keyword_search_for_domain_in_transaction(
                        &mut tx,
                        target,
                        domain,
                        &format!("topic_map_search:{round}:{purpose}"),
                        "person",
                        *authorization_ref,
                    )
                    .await
                    .map_err(collection_error)?;
                    if let Some(order) = outcome.work_order_ref {
                        sqlx::query("UPDATE collection_work_order SET max_works=LEAST(max_works,$2),stop_conditions=stop_conditions||jsonb_build_object('maximumQuota',LEAST(max_works,$2),'topicMapSearch',jsonb_build_object('roundRef',$3::uuid,'scrollRounds',3,'maxDurationSeconds',180,'candidateQuota',LEAST(max_works,$2))) WHERE work_order_ref=$1 AND queue_state='queued' AND NOT EXISTS(SELECT 1 FROM collection_work_order_lease WHERE work_order_ref=$1)").bind(order).bind(quota as i32).bind(round).execute(&mut *tx).await?;
                        orders.push(order);
                    }
                    sqlx::query("INSERT INTO linggan_topic_map_search_target(round_ref,target_ref,keyword,ordinal,candidate_quota,authorization_ref,collection_request_ref,work_order_ref)VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(round).bind(target).bind(word).bind(index as i32+1).bind(quota as i32).bind(authorization_ref).bind(outcome.request_ref).bind(outcome.work_order_ref).execute(&mut *tx).await?;
                }
                if orders.is_empty() {
                    sqlx::query("UPDATE linggan_topic_map_collection_round SET state='blocked',last_reason='collection_admission_rejected' WHERE round_ref=$1").bind(round).execute(&mut *tx).await?;
                }
                json!({"requestRef":request,"roundRef":round,"state":if orders.is_empty(){"blocked"}else{"active"},"phase":"search","workOrderRefs":orders,"keywordCount":words.len(),"candidateLimit":200,"detailLimit":10,"scrollRoundsPerKeyword":3,"maxDurationSecondsPerKeyword":180,"temporary":true})
            }
        }
        SearchCommand::FreezeSearchDetails {
            round_ref,
            work_refs,
            ..
        } => {
            // Recheck the current opt-in before reserving any new detail slots.
            let enabled: Option<bool> = sqlx::query_scalar("SELECT p.collection_enabled AND p.status='active' AND d.status='active' FROM linggan_topic_map_research_policy p JOIN observation_domain d USING(domain_ref) WHERE p.domain_ref=$1 FOR UPDATE OF p,d").bind(domain).fetch_optional(&mut *tx).await?;
            if enabled != Some(true) {
                return Err(ResearchError::Invalid("collection_not_enabled"));
            }
            let row=sqlx::query("SELECT purpose,state FROM linggan_topic_map_collection_round WHERE round_ref=$1 AND domain_ref=$2 AND kind='search'FOR UPDATE").bind(round_ref).bind(domain).fetch_optional(&mut *tx).await?.ok_or(ResearchError::NotFound)?;
            if row.get::<String, _>("state") == "stopped" {
                return Err(ResearchError::Invalid("round_stopped"));
            }
            if let Some(frozen) = sqlx::query_scalar::<_, Vec<Uuid>>(
                "SELECT work_refs FROM linggan_topic_map_search_freeze WHERE round_ref=$1",
            )
            .bind(round_ref)
            .fetch_optional(&mut *tx)
            .await?
            {
                if frozen
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    != work_refs.iter().copied().collect()
                {
                    return Err(ResearchError::Conflict);
                }
                json!({"requestRef":request,"roundRef":round_ref,"phase":"details","workRefs":frozen,"reused":true})
            } else {
                let set: std::collections::HashSet<_> = work_refs.iter().copied().collect();
                if set.len() != work_refs.len() || set.is_empty() || set.len() > 10 {
                    return Err(ResearchError::Invalid(
                        "detail_freeze_requires_1_to_10_unique_works",
                    ));
                }
                let candidates = candidate_refs(&mut tx, *round_ref).await?;
                if work_refs.iter().any(|r| !candidates.contains(r)) {
                    return Err(ResearchError::Invalid(
                        "work_not_in_accepted_search_results",
                    ));
                }
                let missing = missing_qualified_detail_refs_in(&mut tx, work_refs).await?;
                if missing.is_empty() {
                    return Err(ResearchError::Invalid("selected_details_already_available"));
                }
                // The entire exact scope is frozen once. Existing details are reused and consume no new slot.
                sqlx::query("INSERT INTO linggan_topic_map_search_freeze(round_ref,request_ref,work_refs)VALUES($1,$2,$3)").bind(round_ref).bind(request).bind(work_refs).execute(&mut *tx).await?;
                cancel_searches(&mut tx, *round_ref).await?;
                let target:Uuid=sqlx::query_scalar("SELECT target_ref FROM linggan_topic_map_search_target WHERE round_ref=$1 ORDER BY ordinal LIMIT 1").bind(round_ref).fetch_one(&mut *tx).await?;
                for reference in &missing {
                    sqlx::query("INSERT INTO linggan_topic_map_collection_slot(round_ref,work_public_ref,state)VALUES($1,$2,'reserved')").bind(round_ref).bind(reference).execute(&mut *tx).await?;
                }
                let targets: Vec<_> = missing
                    .iter()
                    .map(|r| MaterialDeepeningTarget {
                        content_public_ref: *r,
                        comment_limit: 30,
                        reply_expand_limit: 2,
                        acquire_media: true,
                        allow_ocr: false,
                        allow_asr: false,
                    })
                    .collect();
                let purpose: String = row.get("purpose");
                let admitted = request_and_admit_material_targets_for_domain_in_transaction(
                    &mut tx,
                    target,
                    domain,
                    &format!("topic_map:{round_ref}:{purpose}"),
                    "person",
                    &targets,
                )
                .await
                .map_err(collection_error)?;
                sqlx::query("UPDATE linggan_topic_map_collection_slot SET collection_request_ref=$2,work_order_ref=$3,state=$4 WHERE round_ref=$1").bind(round_ref).bind(admitted.request_ref).bind(admitted.work_order_ref).bind(if admitted.work_order_ref.is_some(){"admitted"}else{"blocked"}).execute(&mut *tx).await?;
                if admitted.work_order_ref.is_some() {
                    sqlx::query("UPDATE linggan_topic_map_collection_round SET state='active',last_reason=NULL WHERE round_ref=$1").bind(round_ref).execute(&mut *tx).await?;
                }
                if admitted.work_order_ref.is_none() {
                    sqlx::query("UPDATE linggan_topic_map_collection_round SET state='blocked',last_reason=$2 WHERE round_ref=$1").bind(round_ref).bind(admitted.reason_code).execute(&mut *tx).await?;
                }
                json!({"requestRef":request,"roundRef":round_ref,"phase":"details","state":if admitted.work_order_ref.is_some(){"active"}else{"blocked"},"workRefs":missing,"workOrderRef":admitted.work_order_ref,"reason":admitted.reason_code,"detailSlotsReserved":targets.len(),"detailLimit":10,"searchStopped":true})
            }
        }
        SearchCommand::StopSearch { round_ref, .. } => {
            let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_collection_round WHERE round_ref=$1 AND domain_ref=$2 AND kind='search')").bind(round_ref).bind(domain).fetch_one(&mut *tx).await?;
            if !exists {
                return Err(ResearchError::NotFound);
            }
            cancel_searches(&mut tx, *round_ref).await?;
            sqlx::query("UPDATE collection_work_order SET queue_state='cancelled'WHERE work_order_ref IN(SELECT work_order_ref FROM linggan_topic_map_collection_slot WHERE round_ref=$1)AND queue_state NOT IN('completed','cancelled')").bind(round_ref).execute(&mut *tx).await?;
            sqlx::query("UPDATE collection_work_order_lease SET released_at=COALESCE(released_at,scope_001_now())WHERE work_order_ref IN(SELECT work_order_ref FROM linggan_topic_map_collection_slot WHERE round_ref=$1)").bind(round_ref).execute(&mut *tx).await?;
            sqlx::query("UPDATE linggan_topic_map_collection_round SET state='stopped',last_reason='user_stopped'WHERE round_ref=$1").bind(round_ref).execute(&mut *tx).await?;
            json!({"requestRef":request,"roundRef":round_ref,"state":"stopped","remoteCallsAlreadySentMayComplete":true})
        }
    };
    sqlx::query("INSERT INTO linggan_topic_map_research_command(request_ref,domain_ref,command_hash,receipt)VALUES($1,$2,$3,$4)").bind(request).bind(domain).bind(hash).bind(&receipt).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(receipt)
}
pub async fn read_search_round_progress(
    db: &Database,
    domain: Uuid,
    round: Uuid,
) -> Result<Value, ResearchError> {
    let mut tx = db.pool().begin().await?;
    let state:Option<String>=sqlx::query_scalar("SELECT state FROM linggan_topic_map_collection_round WHERE round_ref=$1 AND domain_ref=$2 AND kind='search'").bind(round).bind(domain).fetch_optional(&mut *tx).await?;
    let state = state.ok_or(ResearchError::NotFound)?;
    let candidates = candidate_refs(&mut tx, round).await?;
    let slots:Vec<Uuid>=sqlx::query_scalar("SELECT work_public_ref FROM linggan_topic_map_collection_slot WHERE round_ref=$1 ORDER BY work_public_ref").bind(round).fetch_all(&mut *tx).await?;
    let missing = missing_qualified_detail_refs_in(&mut tx, &slots).await?;
    let searches:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('keyword',search.keyword,'candidateQuota',search.candidate_quota,'workOrderRef',search.work_order_ref,'queueState',work.queue_state,'temporary',true) FROM linggan_topic_map_search_target search LEFT JOIN collection_work_order work USING(work_order_ref) WHERE round_ref=$1 ORDER BY ordinal").bind(round).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"roundRef":round,"domainRef":domain,"state":state,"phase":if slots.is_empty(){"search"}else{"details"},"searches":searches,"candidateWorkRefs":candidates,"candidateCount":candidates.len(),"detailSlotsReserved":slots.len(),"actualDetailsAcquired":slots.len()-missing.len(),"pendingDetailWorkRefs":missing,"candidateLimit":200,"detailLimit":10,"emptySearchIsNotMarketAbsence":true}),
    )
}

pub async fn read_search_progress(db: &Database, domain: Uuid) -> Result<Value, ResearchError> {
    let grants:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('authorizationRef',authorization_ref,'purpose',purpose,'maxTargets',max_targets,'maxWorksPerTarget',max_works_per_target,'expiresAt',expires_at::text,'allowedTaskTemplates',allowed_task_templates,'allowedDispatchLanes',allowed_dispatch_lanes) FROM collection_acquisition_authorization WHERE platform='xhs'AND target_kind='keyword'AND lane='deep_archive'AND revoked_at IS NULL AND expires_at>scope_001_now() AND 'keyword_archive'=ANY(allowed_task_templates) AND 'immediate'=ANY(allowed_dispatch_lanes) ORDER BY expires_at DESC").fetch_all(db.pool()).await?;
    let refs:Vec<Uuid>=sqlx::query_scalar("SELECT round_ref FROM linggan_topic_map_collection_round WHERE domain_ref=$1 AND kind='search' ORDER BY created_at DESC LIMIT 30").bind(domain).fetch_all(db.pool()).await?;
    let mut rounds = Vec::new();
    for round in refs {
        rounds.push(read_search_round_progress(db, domain, round).await?)
    }
    Ok(
        json!({"domainRef":domain,"authorizations":grants,"rounds":rounds,"roundReadLimit":30,"candidateLimit":200,"keywordLimit":3,"detailLimit":10,"ordinaryOpenStartsSearch":false}),
    )
}
/// State maintenance alone; only explicit commands admit platform work.
pub async fn advance_search_rounds_once(db: &Database) -> Result<(), ResearchError> {
    let ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_search_target')IS NOT NULL")
            .fetch_one(db.pool())
            .await?;
    if !ready {
        return Ok(());
    }
    let refs:Vec<(Uuid,Uuid,bool)>=sqlx::query_as("SELECT round_ref,domain_ref,created_at<scope_001_now()-interval '15 minutes' FROM linggan_topic_map_collection_round WHERE kind='search' AND state='active' ORDER BY created_at LIMIT 32").fetch_all(db.pool()).await?;
    for (round, domain, expired) in refs {
        let progress = read_search_round_progress(db, domain, round).await?;
        let slots = progress["detailSlotsReserved"].as_u64().unwrap_or(0);
        let mut tx = db.pool().begin().await?;
        if slots > 0 {
            let pending = progress["pendingDetailWorkRefs"]
                .as_array()
                .map_or(0, Vec::len);
            let live:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_collection_slot slot JOIN collection_work_order work USING(work_order_ref)WHERE slot.round_ref=$1 AND work.queue_state IN('queued','leased'))").bind(round).fetch_one(&mut *tx).await?;
            if pending == 0 || !live {
                sqlx::query("UPDATE linggan_topic_map_collection_round SET state=$2,last_reason=$3 WHERE round_ref=$1 AND state='active'").bind(round).bind(if pending==0{"completed"}else{"partial"}).bind(if pending==0{"details_accepted"}else{"partial_details_or_admission_stopped"}).execute(&mut *tx).await?;
            }
        } else {
            let live = progress["searches"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|s| matches!(s["queueState"].as_str(), Some("queued" | "leased")))
            });
            if !live || expired {
                if expired {
                    cancel_searches(&mut tx, round).await?;
                }
                let count = progress["candidateCount"].as_u64().unwrap_or(0);
                sqlx::query("UPDATE linggan_topic_map_collection_round SET state=$2,last_reason=$3 WHERE round_ref=$1 AND state='active'").bind(round).bind(if count>0{"partial"}else{"completed"}).bind(if expired{"time_budget"}else if count==0{"search_finished_no_accepted_candidates"}else{"search_ready_for_detail_selection"}).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
    }
    Ok(())
}
