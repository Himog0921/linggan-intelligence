//! Assemble full qualified canonical materials and already acquired comment lineage.
use super::windows::{input_identity, is_research_evidence, update_source_coverage};
use super::{INPUT_CONTRACT, PARENT_CONTEXT_FIELD, ResearchError, ResearchInput};
use linggan_evidence::creator_discovery::{self, Fragment};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(crate) async fn load_inputs(
    db: &Database,
    domain: Uuid,
    config: Uuid,
) -> Result<Vec<ResearchInput>, ResearchError> {
    let data = creator_discovery::load_topic_materials(db, domain)
        .await
        .map_err(|e| match e {
            creator_discovery::DiscoveryError::Database(e) => ResearchError::Database(e),
            _ => ResearchError::Invalid("source_unavailable"),
        })?;
    let rows=sqlx::query("SELECT t.topic_ref,d.definition_ref,d.version,d.display_name,d.definition_text FROM linggan_topic_workspace t JOIN LATERAL(SELECT * FROM linggan_topic_map_binding WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)b ON true JOIN LATERAL(SELECT * FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true WHERE b.domain_ref=$1 ORDER BY t.topic_ref").bind(domain).fetch_all(db.pool()).await?;
    let topics=Value::Array(rows.iter().map(|r|json!({"topicRef":r.get::<Uuid,_>("topic_ref"),"definitionRef":r.get::<Uuid,_>("definition_ref"),"version":r.get::<i32,_>("version"),"label":r.get::<String,_>("display_name"),"definition":r.get::<String,_>("definition_text")})).collect());
    let study_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_study_effective_signal')IS NOT NULL",
    )
    .fetch_one(db.pool())
    .await?;
    let own_rows=sqlx::query("SELECT platform,author_external_id FROM(SELECT DISTINCT ON(platform,author_external_id)*FROM linggan_topic_map_own_creator WHERE domain_ref=$1 ORDER BY platform,author_external_id,created_at DESC)current WHERE active").bind(domain).fetch_all(db.pool()).await?;
    let own: BTreeSet<(String, String)> = own_rows
        .iter()
        .map(|r| (r.get("platform"), r.get("author_external_id")))
        .collect();
    let selections: Vec<_> = data
        .works
        .iter()
        .filter_map(|work| {
            use crate::comment_study_source::StudySourceRole;
            if work.usage_roles.iter().any(|role| role == "primary") {
                Some((work.work_ref, StudySourceRole::Primary))
            } else if work.usage_roles.iter().any(|role| role == "reference") {
                Some((work.work_ref, StudySourceRole::Reference))
            } else {
                None
            }
        })
        .collect();
    let sources = crate::comment_study_source::eligible_sources_for_topic_research(
        db,
        domain,
        &data.as_of,
        &selections,
    )
    .await
    .map_err(|e| match e {
        crate::comment_study_source::StudySourceError::Database(e) => ResearchError::Database(e),
        _ => ResearchError::Invalid("comment_source_unavailable"),
    })?;
    let (comment_ids, signals) = comment_lineage(db, domain, &sources, study_ready).await?;
    let mut comments_by_work =
        BTreeMap::<Uuid, Vec<crate::comment_study_source::StudySource>>::new();
    for source in sources {
        comments_by_work
            .entry(source.content_public_ref)
            .or_default()
            .push(source);
    }
    let breakouts: BTreeMap<Uuid, bool> = sqlx::query("SELECT DISTINCT ON(work_public_ref) work_public_ref,marked FROM linggan_topic_map_breakout WHERE domain_ref=$1 ORDER BY work_public_ref,created_at DESC")
        .bind(domain).fetch_all(db.pool()).await?.into_iter()
        .map(|r| (r.get("work_public_ref"), r.get("marked"))).collect();
    let mut inputs = Vec::new();
    for mut work in data.works {
        work.usage_roles.sort();
        work.usage_roles.dedup();
        let (comment_fragments, comments) = comment_inputs(
            work.work_ref,
            comments_by_work.remove(&work.work_ref).unwrap_or_default(),
            &comment_ids,
            &signals,
        )?;
        let mut fragments = work.fragments.clone();
        fragments.extend(comment_fragments);
        if !fragments.iter().any(is_research_evidence) {
            continue;
        }
        let role_metadata = json!({
            "workRef":work.work_ref,"platform":work.platform,"authorExternalId":work.author_external_id,"usageRoles":work.usage_roles,
            "own":work.author_external_id.as_ref().is_some_and(|id|own.contains(&(work.platform.clone(),id.clone()))),
            "ownScopeConfigured":!own.is_empty(),"manualOwnBreakout":breakouts.get(&work.work_ref).copied().unwrap_or(false),
            "likes":work.likes,"likesObservedAt":work.likes_observed_at,"followerCount":work.profile["followerCount"],
            "lowFollowerHighLikes":"unknown unless configured rule and known followers","publishedAt":work.published_date,
            "authorTextAvailable":!work.fragments.is_empty(),"commentOnly":work.fragments.is_empty(),
        });
        let mut input = ResearchInput {
            work,
            fragments,
            topics: topics.clone(),
            domain: data.domain.clone(),
            hash: String::new(),
            context_work_refs: Vec::new(),
            comment_study: json!(comments),
            role_metadata,
            coverage: json!({"inputContract":INPUT_CONTRACT,"configRef":config}),
        };
        update_source_coverage(&mut input);
        input.hash = input_identity(&input);
        inputs.push(input);
    }
    Ok(inputs)
}

#[derive(Clone)]
struct CommentIdentity {
    work: Uuid,
    external_id: String,
    author: Option<String>,
}
type CommentIds = BTreeMap<Uuid, CommentIdentity>;
type StudySignals = BTreeMap<(Uuid, String), Vec<Value>>;

async fn comment_lineage(
    db: &Database,
    domain: Uuid,
    sources: &[crate::comment_study_source::StudySource],
    study_ready: bool,
) -> Result<(CommentIds, StudySignals), ResearchError> {
    let refs: Vec<_> = sources
        .iter()
        .flat_map(|source| [Some(source.source_ref), source.parent_source_ref])
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut ids = BTreeMap::new();
    for refs in refs.chunks(500) {
        let rows = sqlx::query("SELECT material_ref,content_public_ref,comment_external_id,author_external_id FROM linggan_material_comment WHERE material_ref=ANY($1)")
            .bind(refs).fetch_all(db.pool()).await?;
        for row in rows {
            ids.insert(
                row.get("material_ref"),
                CommentIdentity {
                    work: row.get("content_public_ref"),
                    external_id: row.get("comment_external_id"),
                    author: row.get("author_external_id"),
                },
            );
        }
    }
    let mut signals = StudySignals::new();
    if study_ready {
        let works: Vec<_> = sources
            .iter()
            .map(|source| source.content_public_ref)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        for works in works.chunks(500) {
            // The effective head already checks current comment/parent text and
            // eligibility. A new physical observation must not detach its Signal.
            let rows = sqlx::query("SELECT content_public_ref,comment_external_id,signal_ref,kind,proposition FROM linggan_comment_study_effective_signal WHERE domain_ref=$1 AND content_public_ref=ANY($2) ORDER BY content_public_ref,comment_external_id,signal_ref")
                .bind(domain).bind(works).fetch_all(db.pool()).await?;
            for row in rows {
                signals.entry((row.get("content_public_ref"),row.get("comment_external_id"))).or_default()
                    .push(json!({"signalRef":row.get::<Uuid,_>("signal_ref"),"kind":row.get::<String,_>("kind"),
                        "proposition":row.get::<String,_>("proposition")}));
            }
        }
    }
    Ok((ids, signals))
}

fn comment_inputs(
    work: Uuid,
    mut sources: Vec<crate::comment_study_source::StudySource>,
    ids: &CommentIds,
    signals: &StudySignals,
) -> Result<(Vec<Fragment>, Vec<Value>), ResearchError> {
    sources.sort_by(|a, b| {
        ids.get(&a.source_ref)
            .map(|c| &c.external_id)
            .cmp(&ids.get(&b.source_ref).map(|c| &c.external_id))
    });
    let mut fragments = Vec::new();
    let mut comments = Vec::new();
    let mut parents = BTreeMap::new();
    for source in sources {
        let identity = ids
            .get(&source.source_ref)
            .filter(|id| id.work == work)
            .ok_or(ResearchError::Invalid("comment_identity_unavailable"))?;
        let signals = signals
            .get(&(work, identity.external_id.clone()))
            .cloned()
            .unwrap_or_default();
        let fragment = Fragment {
            fragment_id: format!(
                "{work}.comment.{}",
                creator_discovery::hash(&identity.external_id)
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
        let mut parent_id = None;
        let mut parent_author = None;
        if let (Some(reference), Some(text)) =
            (source.parent_source_ref, source.parent_research_text)
            && !text.is_empty()
        {
            let identity =
                ids.get(&reference)
                    .filter(|id| id.work == work)
                    .ok_or(ResearchError::Invalid(
                        "parent_comment_identity_unavailable",
                    ))?;
            let id = format!(
                "{work}.comment-context.{}",
                creator_discovery::hash(&identity.external_id)
            );
            parent_author = identity.author.clone();
            parents.entry(id.clone()).or_insert(Fragment {
                fragment_id: id.clone(),
                source_ref: reference,
                field: PARENT_CONTEXT_FIELD.into(),
                source_version: creator_discovery::hash(&text),
                start: 0,
                end: text.chars().count(),
                text,
            });
            parent_id = Some(id);
        }
        comments.push(json!({"workRef":work,"sourceRef":source.source_ref,"fragmentId":fragment.fragment_id,
            "sourceFragmentId":fragment.fragment_id,"commentExternalId":identity.external_id,
            "authorExternalId":identity.author,"role":"eligible_user_comment","contextOnly":false,
            "researchState":if signals.is_empty(){"unresearched"}else{"accepted"},
            "observationRole":source.observation_role.as_str(),"parentSourceRef":source.parent_source_ref,
            "parentFragmentId":parent_id,"parentAuthorExternalId":parent_author,"parentContextOnly":true,"signals":signals}));
        fragments.push(fragment);
    }
    fragments.extend(parents.into_values());
    Ok((fragments, comments))
}
