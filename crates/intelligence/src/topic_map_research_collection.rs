//! Atomic bounded rounds, using ordinary Collection authorization and WorkOrder admission.
use super::topic_map_research::ResearchError;
use linggan_evidence::{
    MaterialDeepeningTarget, request_and_admit_material_targets_for_domain_in_transaction,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CollectionCommand {
    Configure {
        request_ref: Uuid,
        domain_ref: Uuid,
        expected_revision: i64,
        collection_enabled: bool,
    },
    StartCollection {
        request_ref: Uuid,
        domain_ref: Uuid,
        topic_ref: Option<Uuid>,
        target_ref: Uuid,
        work_refs: Vec<Uuid>,
        purpose: String,
        #[serde(default)]
        new_round: bool,
    },
    DeepenComments {
        request_ref: Uuid,
        domain_ref: Uuid,
        topic_ref: Option<Uuid>,
        target_ref: Uuid,
        work_ref: Uuid,
        purpose: String,
        #[serde(default)]
        new_round: bool,
    },
    StopCollection {
        request_ref: Uuid,
        domain_ref: Uuid,
        round_ref: Uuid,
    },
}
impl CollectionCommand {
    fn identity(&self) -> (Uuid, Uuid) {
        match self {
            Self::Configure {
                request_ref,
                domain_ref,
                ..
            } => (*request_ref, *domain_ref),
            Self::StartCollection {
                request_ref,
                domain_ref,
                ..
            }
            | Self::DeepenComments {
                request_ref,
                domain_ref,
                ..
            }
            | Self::StopCollection {
                request_ref,
                domain_ref,
                ..
            } => (*request_ref, *domain_ref),
        }
    }
}
pub async fn apply_collection_command(
    db: &Database,
    command: &CollectionCommand,
) -> Result<Value, ResearchError> {
    let (request, domain) = command.identity();
    let hash = linggan_evidence::creator_discovery::hash(
        &serde_json::to_string(command)
            .map_err(|_| ResearchError::Invalid("invalid_collection_command"))?,
    );
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,43))")
        .bind(domain.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(r) = sqlx::query(
        "SELECT command_hash,receipt FROM linggan_topic_collection_command WHERE request_ref=$1",
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
    let receipt = if let CollectionCommand::Configure {
        expected_revision,
        collection_enabled,
        ..
    } = command
    {
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active')").bind(domain).fetch_one(&mut *tx).await?;
        if !active || *expected_revision < 0 {
            return Err(ResearchError::Invalid("domain_paused_or_missing"));
        }
        let revision: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM linggan_topic_collection_policy WHERE domain_ref=$1 FOR UPDATE",
        )
        .bind(domain)
        .fetch_optional(&mut *tx)
        .await?;
        if revision.unwrap_or(0) != *expected_revision {
            return Err(ResearchError::Conflict);
        }
        sqlx::query("INSERT INTO linggan_topic_collection_policy(domain_ref,collection_enabled,revision) VALUES($1,$2,$3) ON CONFLICT(domain_ref) DO UPDATE SET collection_enabled=EXCLUDED.collection_enabled,status='active',revision=EXCLUDED.revision,updated_at=scope_001_now()").bind(domain).bind(collection_enabled).bind(expected_revision+1).execute(&mut *tx).await?;
        json!({"requestRef":request,"domainRef":domain,"state":"configured","collectionEnabled":collection_enabled,"revision":expected_revision+1})
    } else if let CollectionCommand::StopCollection { round_ref, .. } = command {
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_collection_round WHERE round_ref=$1 AND domain_ref=$2)").bind(round_ref).bind(domain).fetch_one(&mut *tx).await?;
        if !exists {
            return Err(ResearchError::NotFound);
        }
        sqlx::query("UPDATE linggan_topic_map_collection_round SET state='stopped',last_reason='user_stopped' WHERE round_ref=$1 AND state='active'").bind(round_ref).execute(&mut *tx).await?;
        sqlx::query("UPDATE collection_work_order_lease SET released_at=COALESCE(released_at,scope_001_now()) WHERE work_order_ref IN(SELECT work_order_ref FROM linggan_topic_map_collection_slot WHERE round_ref=$1)").bind(round_ref).execute(&mut *tx).await?;
        sqlx::query("UPDATE collection_work_order SET queue_state='cancelled' WHERE work_order_ref IN(SELECT work_order_ref FROM linggan_topic_map_collection_slot WHERE round_ref=$1)AND queue_state NOT IN('completed','cancelled')").bind(round_ref).execute(&mut *tx).await?;
        json!({"requestRef":request,"roundRef":round_ref,"state":"stopped","reason":"user_stopped"})
    } else {
        let (topic, target, works, purpose, new_round, kind) = match command {
            CollectionCommand::StartCollection {
                topic_ref,
                target_ref,
                work_refs,
                purpose,
                new_round,
                ..
            } => (
                *topic_ref,
                *target_ref,
                work_refs.clone(),
                purpose,
                *new_round,
                "details",
            ),
            CollectionCommand::DeepenComments {
                topic_ref,
                target_ref,
                work_ref,
                purpose,
                new_round,
                ..
            } => (
                *topic_ref,
                *target_ref,
                vec![*work_ref],
                purpose,
                *new_round,
                "comments",
            ),
            _ => unreachable!(),
        };
        if works.is_empty()
            || works.len() > 10
            || purpose.trim().is_empty()
            || purpose.chars().count() > 120
        {
            return Err(ResearchError::Invalid("invalid_collection_scope"));
        }
        let unique: std::collections::HashSet<_> = works.iter().collect();
        if unique.len() != works.len() {
            return Err(ResearchError::Invalid("duplicate_collection_work"));
        }
        let policy:Option<bool>=sqlx::query_scalar("SELECT p.collection_enabled AND p.status='active'AND d.status='active'FROM linggan_topic_collection_policy p JOIN observation_domain d USING(domain_ref)WHERE p.domain_ref=$1 FOR UPDATE OF p").bind(domain).fetch_optional(&mut *tx).await?;
        if policy != Some(true) {
            return Err(ResearchError::Invalid("collection_not_enabled"));
        }
        let mut ordered = works.clone();
        ordered.sort();
        let scope_hash=linggan_evidence::creator_discovery::hash(&json!({"domain":domain,"topic":topic,"target":target,"works":ordered,"purpose":purpose,"kind":kind}).to_string());
        let existing=sqlx::query("SELECT round_ref,state,last_reason FROM linggan_topic_map_collection_round WHERE domain_ref=$1 AND scope_hash=$2 AND kind=$3 ORDER BY created_at DESC LIMIT 1 FOR UPDATE").bind(domain).bind(&scope_hash).bind(kind).fetch_optional(&mut *tx).await?;
        if let Some(e) = existing.filter(|e| e.get::<String, _>("state") == "active" || !new_round)
        {
            json!({"requestRef":request,"roundRef":e.get::<Uuid,_>("round_ref"),"state":e.get::<String,_>("state"),"reason":e.get::<Option<String>,_>("last_reason"),"reused":true})
        } else {
            let round = Uuid::new_v4();
            sqlx::query("INSERT INTO linggan_topic_map_collection_round(round_ref,domain_ref,topic_ref,scope_hash,purpose,kind,state,request_ref)VALUES($1,$2,$3,$4,$5,$6,'active',$7)").bind(round).bind(domain).bind(topic).bind(&scope_hash).bind(purpose).bind(kind).bind(request).execute(&mut *tx).await?;
            let targets: Vec<MaterialDeepeningTarget> = works
                .iter()
                .map(|work| MaterialDeepeningTarget {
                    content_public_ref: *work,
                    comment_limit: 30,
                    reply_expand_limit: 2,
                    acquire_media: false,
                    allow_ocr: false,
                    allow_asr: false,
                })
                .collect();
            for work in &works {
                sqlx::query("INSERT INTO linggan_topic_map_collection_slot(round_ref,work_public_ref,state)VALUES($1,$2,'reserved')").bind(round).bind(work).execute(&mut *tx).await?;
            }
            let frozen_purpose = format!("topic_map:{round}:{purpose}");
            let outcome = request_and_admit_material_targets_for_domain_in_transaction(
                &mut tx,
                target,
                domain,
                &frozen_purpose,
                "person",
                &targets,
            )
            .await
            .map_err(|e| match e {
                linggan_evidence::AcquisitionChainError::Database(e) => ResearchError::Database(e),
                _ => ResearchError::Invalid("collection_admission_rejected"),
            })?;
            let admitted = outcome.work_order_ref.is_some();
            sqlx::query("UPDATE linggan_topic_map_collection_slot SET collection_request_ref=$2,work_order_ref=$3,state=$4 WHERE round_ref=$1").bind(round).bind(outcome.request_ref).bind(outcome.work_order_ref).bind(if admitted{"admitted"}else{"blocked"}).execute(&mut *tx).await?;
            if !admitted {
                sqlx::query("UPDATE linggan_topic_map_collection_round SET state='blocked',last_reason=$2 WHERE round_ref=$1").bind(round).bind(outcome.reason_code).execute(&mut *tx).await?;
            }
            if kind == "comments" {
                if let Some(order) = outcome.work_order_ref {
                    for work in &works {
                        let known:Vec<String>=sqlx::query_scalar("SELECT DISTINCT comment_external_id FROM linggan_material_comment WHERE content_public_ref=$1 ORDER BY comment_external_id LIMIT 5001").bind(work).fetch_all(&mut *tx).await?;
                        if known.len() > 5000 {
                            return Err(ResearchError::Invalid("comment_identity_snapshot_limit"));
                        }
                        sqlx::query("INSERT INTO linggan_topic_map_comment_budget(work_order_ref,content_public_ref,known_comment_ids,new_unique_limit,max_scroll_rounds,max_duration_seconds)VALUES($1,$2,$3,30,20,180)").bind(order).bind(work).bind(json!(known)).execute(&mut *tx).await?;
                    }
                }
            }
            json!({"requestRef":request,"roundRef":round,"collectionRequestRef":outcome.request_ref,"workOrderRef":outcome.work_order_ref,"state":if admitted{"active"}else{"blocked"},"reason":outcome.reason_code,"detailSlotsReserved":works.len(),"newCommentsLimit":if kind=="comments"{Some(30)}else{None},"detailLimit":10,"actualAcquired":0})
        }
    };
    sqlx::query("INSERT INTO linggan_topic_collection_command(request_ref,domain_ref,command_hash,receipt)VALUES($1,$2,$3,$4)").bind(request).bind(domain).bind(hash).bind(&receipt).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(receipt)
}

/// Collection receipts have their own read model, independent of AI stage configuration.
pub async fn read_collection_progress(db: &Database, domain: Uuid) -> Result<Value, ResearchError> {
    let policy:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('enabled',collection_enabled AND status='active','revision',revision) FROM linggan_topic_collection_policy WHERE domain_ref=$1").bind(domain).fetch_optional(db.pool()).await?;
    let targets=sqlx::query("SELECT t.target_ref,t.target_kind,COALESCE(t.display_name,t.identity_key) AS display_name FROM collection_observation_target t JOIN observation_domain_target membership USING(target_ref) WHERE membership.domain_ref=$1 AND t.lifecycle_state<>'dismissed' ORDER BY t.target_ref LIMIT 100").bind(domain).fetch_all(db.pool()).await?;
    let rounds=sqlx::query("SELECT r.round_ref,r.kind,r.state,r.last_reason,(SELECT count(*) FROM linggan_topic_map_collection_slot slot WHERE slot.round_ref=r.round_ref) AS slots FROM linggan_topic_map_collection_round r WHERE r.domain_ref=$1 ORDER BY r.created_at DESC,r.round_ref DESC LIMIT 30").bind(domain).fetch_all(db.pool()).await?;
    Ok(
        json!({"domainRef":domain,"policy":policy.unwrap_or(json!({"enabled":false,"revision":0})),"targets":targets.iter().map(|r|json!({"targetRef":r.get::<Uuid,_>("target_ref"),"kind":r.get::<String,_>("target_kind"),"displayName":r.get::<String,_>("display_name")})).collect::<Vec<_>>(),"collectionRounds":rounds.iter().map(|r|json!({"roundRef":r.get::<Uuid,_>("round_ref"),"kind":r.get::<String,_>("kind"),"state":r.get::<String,_>("state"),"reason":r.get::<Option<String>,_>("last_reason"),"detailSlotsReserved":r.get::<i64,_>("slots")})).collect::<Vec<_>>()}),
    )
}
