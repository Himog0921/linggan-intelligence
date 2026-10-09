//! Topic-map research commands and immutable canonical input identity.
use crate::topic_map_research_analysis::METHOD_VERSION;
use linggan_evidence::creator_discovery::{self, DiscoveryWork, Fragment};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("research_not_found")]
    NotFound,
    #[error("request_identity_conflict")]
    Conflict,
}
impl ResearchError {
    pub fn code(&self) -> &str {
        match self {
            Self::Database(_) => "research_read_failed",
            Self::Invalid(c) => c,
            Self::NotFound => "research_not_found",
            Self::Conflict => "request_identity_conflict",
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResearchCommand {
    Configure {
        request_ref: Uuid,
        domain_ref: Uuid,
        model_config_ref: Uuid,
        daily_token_limit: i64,
        run_token_limit: i64,
        automatic_enabled: bool,
        collection_enabled: bool,
    },
    Start {
        request_ref: Uuid,
        domain_ref: Uuid,
        trigger: String,
        #[serde(default)]
        work_refs: Vec<Uuid>,
        topic_ref: Option<Uuid>,
    },
    Pause {
        request_ref: Uuid,
        domain_ref: Uuid,
        run_ref: Option<Uuid>,
    },
    Resume {
        request_ref: Uuid,
        domain_ref: Uuid,
        run_ref: Option<Uuid>,
    },
    Stop {
        request_ref: Uuid,
        domain_ref: Uuid,
        run_ref: Option<Uuid>,
    },
}
impl ResearchCommand {
    fn identity(&self) -> (Uuid, Uuid) {
        match self {
            Self::Configure {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Start {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Pause {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Resume {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Stop {
                request_ref,
                domain_ref,
                ..
            } => (*request_ref, *domain_ref),
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct ResearchInput {
    pub work: DiscoveryWork,
    pub fragments: Vec<Fragment>,
    pub topics: Value,
    pub domain: Value,
    pub hash: String,
    pub context_work_refs: Vec<Uuid>,
    pub comment_study: Value,
    pub role_metadata: Value,
}
pub(crate) async fn load_inputs(
    db: &Database,
    domain: Uuid,
    config: Uuid,
) -> Result<Vec<ResearchInput>, ResearchError> {
    let scope = serde_json::from_value(json!({"domain":domain}))
        .map_err(|_| ResearchError::Invalid("invalid_domain"))?;
    let data = creator_discovery::load(db, &scope)
        .await
        .map_err(|e| match e {
            creator_discovery::DiscoveryError::Database(e) => ResearchError::Database(e),
            _ => ResearchError::Invalid("source_unavailable"),
        })?;
    let rows=sqlx::query("SELECT t.topic_ref,d.definition_ref,d.version,d.display_name,d.definition_text FROM linggan_topic_workspace t JOIN LATERAL(SELECT * FROM linggan_topic_map_binding WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)b ON true JOIN LATERAL(SELECT * FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true WHERE b.domain_ref=$1 ORDER BY t.topic_ref LIMIT 100").bind(domain).fetch_all(db.pool()).await?;
    let topics=Value::Array(rows.iter().map(|r|json!({"topicRef":r.get::<Uuid,_>("topic_ref"),"definitionRef":r.get::<Uuid,_>("definition_ref"),"version":r.get::<i32,_>("version"),"label":r.get::<String,_>("display_name"),"definition":r.get::<String,_>("definition_text")})).collect());
    let study_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_study_effective_signal')IS NOT NULL",
    )
    .fetch_one(db.pool())
    .await?;
    let own_rows=sqlx::query("SELECT platform,author_external_id FROM(SELECT DISTINCT ON(platform,author_external_id)*FROM linggan_topic_map_own_creator WHERE domain_ref=$1 ORDER BY platform,author_external_id,created_at DESC)current WHERE active").bind(domain).fetch_all(db.pool()).await?;
    let own: std::collections::HashSet<(String, String)> = own_rows
        .iter()
        .map(|r| (r.get("platform"), r.get("author_external_id")))
        .collect();
    let mut inputs = Vec::new();
    for work in data.works.into_iter().filter(|w| !w.fragments.is_empty()) {
        let mut fragments = work.fragments.clone();
        let mut comments = Vec::new();
        {
            let sources = crate::comment_study_source::eligible_sources_for_works(
                db,
                domain,
                &data.as_of,
                &[work.work_ref],
                30,
            )
            .await
            .map_err(|e| match e {
                crate::comment_study_source::StudySourceError::Database(e) => {
                    ResearchError::Database(e)
                }
                _ => ResearchError::Invalid("comment_source_unavailable"),
            })?;
            for source in sources {
                let signals = if study_ready {
                    sqlx::query("SELECT s.signal_ref,s.kind,s.proposition FROM linggan_comment_study_effective_signal s JOIN linggan_comment_study_target t USING(target_ref)WHERE s.domain_ref=$1 AND t.source_ref=$2 ORDER BY s.signal_ref LIMIT 8").bind(domain).bind(source.source_ref).fetch_all(db.pool()).await?
                } else {
                    Vec::new()
                };
                let comment_id:String=sqlx::query_scalar("SELECT comment_external_id FROM linggan_material_comment WHERE material_ref=$1").bind(source.source_ref).fetch_one(db.pool()).await?;
                let f = Fragment {
                    fragment_id: format!(
                        "{}.comment.{}",
                        work.work_ref,
                        creator_discovery::hash(&comment_id)
                    ),
                    source_ref: source.source_ref,
                    field: if signals.is_empty() {
                        "unresearched_comment"
                    } else {
                        "studied_comment"
                    }
                    .into(),
                    source_version: creator_discovery::hash(&source.research_text),
                    start: 0,
                    end: source.research_text.chars().count(),
                    text: source.research_text,
                };
                comments.push(json!({"workRef":work.work_ref,"sourceRef":source.source_ref,"fragmentId":f.fragment_id,"role":"eligible_user_comment","researchState":if signals.is_empty(){"unresearched"}else{"accepted"},"parentSourceRef":source.parent_source_ref,"signals":signals.iter().map(|r|json!({"signalRef":r.get::<Uuid,_>("signal_ref"),"kind":r.get::<String,_>("kind"),"proposition":r.get::<String,_>("proposition")})).collect::<Vec<_>>()}));
                fragments.push(f);
            }
        }
        let breakout:Option<bool>=sqlx::query_scalar("SELECT marked FROM linggan_topic_map_breakout WHERE domain_ref=$1 AND work_public_ref=$2 ORDER BY created_at DESC LIMIT 1").bind(domain).bind(work.work_ref).fetch_optional(db.pool()).await?;
        let role_metadata = json!({"workRef":work.work_ref,"usageRoles":work.usage_roles,"own":work.author_external_id.as_ref().is_some_and(|id|own.contains(&(work.platform.clone(),id.clone()))),"ownScopeConfigured":!own.is_empty(),"manualOwnBreakout":breakout.unwrap_or(false),"likes":work.likes,"likesObservedAt":work.likes_observed_at,"followerCount":work.profile["followerCount"],"lowFollowerHighLikes":"unknown unless configured rule and known followers","publishedAt":work.published_date});
        let comment_study = json!(&comments);
        let hash=creator_discovery::hash(&json!({"domain":domain,"domainDefinition":data.domain,"work":work.work_ref,"fragments":fragments.iter().map(|f|json!({"id":f.fragment_id,"field":f.field,"start":f.start,"end":f.end,"hash":creator_discovery::hash(&f.text)})).collect::<Vec<_>>(),"commentStudy":comments.iter().map(|c|json!({"fragmentId":c["fragmentId"],"state":c["researchState"],"signals":c["signals"].as_array().map(|v|v.iter().map(|v|json!({"kind":v["kind"],"proposition":v["proposition"]})).collect::<Vec<_>>())})).collect::<Vec<_>>(),"roles":{"usageRoles":work.usage_roles,"own":role_metadata["own"]},"topics":topics,"method":METHOD_VERSION,"config":config}).to_string());
        inputs.push(ResearchInput {
            work,
            fragments,
            topics: topics.clone(),
            domain: data.domain.clone(),
            hash,
            context_work_refs: vec![],
            comment_study,
            role_metadata,
        });
    }
    Ok(inputs)
}
pub(crate) fn with_comparison_context(
    mut input: ResearchInput,
    all: &[ResearchInput],
    context: &[Uuid],
) -> ResearchInput {
    let mut hashes = Vec::new();
    for work in context {
        if let Some(other) = all.iter().find(|i| i.work.work_ref == *work) {
            hashes.push((other.work.work_ref, other.hash.clone()));
            if other.work.work_ref != input.work.work_ref {
                input.fragments.extend(other.fragments.clone());
                if let (Some(target), Some(more)) = (
                    input.comment_study.as_array_mut(),
                    other.comment_study.as_array(),
                ) {
                    target.extend(more.clone());
                }
            }
        }
    }
    if !context.is_empty() {
        input.role_metadata = json!({"works":all.iter().filter(|i|context.contains(&i.work.work_ref)).map(|i|i.role_metadata.clone()).collect::<Vec<_>>()});
        input.hash = creator_discovery::hash(
            &json!({"primaryInput":input.hash,"comparisonInputs":hashes,"method":METHOD_VERSION})
                .to_string(),
        );
        input.context_work_refs = context.to_vec();
    }
    input
}
pub(crate) fn reference_manifest(input: &ResearchInput) -> Value {
    json!({"workRef":input.work.work_ref,"contextWorkRefs":input.context_work_refs,"methodVersion":METHOD_VERSION,"domain":input.domain["domainRef"],"topics":input.topics,"commentStudy":input.comment_study,"roleMetadata":input.role_metadata,"fragments":input.fragments.iter().map(|f|json!({"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"sourceVersion":f.source_version,"field":f.field,"start":f.start,"end":f.end,"textHash":creator_discovery::hash(&f.text)})).collect::<Vec<_>>()})
}

/// Re-resolve every frozen dependency through the current Domain/comment restriction gates.
/// Physical observation IDs may differ; stable fragment identity and qualified text must match.
pub(crate) fn sources_current(input: &ResearchInput, manifest: &Value) -> bool {
    manifest["fragments"].as_array().is_some_and(|refs| {
        refs.len() == input.fragments.len()
            && refs.iter().all(|r| {
                input.fragments.iter().any(|f| {
                    r["fragmentId"].as_str() == Some(&f.fragment_id)
                        && r["field"].as_str() == Some(&f.field)
                        && r["textHash"].as_str() == Some(&creator_discovery::hash(&f.text))
                })
            })
    }) && manifest["topics"].as_array().is_some_and(|defs| {
        defs.iter().all(|d| {
            input.topics.as_array().is_some_and(|current| {
                current
                    .iter()
                    .any(|t| t["definitionRef"] == d["definitionRef"])
            })
        })
    })
}
pub(crate) async fn research_result_sources_current(
    db: &Database,
    result: Uuid,
) -> Result<bool, ResearchError> {
    let row=sqlx::query("SELECT r.domain_ref,r.work_public_ref,run.config_ref,t.input_refs FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_task t ON t.domain_ref=r.domain_ref AND t.work_public_ref=r.work_public_ref AND t.input_hash=r.input_hash JOIN linggan_topic_map_research_run run USING(run_ref)WHERE r.result_ref=$1").bind(result).fetch_optional(db.pool()).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let domain: Uuid = row.get("domain_ref");
    let config: Uuid = row.get("config_ref");
    let work: Uuid = row.get("work_public_ref");
    let manifest: Value = row.get("input_refs");
    let all = load_inputs(db, domain, config).await?;
    let context: Vec<Uuid> = manifest["contextWorkRefs"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|v| v.as_str().and_then(|v| v.parse().ok()))
                .collect()
        })
        .unwrap_or_default();
    if context.len() > 10
        || context
            .iter()
            .any(|r| !all.iter().any(|i| i.work.work_ref == *r))
    {
        return Ok(false);
    }
    Ok(all
        .iter()
        .find(|i| i.work.work_ref == work)
        .is_some_and(|i| {
            sources_current(
                &with_comparison_context(i.clone(), &all, &context),
                &manifest,
            )
        }))
}

pub async fn apply_research_command(
    db: &Database,
    command: &ResearchCommand,
) -> Result<Value, ResearchError> {
    let (request, domain) = command.identity();
    let hash = creator_discovery::hash(
        &serde_json::to_string(command).map_err(|_| ResearchError::Invalid("invalid_command"))?,
    );
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,41))")
        .bind(request.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query(
        "SELECT command_hash,receipt FROM linggan_topic_map_research_command WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        if row.get::<String, _>("command_hash") != hash {
            return Err(ResearchError::Conflict);
        }
        let receipt: Value = row.get("receipt");
        if receipt["state"] != "creating" {
            return Ok(receipt);
        }
        tx.rollback().await?;
        return finish_pending_start(db, command).await;
    }
    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain)
            .fetch_one(&mut *tx)
            .await?;
    if !valid {
        return Err(ResearchError::NotFound);
    }
    let mut start = None;
    let receipt = match command {
        ResearchCommand::Configure {
            model_config_ref,
            daily_token_limit,
            run_token_limit,
            automatic_enabled,
            collection_enabled,
            ..
        } => {
            if !(1024..=10000000).contains(daily_token_limit)
                || !(1024..=10000000).contains(run_token_limit)
            {
                return Err(ResearchError::Invalid("invalid_research_budget"));
            }
            let config: Option<Uuid> = sqlx::query_scalar(
                "SELECT config_ref FROM linggan_model_config WHERE config_ref=$1",
            )
            .bind(model_config_ref)
            .fetch_optional(&mut *tx)
            .await?;
            if config.is_none() {
                return Err(ResearchError::Invalid("model_configuration_missing"));
            }
            let previous:Option<bool>=sqlx::query_scalar("SELECT automatic_enabled FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE").bind(domain).fetch_optional(&mut *tx).await?;
            sqlx::query("INSERT INTO linggan_topic_map_research_policy(domain_ref,model_config_ref,daily_token_limit,run_token_limit,automatic_enabled,collection_enabled) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(domain_ref) DO UPDATE SET model_config_ref=$2,daily_token_limit=$3,run_token_limit=$4,automatic_enabled=$5,collection_enabled=$6,status='active',revision=linggan_topic_map_research_policy.revision+1,updated_at=scope_001_now()").bind(domain).bind(model_config_ref).bind(daily_token_limit).bind(run_token_limit).bind(automatic_enabled).bind(collection_enabled).execute(&mut *tx).await?;
            sqlx::query("UPDATE linggan_topic_map_research_run SET state=CASE WHEN $2 THEN 'queued' ELSE 'paused' END,last_reason=CASE WHEN $2 THEN NULL ELSE 'automatic_disabled' END,updated_at=scope_001_now() WHERE domain_ref=$1 AND trigger<>'on_demand'AND (($2 AND state='paused'AND last_reason='automatic_disabled')OR(NOT $2 AND state IN('queued','running','daily_budget_paused')))").bind(domain).bind(automatic_enabled).execute(&mut *tx).await?;
            if *automatic_enabled && previous != Some(true) {
                start = Some(("historical".to_string(), vec![], None));
            }
            json!({"requestRef":request,"domainRef":domain,"state":"configured","automaticEnabled":automatic_enabled,"collectionEnabled":collection_enabled})
        }
        ResearchCommand::Start {
            trigger,
            work_refs,
            topic_ref,
            ..
        } => {
            if !["historical", "incremental", "on_demand"].contains(&trigger.as_str())
                || work_refs.len() > 100
            {
                return Err(ResearchError::Invalid("invalid_research_scope"));
            }
            start = Some((trigger.clone(), work_refs.clone(), *topic_ref));
            json!({"requestRef":request,"domainRef":domain,"state":"queued"})
        }
        ResearchCommand::Pause { run_ref, .. }
        | ResearchCommand::Resume { run_ref, .. }
        | ResearchCommand::Stop { run_ref, .. } => {
            let state = match command {
                ResearchCommand::Pause { .. } => "paused",
                ResearchCommand::Resume { .. } => "queued",
                _ => "stopped",
            };
            if let Some(run) = run_ref {
                let count=sqlx::query("UPDATE linggan_topic_map_research_run SET state=$3,last_reason=$3,updated_at=scope_001_now() WHERE run_ref=$1 AND domain_ref=$2 AND state IN ('queued','running','daily_budget_paused','paused')").bind(run).bind(domain).bind(state).execute(&mut *tx).await?.rows_affected();
                if count == 0 {
                    return Err(ResearchError::Invalid("run_terminal_or_unavailable"));
                }
                if state == "stopped" {
                    sqlx::query("UPDATE linggan_topic_map_research_task SET state='stopped',last_reason='user_stopped' WHERE run_ref=$1 AND state='queued'").bind(run).execute(&mut *tx).await?;
                }
            } else {
                let policy_state = if state == "queued" { "active" } else { state };
                sqlx::query("UPDATE linggan_topic_map_research_policy SET status=$2,updated_at=scope_001_now() WHERE domain_ref=$1").bind(domain).bind(policy_state).execute(&mut *tx).await?;
                if state == "stopped" {
                    sqlx::query("UPDATE linggan_topic_map_research_run SET state='stopped',last_reason='user_stopped' WHERE domain_ref=$1 AND state IN ('queued','running','paused','daily_budget_paused')").bind(domain).execute(&mut *tx).await?;
                }
            }
            json!({"requestRef":request,"domainRef":domain,"runRef":run_ref,"state":state})
        }
    };
    // Queue loading uses canonical readers outside this short policy transaction.
    sqlx::query("INSERT INTO linggan_topic_map_research_command(request_ref,domain_ref,command_hash,receipt) VALUES($1,$2,$3,$4)").bind(request).bind(domain).bind(hash).bind(if start.is_some(){json!({"requestRef":request,"domainRef":domain,"state":"creating"})}else{receipt.clone()}).execute(&mut *tx).await?;
    tx.commit().await?;
    if start.is_some() {
        return finish_pending_start(db, command).await;
    }
    Ok(receipt)
}
async fn finish_pending_start(
    db: &Database,
    command: &ResearchCommand,
) -> Result<Value, ResearchError> {
    let (request, domain) = command.identity();
    let (trigger, works, topic) = match command {
        ResearchCommand::Start {
            trigger,
            work_refs,
            topic_ref,
            ..
        } => (trigger.as_str(), work_refs.as_slice(), *topic_ref),
        ResearchCommand::Configure {
            automatic_enabled: true,
            ..
        } => ("historical", &[][..], None),
        _ => return Err(ResearchError::Invalid("invalid_pending_command")),
    };
    let run = queue_research_run(db, domain, request, trigger, works, topic).await?;
    let receipt = json!({"requestRef":request,"domainRef":domain,"state":if run.is_some(){"queued"}else{"no_new_input"},"runRef":run});
    sqlx::query("UPDATE linggan_topic_map_research_command SET receipt=$2 WHERE request_ref=$1 AND receipt->>'state'='creating'").bind(request).bind(&receipt).execute(db.pool()).await?;
    Ok(receipt)
}
pub(crate) async fn queue_research_run(
    db: &Database,
    domain: Uuid,
    request: Uuid,
    trigger: &str,
    works: &[Uuid],
    topic: Option<Uuid>,
) -> Result<Option<Uuid>, ResearchError> {
    let p = sqlx::query("SELECT * FROM linggan_topic_map_research_policy WHERE domain_ref=$1")
        .bind(domain)
        .fetch_optional(db.pool())
        .await?
        .ok_or(ResearchError::Invalid("research_not_configured"))?;
    let config: Uuid = p.get("model_config_ref");
    let input = load_inputs(db, domain, config).await?;
    let mut tx = db.pool().begin().await?;
    sqlx::query(
        "SELECT domain_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(domain)
    .fetch_one(&mut *tx)
    .await?;
    if let Some(run) = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_topic_map_research_run WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        return Ok(Some(run));
    }
    let context: Vec<Uuid> = if trigger == "on_demand" {
        let topic_works: Vec<Uuid> = if let Some(t) = topic {
            sqlx::query_scalar("SELECT member.work_public_ref FROM linggan_topic_material_member member JOIN linggan_topic_classification_run run USING(classification_run_ref) JOIN linggan_topic_definition def USING(definition_ref) WHERE def.topic_ref=$1 ORDER BY member.ordinal LIMIT 10").bind(t).fetch_all(&mut *tx).await?
        } else {
            vec![]
        };
        if !works.is_empty() {
            works
                .iter()
                .filter(|r| input.iter().any(|i| i.work.work_ref == **r))
                .take(10)
                .copied()
                .collect()
        } else {
            input
                .iter()
                .filter(|i| topic_works.is_empty() || topic_works.contains(&i.work.work_ref))
                .take(10)
                .map(|i| i.work.work_ref)
                .collect()
        }
    } else {
        vec![]
    };
    let mut selected = Vec::new();
    for original in input
        .iter()
        .filter(|i| works.is_empty() || works.contains(&i.work.work_ref))
    {
        let i = if trigger == "on_demand" {
            if context.first() != Some(&original.work.work_ref) {
                continue;
            }
            with_comparison_context(original.clone(), &input, &context)
        } else {
            original.clone()
        };
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND work_public_ref=$2 AND input_hash=$3)").bind(domain).bind(i.work.work_ref).bind(&i.hash).fetch_one(&mut *tx).await?;
        if !exists {
            selected.push(i);
            if selected.len() >= 100 || trigger == "on_demand" {
                break;
            }
        }
    }
    if selected.is_empty() {
        return Ok(None);
    }
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_map_research_run(run_ref,domain_ref,request_ref,trigger,topic_ref,config_ref,method_version,token_limit,state) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'queued')").bind(run).bind(domain).bind(request).bind(trigger).bind(topic).bind(config).bind(METHOD_VERSION).bind(p.get::<i64,_>("run_token_limit")).execute(&mut *tx).await?;
    for i in selected {
        sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING").bind(Uuid::new_v4()).bind(run).bind(domain).bind(i.work.work_ref).bind(&i.hash).bind(reference_manifest(&i)).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Some(run))
}
pub async fn read_research_progress(db: &Database, domain: Uuid) -> Result<Value, ResearchError> {
    let p=sqlx::query("SELECT *,updated_at::text AS updated FROM linggan_topic_map_research_policy WHERE domain_ref=$1").bind(domain).fetch_optional(db.pool()).await?;
    let policy=p.map(|r|json!({"modelConfigRef":r.get::<Uuid,_>("model_config_ref"),"dailyTokenLimit":r.get::<i64,_>("daily_token_limit"),"runTokenLimit":r.get::<i64,_>("run_token_limit"),"automaticEnabled":r.get::<bool,_>("automatic_enabled"),"collectionEnabled":r.get::<bool,_>("collection_enabled"),"status":r.get::<String,_>("status"),"updatedAt":r.get::<String,_>("updated")}));
    let usage=sqlx::query("SELECT COALESCE(sum(i.charged_tokens),0)::bigint AS charged,COALESCE(sum(i.reserved_tokens)FILTER(WHERE i.state='running'),0)::bigint AS reserved FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) JOIN linggan_topic_map_research_run r USING(run_ref) WHERE r.domain_ref=$1 AND q.budget_day=(scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date").bind(domain).fetch_one(db.pool()).await?;
    let rows=sqlx::query("SELECT r.*,r.created_at::text AS created,(SELECT count(*)FROM linggan_topic_map_research_task t WHERE t.run_ref=r.run_ref AND t.state='queued')AS queued_count,(SELECT count(*)FROM linggan_topic_map_research_task t WHERE t.run_ref=r.run_ref AND t.state IN ('succeeded','no_signal','insufficient'))AS succeeded_count,(SELECT count(*)FROM linggan_topic_map_research_task t WHERE t.run_ref=r.run_ref AND t.state IN ('failed','unknown_dispatch'))AS failed_count FROM linggan_topic_map_research_run r WHERE domain_ref=$1 ORDER BY created_at DESC LIMIT 30").bind(domain).fetch_all(db.pool()).await?;
    let models=sqlx::query("SELECT c.*,m.model_id FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) WHERE conn.enabled ORDER BY c.created_at DESC LIMIT 30").fetch_all(db.pool()).await?;
    let targets=sqlx::query("SELECT t.target_ref,t.target_kind,t.display_name,t.identity_key FROM collection_observation_target t JOIN observation_domain_target membership USING(target_ref)WHERE membership.domain_ref=$1 AND t.lifecycle_state<>'dismissed'ORDER BY t.display_name,t.target_ref LIMIT 100").bind(domain).fetch_all(db.pool()).await?;
    let rounds=sqlx::query("SELECT r.*,r.created_at::text AS created,(SELECT count(*)FROM linggan_topic_map_collection_slot s WHERE s.round_ref=r.round_ref)AS reserved_count FROM linggan_topic_map_collection_round r WHERE r.domain_ref=$1 ORDER BY created_at DESC LIMIT 20").bind(domain).fetch_all(db.pool()).await?;
    Ok(
        json!({"domainRef":domain,"methodVersion":METHOD_VERSION,"policy":policy,"targets":targets.iter().map(|r|json!({"targetRef":r.get::<Uuid,_>("target_ref"),"kind":r.get::<String,_>("target_kind"),"displayName":r.get::<Option<String>,_>("display_name").unwrap_or(r.get::<String,_>("identity_key")),"domainRef":domain})).collect::<Vec<_>>(),"collectionRounds":rounds.iter().map(|r|json!({"roundRef":r.get::<Uuid,_>("round_ref"),"kind":r.get::<String,_>("kind"),"state":r.get::<String,_>("state"),"reason":r.get::<Option<String>,_>("last_reason"),"detailSlotsReserved":r.get::<i64,_>("reserved_count"),"detailLimit":r.get::<i32,_>("detail_limit"),"createdAt":r.get::<String,_>("created")})).collect::<Vec<_>>(),"usage":{"chargedTokens":usage.get::<i64,_>("charged"),"reservedTokens":usage.get::<i64,_>("reserved"),"timezone":"Asia/Shanghai"},"runs":rows.iter().map(|r|json!({"runRef":r.get::<Uuid,_>("run_ref"),"trigger":r.get::<String,_>("trigger"),"state":r.get::<String,_>("state"),"lastReason":r.get::<Option<String>,_>("last_reason"),"queuedCount":r.get::<i64,_>("queued_count"),"succeededCount":r.get::<i64,_>("succeeded_count"),"failedCount":r.get::<i64,_>("failed_count"),"createdAt":r.get::<String,_>("created")})).collect::<Vec<_>>(),"models":models.iter().map(|r|json!({"configRef":r.get::<Uuid,_>("config_ref"),"modelId":r.get::<String,_>("model_id"),"inputTokenLimit":r.get::<i32,_>("input_token_limit"),"outputTokenLimit":r.get::<i32,_>("output_token_limit"),"maxAttempts":r.get::<i32,_>("max_attempts"),"timeoutSeconds":r.get::<i32,_>("timeout_seconds")})).collect::<Vec<_>>() }),
    )
}
