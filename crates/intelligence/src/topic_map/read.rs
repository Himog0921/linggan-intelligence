use super::*;
use linggan_contracts::EvidenceQuery;
use linggan_evidence::read_work_resources;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub async fn read_topic_map(
    database: &Database,
    q: &TopicMapQuery,
) -> Result<TopicMapSnapshot, TopicMapError> {
    if q.platform
        .as_deref()
        .is_some_and(|p| !matches!(p, "xhs" | "douyin"))
        || q.window_days.is_some_and(|d| ![7, 30, 90].contains(&d))
        || q.reference_window_days
            .is_some_and(|d| ![0, 7, 30, 90].contains(&d))
        || q.overlay.as_deref().is_some_and(|p| !OVERLAYS.contains(&p))
        || q.path
            .as_deref()
            .is_some_and(|p| !matches!(p, "family" | "adult"))
    {
        return Err(TopicMapError::Invalid("invalid scope"));
    }
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(database.pool())
        .await?;
    let domains:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('domainRef',domain_ref,'name',name,'status',status,'description',description,'researchGoal',research_goal) FROM observation_domain ORDER BY name").fetch_all(database.pool()).await?;
    if q.domain_ref.is_some_and(|d| {
        !domains
            .iter()
            .any(|v| v["domainRef"].as_str() == Some(&d.to_string()))
    }) {
        return Err(TopicMapError::NotFound);
    }
    let scope_rows=sqlx::query("SELECT u.content_public_ref,u.domain_ref,array_agg(DISTINCT u.role) AS roles FROM linggan_material_domain_usage u JOIN linggan_material_content c ON c.public_ref=u.content_public_ref WHERE ($1::uuid IS NULL OR u.domain_ref=$1) AND ($2::text IS NULL OR c.platform=$2) AND ($3::bool OR u.role='primary') GROUP BY u.content_public_ref,u.domain_ref ORDER BY u.domain_ref,u.content_public_ref").bind(q.domain_ref).bind(&q.platform).bind(q.include_reference).fetch_all(database.pool()).await?;
    let mut roles: HashMap<Uuid, BTreeSet<String>> = HashMap::new();
    let mut domain_batches: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for r in &scope_rows {
        let reference = r.get("content_public_ref");
        let domain_ref = r.get("domain_ref");
        domain_batches
            .entry(domain_ref)
            .or_default()
            .push(reference);
        roles
            .entry(reference)
            .or_default()
            .extend(r.get::<Vec<String>, _>("roles"));
    }
    let annotation_rows=sqlx::query("SELECT DISTINCT ON(work_public_ref) work_public_ref,to_jsonb(a)-'receipt_ref' AS value FROM linggan_topic_map_work_annotation a WHERE ($1::uuid IS NULL OR domain_ref=$1) ORDER BY work_public_ref,created_at DESC,annotation_ref DESC").bind(q.domain_ref).fetch_all(database.pool()).await?;
    let annotations: HashMap<Uuid, Value> = annotation_rows
        .iter()
        .map(|r| (r.get("work_public_ref"), r.get("value")))
        .collect();
    let own_creators:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('domainRef',domain_ref,'platform',platform,'authorExternalId',author_external_id,'active',active,'membershipRef',membership_ref) FROM (SELECT DISTINCT ON(domain_ref,platform,author_external_id) * FROM linggan_topic_map_own_creator WHERE ($1::uuid IS NULL OR domain_ref=$1) ORDER BY domain_ref,platform,author_external_id,created_at DESC,membership_ref DESC) current").bind(q.domain_ref).fetch_all(database.pool()).await?;
    let breakout_rows=sqlx::query("SELECT DISTINCT ON(domain_ref,work_public_ref) domain_ref,work_public_ref,marked FROM linggan_topic_map_breakout WHERE ($1::uuid IS NULL OR domain_ref=$1) ORDER BY domain_ref,work_public_ref,created_at DESC,marking_ref DESC").bind(q.domain_ref).fetch_all(database.pool()).await?;
    let breakouts: HashSet<Uuid> = breakout_rows
        .iter()
        .filter(|r| r.get::<bool, _>("marked"))
        .map(|r| r.get("work_public_ref"))
        .collect();
    let refs: Vec<_> = roles.keys().copied().collect();
    let authors=sqlx::query("SELECT c.public_ref,a.author_external_id,p.follower_count FROM linggan_material_content c LEFT JOIN linggan_material_content_author a ON a.content_public_ref=c.public_ref LEFT JOIN LATERAL(SELECT follower_count FROM linggan_material_author_profile p JOIN linggan_runtime_capture_package package USING(package_ref) WHERE p.platform=c.platform AND p.author_external_id=a.author_external_id AND p.follower_count_state='KNOWN' AND package.accepted_at<=$2::timestamptz ORDER BY p.observed_at::timestamptz DESC,p.created_at DESC,p.material_ref DESC LIMIT 1) p ON true WHERE c.public_ref=ANY($1)").bind(&refs).bind(&as_of).fetch_all(database.pool()).await?;
    let authors: HashMap<Uuid, (Option<String>, Option<i64>)> = authors
        .iter()
        .map(|r| {
            (
                r.get("public_ref"),
                (r.get("author_external_id"), r.get("follower_count")),
            )
        })
        .collect();
    let mut works = Vec::new();
    let mut seen = HashSet::new();
    for (domain_ref, references) in domain_batches {
        for batch in references.chunks(100) {
            let query:EvidenceQuery=serde_json::from_value(json!({"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery","domainRef":domain_ref,"publicRefs":batch})).map_err(|e|TopicMapError::Source(e.to_string()))?;
            let page = read_work_resources(database, &query)
                .await
                .map_err(|e| TopicMapError::Source(e.to_string()))?;
            for resource in page.items {
                let source_manifest = canonical_source_manifest(&resource);
                let reference = resource.identity.public_ref;
                if !seen.insert(reference) {
                    continue;
                }
                let (author, follower) = authors.get(&reference).cloned().unwrap_or_default();
                let own = author.as_ref().is_some_and(|author| {
                    own_creators.iter().any(|v| {
                        v["active"] == true
                            && v["platform"] == resource.identity.platform
                            && v["authorExternalId"] == *author
                    })
                });
                let mut annotation = annotations.get(&reference).cloned();
                let readable = resource.summary.restriction_state != "WITHDRAWN_OR_RESTRICTED"
                    && (resource.evidence_fragment.is_some() || resource.display.title.is_some());
                if !readable {
                    annotation = None;
                }
                let stage = annotation
                    .as_ref()
                    .and_then(|v| v["main_stage"].as_str())
                    .unwrap_or("pending")
                    .to_owned();
                let strings = |field: &str| {
                    annotation
                        .as_ref()
                        .and_then(|v| v[field].as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let path = annotation
                    .as_ref()
                    .and_then(|v| v["path"].as_str())
                    .unwrap_or("unknown")
                    .to_owned();
                works.push(TopicMapWork {
                    work_ref: reference,
                    platform: resource.identity.platform,
                    content_external_id: resource.identity.content_external_id,
                    title: resource.display.title,
                    title_source: resource.display.title_source,
                    author_external_id: author,
                    creator_display_name: resource.display.creator_display_name,
                    published_at: resource.display.published_at,
                    published_at_source_text: resource.display.published_at_source_text,
                    published_at_precision: resource.display.published_at_precision,
                    likes: resource.display.engagement.like_count,
                    comments: resource.display.engagement.comment_count,
                    collects: resource.display.engagement.collect_count,
                    shares: resource.display.engagement.share_count,
                    follower_count: follower,
                    own,
                    own_breakout: own && breakouts.contains(&reference),
                    readable,
                    usage_roles: roles
                        .get(&reference)
                        .map(|r| r.iter().cloned().collect())
                        .unwrap_or_default(),
                    topic_refs: Vec::new(),
                    main_stage: stage,
                    involved_stages: strings("involved_stages"),
                    overlays: strings("overlays"),
                    path,
                    annotation,
                    research: None,
                    media: resource.media,
                    preview: serde_json::to_value(resource.preview)
                        .map_err(|e| TopicMapError::Source(e.to_string()))?,
                    source_manifest,
                    evidence_fragment: resource
                        .evidence_fragment
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(|e| TopicMapError::Source(e.to_string()))?,
                });
            }
        }
    }
    let rules:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('domainRef',domain_ref,'platform',platform,'likeThreshold',like_threshold,'version',version,'ruleRef',rule_ref) FROM (SELECT DISTINCT ON(domain_ref,platform) * FROM linggan_topic_map_performance_rule WHERE ($1::uuid IS NULL OR domain_ref=$1) ORDER BY domain_ref,platform,version DESC) current").bind(q.domain_ref).fetch_all(database.pool()).await?;
    let rows=sqlx::query("SELECT t.topic_ref,t.canonical_key,d.definition_ref,d.version,d.display_name,d.definition_text,d.lifecycle_state,b.domain_ref,b.parent_topic_ref,b.version AS binding_version,array(SELECT m.work_public_ref FROM linggan_topic_classification_run r JOIN linggan_topic_material_member m USING(classification_run_ref) WHERE r.definition_ref=d.definition_ref AND r.run_kind='human_adjudicated' ORDER BY m.ordinal) AS members FROM linggan_topic_workspace t JOIN LATERAL(SELECT * FROM linggan_topic_definition d WHERE d.topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true LEFT JOIN LATERAL(SELECT * FROM linggan_topic_map_binding b WHERE b.topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE ($1::uuid IS NULL OR b.domain_ref=$1) ORDER BY d.display_name").bind(q.domain_ref).fetch_all(database.pool()).await?;
    let mut topics: Vec<TopicMapTopic> = rows
        .iter()
        .map(|r| TopicMapTopic {
            topic_ref: r.get("topic_ref"),
            canonical_key: r.get("canonical_key"),
            domain_ref: r.get("domain_ref"),
            definition_ref: r.get("definition_ref"),
            definition_version: r.get("version"),
            display_name: r.get("display_name"),
            definition_text: r.get("definition_text"),
            lifecycle_state: r.get("lifecycle_state"),
            parent_topic_ref: r.get("parent_topic_ref"),
            binding_version: r.get("binding_version"),
            direct_work_refs: r
                .get::<Vec<Uuid>, _>("members")
                .into_iter()
                .filter(|r| seen.contains(r))
                .collect(),
            work_refs: Vec::new(),
            statistics: Value::Null,
            journey: Value::Null,
        })
        .collect();
    let structure_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_structure_source')IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    if structure_ready {
        let replaced: Vec<Uuid> =
            sqlx::query_scalar("SELECT topic_ref FROM linggan_topic_map_structure_source")
                .fetch_all(database.pool())
                .await?;
        for topic in &mut topics {
            if replaced.contains(&topic.topic_ref) {
                topic.lifecycle_state = "superseded".into();
            }
        }
    }
    let candidates = attach_research(database, q, &mut works, &mut topics).await?;
    // Accepted, evidenced classifications extend membership without replacing a definition.
    for t in &mut topics {
        for w in &works {
            if w.annotation.as_ref().is_some_and(|v| {
                v["topic_ref"].as_str() == Some(&t.topic_ref.to_string())
                    && v["definition_ref"].as_str() == Some(&t.definition_ref.to_string())
            }) && !t.direct_work_refs.contains(&w.work_ref)
            {
                t.direct_work_refs.push(w.work_ref)
            }
        }
    }
    for index in 0..topics.len() {
        let mut descendants = HashSet::from([topics[index].topic_ref]);
        loop {
            let before = descendants.len();
            for t in &topics {
                if t.parent_topic_ref.is_some_and(|p| descendants.contains(&p)) {
                    descendants.insert(t.topic_ref);
                }
            }
            if descendants.len() == before {
                break;
            }
        }
        topics[index].work_refs = topics
            .iter()
            .filter(|t| descendants.contains(&t.topic_ref))
            .flat_map(|t| t.direct_work_refs.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    for w in &mut works {
        w.topic_refs = topics
            .iter()
            .filter(|t| t.work_refs.contains(&w.work_ref))
            .map(|t| t.topic_ref)
            .collect();
    }
    let all_history_own: Vec<_> = works.iter().filter(|w| w.own).collect();
    let history_own_published = all_history_own
        .iter()
        .filter(|w| w.published_at.is_some() || w.published_at_source_text.is_some())
        .count();
    let history_own_unknown = all_history_own.len() - history_own_published;
    let history_own_refs: Vec<_> = all_history_own.iter().map(|w| w.work_ref).collect();
    let dates: Value = json!(
        works
            .iter()
            .map(|w| json!({"work_ref":w.work_ref,"published_at":w.published_at}))
            .collect::<Vec<_>>()
    );
    let window_refs: HashSet<Uuid> = if let Some(days) = q.window_days {
        sqlx::query_scalar("SELECT work_ref FROM jsonb_to_recordset($1) AS input(work_ref uuid,published_at timestamptz) WHERE published_at >= $2::timestamptz-($3::int*interval '1 day') AND published_at<=$2::timestamptz").bind(dates.clone()).bind(&as_of).bind(days).fetch_all(database.pool()).await?.into_iter().collect()
    } else {
        works.iter().map(|w| w.work_ref).collect()
    };
    let reference_days = q.reference_window_days.unwrap_or(30);
    let recent: HashSet<Uuid> = if reference_days == 0 {
        works.iter().map(|w| w.work_ref).collect()
    } else {
        sqlx::query_scalar("SELECT work_ref FROM jsonb_to_recordset($1) AS input(work_ref uuid,published_at timestamptz) WHERE published_at >= $2::timestamptz-($3::int*interval '1 day') AND published_at<=$2::timestamptz").bind(dates).bind(&as_of).bind(reference_days).fetch_all(database.pool()).await?.into_iter().collect()
    };
    let selected_refs = if let Some(topic) = q.topic_ref {
        Some(
            topics
                .iter()
                .find(|t| t.topic_ref == topic)
                .ok_or(TopicMapError::NotFound)?
                .work_refs
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
        )
    } else {
        None
    };
    let included = |w: &&TopicMapWork| {
        window_refs.contains(&w.work_ref)
            && q.overlay.as_ref().is_none_or(|p| w.overlays.contains(p))
            && q.path
                .as_ref()
                .is_none_or(|p| &w.path == p || w.path == "both")
    };
    for t in &mut topics {
        let tw: Vec<_> = works
            .iter()
            .filter(|w| t.work_refs.contains(&w.work_ref))
            .filter(included)
            .collect();
        t.statistics = statistics(&tw, &rules);
        let history: Vec<_> = works
            .iter()
            .filter(|w| t.work_refs.contains(&w.work_ref) && w.own)
            .collect();
        t.statistics["ownHistoryPublishedCount"] = json!(
            history
                .iter()
                .filter(|w| w.published_at.is_some() || w.published_at_source_text.is_some())
                .count()
        );
        t.statistics["ownHistoryPublicationUnknownCount"] = json!(
            history
                .iter()
                .filter(|w| w.published_at.is_none() && w.published_at_source_text.is_none())
                .count()
        );
        t.statistics["ownHistoryBreakoutCount"] =
            json!(history.iter().filter(|w| w.own_breakout).count());
        t.statistics["ownHistoricalWorkRefs"] =
            json!(history.iter().map(|w| w.work_ref).collect::<Vec<_>>());
        t.statistics["recentLowFollowerHighLikeCount"] = json!(
            works
                .iter()
                .filter(|w| t.work_refs.contains(&w.work_ref)
                    && recent.contains(&w.work_ref)
                    && !w.own
                    && w.follower_count.is_some_and(|f| f <= 1000)
                    && w.likes.is_some_and(|l| l >= 500))
                .count()
        );
        t.journey = journey(&tw);
        t.work_refs = tw.iter().map(|w| w.work_ref).collect();
    }
    let selected: Vec<_> = works
        .iter()
        .filter(included)
        .filter(|w| {
            selected_refs
                .as_ref()
                .is_none_or(|s| s.contains(&w.work_ref))
        })
        .collect();
    let stats = statistics(&selected, &rules);
    let journeys = journey(&selected);
    let sources = sources(&selected, &rules);
    let work_count = selected.len();
    let readable = selected.iter().filter(|w| w.readable).count();
    let pending = selected
        .iter()
        .filter(|w| w.main_stage == "pending")
        .count();
    let alternatives = read_alternatives(database, q, &works).await?;
    let mut changes:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('topicRef',t.topic_ref,'definitionRef',d.definition_ref,'definitionVersion',d.version,'lastViewedDefinitionRef',v.definition_ref,'lastViewedAt',v.created_at::text,'structureChanged',v.definition_ref IS NOT NULL AND v.definition_ref<>d.definition_ref,'notViewed',v.definition_ref IS NULL,'lastViewedWorkRefs',v.observed_work_refs) FROM linggan_topic_workspace t JOIN LATERAL(SELECT * FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true LEFT JOIN LATERAL(SELECT * FROM linggan_topic_map_viewed WHERE topic_ref=t.topic_ref AND ($1::uuid IS NULL OR domain_ref=$1) ORDER BY created_at DESC LIMIT 1)v ON true WHERE t.topic_ref=ANY($2)").bind(q.domain_ref).bind(topics.iter().map(|t|t.topic_ref).collect::<Vec<_>>()).fetch_all(database.pool()).await?;
    for change in &mut changes {
        let Some(topic) = topics
            .iter()
            .find(|t| change["topicRef"].as_str() == Some(&t.topic_ref.to_string()))
        else {
            continue;
        };
        let viewed: HashSet<Uuid> = change["lastViewedWorkRefs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().and_then(|v| v.parse().ok()))
            .collect();
        let unseen: Vec<_> = topic
            .work_refs
            .iter()
            .copied()
            .filter(|r| !viewed.contains(r))
            .collect();
        change["workSetDifferenceKnown"] = json!(change["lastViewedAt"].is_string());
        change["unviewedWorkRefs"] = json!(unseen);
        change["differenceKind"] = json!("material_or_classification_change_not_market_trend");
    }
    let mut scoped_works: Vec<_> = works
        .into_iter()
        .filter(|w| {
            window_refs.contains(&w.work_ref)
                && selected_refs
                    .as_ref()
                    .is_none_or(|s| s.contains(&w.work_ref))
                && q.overlay.as_ref().is_none_or(|p| w.overlays.contains(p))
                && q.path
                    .as_ref()
                    .is_none_or(|p| &w.path == p || w.path == "both")
        })
        .collect();
    scoped_works.sort_by(|a, b| {
        b.published_at
            .cmp(&a.published_at)
            .then_with(|| a.work_ref.cmp(&b.work_ref))
    });
    Ok(TopicMapSnapshot {
        domains,
        scope: json!({"domainRef":q.domain_ref,"topicRef":q.topic_ref,"platform":q.platform,"windowDays":q.window_days,"referenceWindowDays":reference_days,"asOf":as_of,"totalWorkCount":work_count,"loadedWorkCount":scoped_works.len(),"canonicalWorkCount":refs.len(),"readableWorkCount":readable,"unclassifiedWorkCount":pending,"readLimit":null,"ownIdentityState":if own_creators.iter().any(|v|v["active"]==true){"partial_observed"}else{"unknown"},"includeReference":q.include_reference,"ownHistoricalWorkRefs":history_own_refs,"ownHistoryPublishedCount":history_own_published,"ownHistoryPublicationUnknownCount":history_own_unknown,"recentReferenceWorkRefs":recent}),
        topics,
        works: scoped_works,
        statistics: stats,
        journey: journeys,
        sources,
        candidates,
        changes,
        alternatives,
        own_creators,
        method_version: METHOD_VERSION,
        source_boundary: "已接纳且当前可读的独立作品样本；领域用途去重，非平台或市场总体。我方覆盖仅限明确身份的已观察发布历史。",
    })
}

pub(crate) fn statistics(works: &[&TopicMapWork], rules: &[Value]) -> Value {
    let mut platforms = Vec::new();
    for platform in ["xhs", "douyin"] {
        let items: Vec<_> = works.iter().filter(|w| w.platform == platform).collect();
        if items.is_empty() {
            continue;
        }
        let mut likes: Vec<_> = items.iter().filter_map(|w| w.likes).collect();
        likes.sort_unstable();
        let threshold = rules
            .iter()
            .find(|r| r["platform"] == platform)
            .and_then(|r| r["likeThreshold"].as_i64());
        let mut authors = HashSet::new();
        for w in &items {
            if let Some(a) = &w.author_external_id {
                authors.insert(a);
            }
        }
        platforms.push(json!({"platform":platform,"workCount":items.len(),"knownLikeCount":likes.len(),"unknownLikeCount":items.len()-likes.len(),"medianLikes":quantile(&likes,0.5),"p90Likes":quantile(&likes,0.9),"highPerformanceCount":threshold.map(|n|items.iter().filter(|w|w.likes.is_some_and(|v|v>=n)).count()),"highPerformanceThreshold":threshold,"highPerformanceRuleVersion":rules.iter().find(|r|r["platform"]==platform).map(|r|r["version"].clone()),"highPerformanceRate":threshold.and_then(|n|(!likes.is_empty()).then(||likes.iter().filter(|v|**v>=n).count() as f64/likes.len() as f64)),"highPerformanceDenominator":likes.len(),"lowFollowerHighLikeCount":items.iter().filter(|w|w.follower_count.is_some_and(|f|f<=1000)&&w.likes.is_some_and(|l|l>=500)).count(),"lowFollowerHighLikeRule":{"maximumFollowers":1000,"minimumLikes":500},"unknownFollowerCount":items.iter().filter(|w|w.follower_count.is_none()).count(),"authorCount":authors.len()}));
    }
    let authors: HashSet<_> = works
        .iter()
        .filter_map(|w| w.author_external_id.as_ref().map(|a| (&w.platform, a)))
        .collect();
    json!({"workCount":works.len(),"platforms":platforms,"authorCount":authors.len(),"unknownAuthorCount":works.iter().filter(|w|w.author_external_id.is_none()).count(),"ownPublishedCount":works.iter().filter(|w|w.own&&(w.published_at.is_some()||w.published_at_source_text.is_some())).count(),"ownPublicationUnknownCount":works.iter().filter(|w|w.own&&w.published_at.is_none()&&w.published_at_source_text.is_none()).count(),"ownBreakoutCount":works.iter().filter(|w|w.own_breakout).count()})
}
fn quantile(values: &[i64], q: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let p = (values.len() - 1) as f64 * q;
    let lower = p.floor() as usize;
    let upper = p.ceil() as usize;
    Some(values[lower] as f64 + (values[upper] - values[lower]) as f64 * (p - lower as f64))
}
pub(crate) fn journey(works: &[&TopicMapWork]) -> Value {
    let readable: Vec<_> = works.iter().filter(|w| w.readable).collect();
    let denominator = readable.len();
    let row = |stage: &str, kind: &str| {
        let refs: Vec<_> = readable
            .iter()
            .filter(|w| match kind {
                "main" => w.main_stage == stage,
                "involved" => w.involved_stages.iter().any(|s| s == stage),
                _ => w.overlays.iter().any(|s| s == stage),
            })
            .map(|w| w.work_ref)
            .collect();
        json!({"stage":stage,"count":refs.len(),"workRefs":refs,"share":if denominator==0{None}else{Some(refs.len() as f64/denominator as f64)}})
    };
    json!({"denominator":denominator,"main":MAIN_STAGES.iter().chain(OTHER_STAGES.iter()).map(|s|row(s,"main")).collect::<Vec<_>>(),"involved":MAIN_STAGES.iter().map(|s|row(s,"involved")).collect::<Vec<_>>(),"overlays":OVERLAYS.iter().map(|s|row(s,"overlay")).collect::<Vec<_>>(),"involvedMayOverlap":true})
}
fn sources(works: &[&TopicMapWork], rules: &[Value]) -> Vec<Value> {
    let mut groups: BTreeMap<(String, Option<String>), Vec<&TopicMapWork>> = BTreeMap::new();
    for w in works {
        groups
            .entry((w.platform.clone(), w.author_external_id.clone()))
            .or_default()
            .push(w);
    }
    groups.into_iter().map(|((p,a),ws)|json!({"platform":p,"authorExternalId":a,"creatorDisplayName":ws.iter().find_map(|w|w.creator_display_name.as_ref()),"workCount":ws.len(),"workRefs":ws.iter().map(|w|w.work_ref).collect::<Vec<_>>(),"topicRefs":ws.iter().flat_map(|w|w.topic_refs.iter().copied()).collect::<BTreeSet<_>>(),"statistics":statistics(&ws,rules)})).collect()
}
async fn read_alternatives(
    database: &Database,
    q: &TopicMapQuery,
    works: &[TopicMapWork],
) -> Result<Vec<Value>, TopicMapError> {
    let rows=sqlx::query("SELECT a.*,a.created_at::text AS time,array(SELECT work_public_ref FROM linggan_topic_map_alternative_evidence WHERE alternative_ref=a.alternative_ref ORDER BY work_public_ref) AS refs,(SELECT jsonb_agg(jsonb_build_object('workRef',work_public_ref,'sourceManifest',source_manifest) ORDER BY work_public_ref) FROM linggan_topic_map_alternative_evidence WHERE alternative_ref=a.alternative_ref) AS manifests FROM linggan_topic_map_alternative a WHERE ($1::uuid IS NULL OR a.domain_ref=$1) AND ($2::uuid IS NULL OR a.topic_ref=$2) ORDER BY created_at DESC").bind(q.domain_ref).bind(q.topic_ref).fetch_all(database.pool()).await?;
    let mut results = Vec::new();
    for row in rows {
        let alternative: Uuid = row.get("alternative_ref");
        let saved =
            super::saved::read_saved_alternative(database, row.get("domain_ref"), alternative)
                .await?;
        results.push(json!({"alternativeRef":saved.alternative_ref,"domainRef":saved.domain_ref,"topicRef":saved.topic_ref,"definitionRef":saved.definition_ref,"kind":saved.kind,"title":saved.title,"angle":saved.angle,"rationale":saved.rationale,"methodVersion":saved.method_version,"evidenceWorkRefs":row.get::<Vec<Uuid>,_>("refs"),"createdAt":row.get::<String,_>("time"),"sourceState":saved.source_state,"researchManifest":saved.research_manifest,"sourceManifests":saved.source_manifests,"trackingEnabled":false}));
    }
    let _ = works; // Original-version qualification is independent of the overview time window.
    Ok(results)
}

async fn attach_research(
    database: &Database,
    q: &TopicMapQuery,
    works: &mut [TopicMapWork],
    topics: &mut [TopicMapTopic],
) -> Result<Vec<Value>, TopicMapError> {
    let installed: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_research_result')IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    let mut candidates = Vec::new();
    // All annotations, including human entries, lose applicability when cited fragments change.
    let domains: Vec<Uuid> = sqlx::query_scalar(
        "SELECT domain_ref FROM observation_domain WHERE ($1::uuid IS NULL OR domain_ref=$1)",
    )
    .bind(q.domain_ref)
    .fetch_all(database.pool())
    .await?;
    let mut canonical = HashMap::new();
    for d in domains {
        let scope = serde_json::from_value(json!({"domain":d}))
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
        let data = linggan_evidence::creator_discovery::load(database, &scope)
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
        for w in data.works {
            canonical.insert(w.work_ref, w.fragments);
        }
    }
    for w in works.iter_mut() {
        if let Some(annotation) = w.annotation.as_ref() {
            let valid = canonical.get(&w.work_ref).is_some_and(|fragments| {
                annotation["evidence_citations"]
                    .as_array()
                    .is_some_and(|citations| {
                        !citations.is_empty()
                            && citations.iter().all(|c| {
                                fragments.iter().any(|f| {
                                    c["fragmentId"].as_str() == Some(&f.fragment_id)
                                        && c["sourceRef"].as_str()
                                            == Some(&f.source_ref.to_string())
                                        && c["field"].as_str() == Some(&f.field)
                                })
                            })
                    })
            }) && annotation["definition_ref"]
                .as_str()
                .is_none_or(|def| topics.iter().any(|t| t.definition_ref.to_string() == def));
            if !valid {
                w.annotation = None;
                w.main_stage = "pending".into();
                w.involved_stages.clear();
                w.overlays.clear();
                w.path = "unknown".into();
            }
        }
    }
    let memberships=sqlx::query("SELECT topic_ref,definition_ref,work_public_ref,evidence_citations FROM linggan_topic_map_membership WHERE ($1::uuid IS NULL OR domain_ref=$1)").bind(q.domain_ref).fetch_all(database.pool()).await?;
    for row in memberships {
        let reference: Uuid = row.get("work_public_ref");
        let def: Uuid = row.get("definition_ref");
        let topic: Uuid = row.get("topic_ref");
        let citations: Value = row.get("evidence_citations");
        let valid = canonical.get(&reference).is_some_and(|fragments| {
            citations.as_array().is_some_and(|cs| {
                !cs.is_empty()
                    && cs.iter().all(|c| {
                        fragments.iter().any(|f| {
                            c["fragmentId"].as_str() == Some(&f.fragment_id)
                                && c["sourceRef"].as_str() == Some(&f.source_ref.to_string())
                        })
                    })
            })
        });
        if valid {
            if let Some(t) = topics
                .iter_mut()
                .find(|t| t.topic_ref == topic && t.definition_ref == def)
            {
                if !t.direct_work_refs.contains(&reference)
                    && works.iter().any(|w| w.work_ref == reference && w.readable)
                {
                    t.direct_work_refs.push(reference);
                }
            }
        }
    }
    if !installed {
        return Ok(candidates);
    }
    let rows=sqlx::query("SELECT DISTINCT ON(r.work_public_ref) r.work_public_ref,r.domain_ref,r.output_json,r.method_version,r.result_ref,r.created_at::text AS time,t.input_refs,run.config_ref FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_task t ON t.domain_ref=r.domain_ref AND t.work_public_ref=r.work_public_ref AND t.input_hash=r.input_hash JOIN linggan_topic_map_research_run run ON run.run_ref=t.run_ref WHERE ($1::uuid IS NULL OR r.domain_ref=$1) ORDER BY r.work_public_ref,r.created_at DESC,r.result_ref DESC").bind(q.domain_ref).fetch_all(database.pool()).await?;
    let allowed: Vec<_> = topics.iter().map(|t| t.topic_ref).collect();
    let mut research_inputs: HashMap<(Uuid, Uuid), Vec<crate::topic_map_research::ResearchInput>> =
        HashMap::new();
    for r in rows {
        let domain: Uuid = r.get("domain_ref");
        let config: Uuid = r.get("config_ref");
        if !research_inputs.contains_key(&(domain, config)) {
            let current = crate::topic_map_research::load_inputs(database, domain, config)
                .await
                .map_err(|e| TopicMapError::Source(e.to_string()))?;
            research_inputs.insert((domain, config), current);
        }
        let work_ref: Uuid = r.get("work_public_ref");
        let Some(w) = works
            .iter_mut()
            .find(|w| w.work_ref == work_ref && w.readable)
        else {
            continue;
        };
        let manifest: Value = r.get("input_refs");
        let inputs = &research_inputs[&(domain, config)];
        let Some(primary) = inputs.iter().find(|i| i.work.work_ref == work_ref) else {
            continue;
        };
        let context: Vec<Uuid> = manifest["contextWorkRefs"]
            .as_array()
            .map(|vs| {
                vs.iter()
                    .filter_map(|v| v.as_str().and_then(|v| v.parse().ok()))
                    .collect()
            })
            .unwrap_or_default();
        if context.len() > 10
            || context
                .iter()
                .any(|r| !inputs.iter().any(|i| i.work.work_ref == *r))
        {
            continue;
        }
        let current =
            crate::topic_map_research::with_comparison_context(primary.clone(), inputs, &context);
        let fragments = &current.fragments;
        // Physical recapture IDs may change; applicability follows current qualified text and
        // stable fragment identity. Every frozen context work is resolved within this domain.
        let same = manifest["fragments"].as_array().is_some_and(|refs| {
            refs.len() == fragments.len()
                && refs.iter().all(|reference| {
                    fragments.iter().any(|f| {
                        reference["fragmentId"].as_str() == Some(&f.fragment_id)
                            && reference["field"].as_str() == Some(&f.field)
                            && reference["textHash"].as_str()
                                == Some(&linggan_evidence::creator_discovery::hash(&f.text))
                    })
                })
        });
        let output: Value = r.get("output_json");
        let Ok(typed) = serde_json::from_value::<crate::topic_map_research_analysis::ResearchOutput>(
            output.clone(),
        ) else {
            continue;
        };
        let same_defs = manifest["topics"].as_array().is_some_and(|defs| {
            defs.iter().all(|d| {
                topics
                    .iter()
                    .any(|t| d["definitionRef"].as_str() == Some(&t.definition_ref.to_string()))
            })
        });
        if !same
            || !same_defs
            || crate::topic_map_research_analysis::validate_output(&typed, fragments, &allowed)
                .is_err()
        {
            continue;
        }
        for discussion in typed.discussions.iter().filter(|d| d.topic_ref.is_none()) {
            candidates.push(json!({"label":discussion.label,"workRef":work_ref,"evidence":discussion.evidence,"resultRef":r.get::<Uuid,_>("result_ref"),"methodVersion":r.get::<String,_>("method_version"),"state":"candidate","formalRelease":false}));
        }
        w.research = Some(
            json!({"resultRef":r.get::<Uuid,_>("result_ref"),"methodVersion":r.get::<String,_>("method_version"),"createdAt":r.get::<String,_>("time"),"output":output,"fragments":fragments}),
        );
    }
    Ok(candidates)
}
