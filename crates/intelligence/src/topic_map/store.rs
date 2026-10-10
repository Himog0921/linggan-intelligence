use super::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;

fn text(value: &str, max: usize) -> Result<(), TopicMapError> {
    if value.trim().is_empty() || value.chars().count() > max {
        Err(TopicMapError::Invalid("invalid text"))
    } else {
        Ok(())
    }
}
fn key(value: &str) -> Result<(), TopicMapError> {
    if !(8..=128).contains(&value.len())
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
    {
        Err(TopicMapError::Invalid("invalid idempotency key"))
    } else {
        Ok(())
    }
}
fn platform(value: &str) -> Result<(), TopicMapError> {
    if matches!(value, "xhs" | "douyin") {
        Ok(())
    } else {
        Err(TopicMapError::Invalid("invalid platform"))
    }
}
fn hash(value: &impl Serialize) -> Result<String, TopicMapError> {
    let encoded = serde_json::to_vec(value).map_err(|e| TopicMapError::Source(e.to_string()))?;
    Ok(Sha256::digest(encoded)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
async fn lock(tx: &mut Transaction<'_, Postgres>, scope: &str) -> Result<(), TopicMapError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(scope)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    key: &str,
    hash: &str,
) -> Result<Option<TopicMapReceipt>, TopicMapError> {
    let row=sqlx::query("SELECT *,persisted_at::text AS time FROM linggan_topic_map_receipt WHERE idempotency_key=$1").bind(key).fetch_optional(&mut **tx).await?;
    match row {
        Some(row) => {
            if row.get::<String, _>("request_sha256") != hash {
                return Err(TopicMapError::Conflict);
            }
            Ok(Some(TopicMapReceipt {
                receipt_ref: row.get("receipt_ref"),
                action: row.get("action"),
                subject_ref: row.get("subject_ref"),
                revision: row.get("revision"),
                persisted_at: row.get("time"),
            }))
        }
        None => Ok(None),
    }
}
async fn receipt(
    tx: &mut Transaction<'_, Postgres>,
    key: &str,
    hash: &str,
    action: &str,
    subject: Option<Uuid>,
    version: i32,
) -> Result<TopicMapReceipt, TopicMapError> {
    let reference = Uuid::new_v4();
    let time=sqlx::query_scalar("INSERT INTO linggan_topic_map_receipt(receipt_ref,idempotency_key,request_sha256,action,subject_ref,revision) VALUES($1,$2,$3,$4,$5,$6) RETURNING persisted_at::text").bind(reference).bind(key).bind(hash).bind(action).bind(subject).bind(version).fetch_one(&mut **tx).await?;
    Ok(TopicMapReceipt {
        receipt_ref: reference,
        action: action.into(),
        subject_ref: subject,
        revision: version,
        persisted_at: time,
    })
}
async fn domain(tx: &mut Transaction<'_, Postgres>, reference: Uuid) -> Result<(), TopicMapError> {
    let found: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(reference)
            .fetch_one(&mut **tx)
            .await?;
    if found {
        Ok(())
    } else {
        Err(TopicMapError::NotFound)
    }
}
async fn work(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    reference: Uuid,
) -> Result<(), TopicMapError> {
    let found:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=$2)").bind(domain).bind(reference).fetch_one(&mut **tx).await?;
    if found {
        Ok(())
    } else {
        Err(TopicMapError::Invalid("work outside domain"))
    }
}
async fn definition(
    tx: &mut Transaction<'_, Postgres>,
    topic: Uuid,
    definition: Uuid,
) -> Result<(), TopicMapError> {
    let found:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_definition WHERE topic_ref=$1 AND definition_ref=$2)").bind(topic).bind(definition).fetch_one(&mut **tx).await?;
    if found {
        Ok(())
    } else {
        Err(TopicMapError::Invalid(
            "definition does not belong to topic",
        ))
    }
}

async fn topic_scope(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    topic: Uuid,
) -> Result<(), TopicMapError> {
    let allowed:Option<Uuid>=sqlx::query_scalar("SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=$1 ORDER BY version DESC LIMIT 1").bind(topic).fetch_optional(&mut **tx).await?;
    if allowed == Some(domain) {
        Ok(())
    } else {
        Err(TopicMapError::Invalid("topic outside domain"))
    }
}

pub async fn save_topic_map_command(
    database: &Database,
    request: &TopicMapCommand,
) -> Result<TopicMapReceipt, TopicMapError> {
    let (idempotency, domain_ref) = match request {
        TopicMapCommand::OwnCreator {
            idempotency_key,
            domain_ref,
            ..
        }
        | TopicMapCommand::Breakout {
            idempotency_key,
            domain_ref,
            ..
        }
        | TopicMapCommand::SaveAlternative {
            idempotency_key,
            domain_ref,
            ..
        }
        | TopicMapCommand::BindTopic {
            idempotency_key,
            domain_ref,
            ..
        }
        | TopicMapCommand::PerformanceRule {
            idempotency_key,
            domain_ref,
            ..
        }
        | TopicMapCommand::Viewed {
            idempotency_key,
            domain_ref,
            ..
        } => (idempotency_key, *domain_ref),
    };
    key(idempotency)?;
    let digest = hash(request)?;
    // Research qualification may hydrate canonical sources. Complete it before owning a
    // pooled writer connection; replay below remains valid even when sources later change.
    let research_sources_current = if let TopicMapCommand::SaveAlternative {
        research_result_ref: Some(result),
        ..
    } = request
    {
        Some(
            crate::topic_map_research::research_result_sources_current(database, *result)
                .await
                .map_err(|e| TopicMapError::Source(e.to_string()))?,
        )
    } else {
        None
    };
    let unavailable_definitions = if matches!(request, TopicMapCommand::SaveAlternative { .. }) {
        crate::topic_map_core::unavailable_definitions(database, Some(domain_ref))
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?
    } else {
        std::collections::HashSet::new()
    };
    let mut tx = database.pool().begin().await?;
    lock(&mut tx, "topic-map:commands").await?;
    if let Some(found) = replay(&mut tx, idempotency, &digest).await? {
        tx.commit().await?;
        return Ok(found);
    }
    domain(&mut tx, domain_ref).await?;
    let result = match request {
        TopicMapCommand::OwnCreator {
            platform: p,
            author_external_id,
            active,
            ..
        } => {
            platform(p)?;
            text(author_external_id, 256)?;
            let r = receipt(&mut tx, idempotency, &digest, "ownCreator", None, 1).await?;
            sqlx::query("INSERT INTO linggan_topic_map_own_creator(membership_ref,domain_ref,platform,author_external_id,active,receipt_ref) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::new_v4()).bind(domain_ref).bind(p).bind(author_external_id).bind(active).bind(r.receipt_ref).execute(&mut *tx).await?;
            r
        }
        TopicMapCommand::Breakout {
            work_ref, marked, ..
        } => {
            work(&mut tx, domain_ref, *work_ref).await?;
            let own:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_content c JOIN linggan_material_content_author a ON a.content_public_ref=c.public_ref JOIN (SELECT DISTINCT ON(platform,author_external_id) * FROM linggan_topic_map_own_creator WHERE domain_ref=$1 ORDER BY platform,author_external_id,created_at DESC,membership_ref DESC) own ON own.platform=c.platform AND own.author_external_id=a.author_external_id AND own.active WHERE c.public_ref=$2)").bind(domain_ref).bind(work_ref).fetch_one(&mut *tx).await?;
            if !own {
                return Err(TopicMapError::Invalid(
                    "breakout requires explicit own creator identity",
                ));
            }
            let r = receipt(
                &mut tx,
                idempotency,
                &digest,
                "breakout",
                Some(*work_ref),
                1,
            )
            .await?;
            sqlx::query("INSERT INTO linggan_topic_map_breakout(marking_ref,domain_ref,work_public_ref,marked,receipt_ref) VALUES($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(domain_ref).bind(work_ref).bind(marked).bind(r.receipt_ref).execute(&mut *tx).await?;
            r
        }
        TopicMapCommand::SaveAlternative {
            topic_ref,
            definition_ref,
            title,
            angle,
            rationale,
            evidence_work_refs,
            method_version,
            research_result_ref,
            research_angle_index,
            research_opportunity_index,
            ..
        } => {
            topic_scope(&mut tx, domain_ref, *topic_ref).await?;
            text(title, 120)?;
            text(angle, 2000)?;
            text(rationale, 2000)?;
            text(method_version, 120)?;
            definition(&mut tx, *topic_ref, *definition_ref).await?;
            let refs: BTreeSet<_> = evidence_work_refs.iter().copied().collect();
            if refs.is_empty() || refs.len() > 10 || refs.len() != evidence_work_refs.len() {
                return Err(TopicMapError::Invalid(
                    "alternative requires 1 to 10 unique evidence works",
                ));
            }
            for reference in &refs {
                work(&mut tx, domain_ref, *reference).await?;
            }
            let mut research_manifest = match (
                research_result_ref,
                research_angle_index,
                research_opportunity_index,
            ) {
                (Some(result), Some(index), None) | (Some(result), None, Some(index)) => {
                    if research_sources_current != Some(true) {
                        return Err(TopicMapError::Invalid(
                            "research source changed or unavailable",
                        ));
                    }
                    let row = sqlx::query("SELECT r.work_public_ref,r.method_version,r.output_json,t.input_refs FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_request request USING(invocation_ref) JOIN linggan_topic_map_research_task t USING(task_ref) WHERE r.result_ref=$1 AND r.domain_ref=$2").bind(result).bind(domain_ref).fetch_optional(&mut *tx).await?.ok_or(TopicMapError::NotFound)?;
                    if !refs.contains(&row.get::<Uuid, _>("work_public_ref"))
                        || row.get::<String, _>("method_version") != *method_version
                    {
                        return Err(TopicMapError::Invalid(
                            "alternative research lineage mismatch",
                        ));
                    }
                    let output: Value = row.get("output_json");
                    let collection = if research_opportunity_index.is_some() {
                        "productOpportunities"
                    } else {
                        "angles"
                    };
                    let angle = output[collection]
                        .as_array()
                        .and_then(|angles| angles.get(*index))
                        .ok_or(TopicMapError::Invalid("research angle does not exist"))?;
                    json!({"resultRef":result,"angleIndex":research_angle_index,"opportunityIndex":research_opportunity_index,"citations":angle["evidence"],"inputRefs":row.get::<Value,_>("input_refs")})
                }
                (None, None, None) => json!({}),
                _ => {
                    return Err(TopicMapError::Invalid(
                        "research result requires exactly one angle or opportunity index",
                    ));
                }
            };
            if !super::saved::definitions_available(
                &mut tx,
                domain_ref,
                *definition_ref,
                &research_manifest,
                &unavailable_definitions,
            )
            .await?
            {
                return Err(TopicMapError::Invalid(
                    "alternative definition source unavailable",
                ));
            }
            let performance_rules: Value = sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('platform',rule.platform,'version',rule.version,'likeThreshold',rule.like_threshold,'ruleRef',rule.rule_ref)), '[]'::jsonb) FROM (SELECT DISTINCT ON(platform) platform,version,like_threshold,rule_ref FROM linggan_topic_map_performance_rule WHERE domain_ref=$1 ORDER BY platform,version DESC)rule").bind(domain_ref).fetch_one(&mut *tx).await?;
            research_manifest["performanceRules"] = performance_rules;
            let alternative = Uuid::new_v4();
            let r = receipt(
                &mut tx,
                idempotency,
                &digest,
                "saveAlternative",
                Some(alternative),
                1,
            )
            .await?;
            sqlx::query("INSERT INTO linggan_topic_map_alternative(alternative_ref,domain_ref,topic_ref,definition_ref,title,angle,rationale,method_version,receipt_ref,research_manifest,kind) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(alternative).bind(domain_ref).bind(topic_ref).bind(definition_ref).bind(title.trim()).bind(angle.trim()).bind(rationale.trim()).bind(method_version).bind(r.receipt_ref).bind(safe_research_manifest(&research_manifest)).bind(if research_opportunity_index.is_some(){"product_research"}else{"angle"}).execute(&mut *tx).await?;
            for reference in refs {
                let resource =
                    linggan_evidence::read_work_resource_in_transaction(&mut tx, reference)
                        .await
                        .map_err(|e| TopicMapError::Source(e.to_string()))?
                        .ok_or(TopicMapError::NotFound)?;
                if resource.summary.restriction_state == "WITHDRAWN_OR_RESTRICTED" {
                    return Err(TopicMapError::Invalid("alternative source unavailable"));
                }
                let mut manifest = canonical_source_manifest(&resource);
                manifest["frozenFragments"] = serde_json::to_value(
                    linggan_evidence::frozen_work_fragment_manifest_in_transaction(
                        &mut tx, reference,
                    )
                    .await
                    .map_err(|e| TopicMapError::Source(e.to_string()))?,
                )
                .map_err(|e| TopicMapError::Source(e.to_string()))?;
                sqlx::query("INSERT INTO linggan_topic_map_alternative_evidence(alternative_ref,work_public_ref,source_manifest) VALUES($1,$2,$3)").bind(alternative).bind(reference).bind(manifest).execute(&mut *tx).await?;
            }
            r
        }
        TopicMapCommand::BindTopic {
            topic_ref,
            parent_topic_ref,
            expected_version,
            ..
        } => {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM linggan_topic_workspace WHERE topic_ref=$1)",
            )
            .bind(topic_ref)
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                return Err(TopicMapError::NotFound);
            }
            let latest=sqlx::query("SELECT version,domain_ref FROM linggan_topic_map_binding WHERE topic_ref=$1 ORDER BY version DESC LIMIT 1").bind(topic_ref).fetch_optional(&mut *tx).await?;
            let actual: Option<i32> = latest.as_ref().map(|r| r.get("version"));
            if &actual != expected_version {
                return Err(TopicMapError::Conflict);
            }
            if latest
                .as_ref()
                .is_some_and(|r| r.get::<Uuid, _>("domain_ref") != domain_ref)
            {
                return Err(TopicMapError::Invalid("topic domain cannot change"));
            }
            let structure_ready: bool = sqlx::query_scalar(
                "SELECT to_regclass('linggan_topic_map_structure_source')IS NOT NULL",
            )
            .fetch_one(&mut *tx)
            .await?;
            if structure_ready {
                let superseded:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=$1 OR topic_ref=$2)").bind(topic_ref).bind(parent_topic_ref).fetch_one(&mut *tx).await?;
                if superseded {
                    return Err(TopicMapError::Invalid(
                        "superseded topic cannot be moved or used as a parent",
                    ));
                }
            }
            if let Some(parent) = parent_topic_ref {
                let cycle:bool=sqlx::query_scalar("WITH RECURSIVE current AS(SELECT DISTINCT ON(topic_ref) topic_ref,parent_topic_ref,domain_ref FROM linggan_topic_map_binding ORDER BY topic_ref,version DESC), ancestors AS(SELECT topic_ref,parent_topic_ref FROM current WHERE topic_ref=$1 AND domain_ref=$3 UNION SELECT c.topic_ref,c.parent_topic_ref FROM current c JOIN ancestors a ON c.topic_ref=a.parent_topic_ref) SELECT $1=$2 OR EXISTS(SELECT 1 FROM ancestors WHERE topic_ref=$2) OR NOT EXISTS(SELECT 1 FROM current WHERE topic_ref=$1 AND domain_ref=$3)").bind(parent).bind(topic_ref).bind(domain_ref).fetch_one(&mut *tx).await?;
                if cycle {
                    return Err(TopicMapError::Invalid("invalid parent or hierarchy cycle"));
                }
            }
            let version = actual.unwrap_or(0) + 1;
            let r = receipt(
                &mut tx,
                idempotency,
                &digest,
                "bindTopic",
                Some(*topic_ref),
                version,
            )
            .await?;
            sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::new_v4()).bind(topic_ref).bind(domain_ref).bind(parent_topic_ref).bind(version).bind(r.receipt_ref).execute(&mut *tx).await?;
            r
        }
        TopicMapCommand::PerformanceRule {
            platform: p,
            like_threshold,
            expected_version,
            ..
        } => {
            platform(p)?;
            if *like_threshold <= 0 {
                return Err(TopicMapError::Invalid("threshold must be positive"));
            }
            let actual:Option<i32>=sqlx::query_scalar("SELECT max(version) FROM linggan_topic_map_performance_rule WHERE domain_ref=$1 AND platform=$2").bind(domain_ref).bind(p).fetch_one(&mut *tx).await?;
            if &actual != expected_version {
                return Err(TopicMapError::Conflict);
            }
            let version = actual.unwrap_or(0) + 1;
            let r = receipt(
                &mut tx,
                idempotency,
                &digest,
                "performanceRule",
                None,
                version,
            )
            .await?;
            sqlx::query("INSERT INTO linggan_topic_map_performance_rule(rule_ref,domain_ref,platform,like_threshold,version,receipt_ref) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::new_v4()).bind(domain_ref).bind(p).bind(like_threshold).bind(version).bind(r.receipt_ref).execute(&mut *tx).await?;
            r
        }
        TopicMapCommand::Viewed {
            topic_ref,
            definition_ref,
            work_refs,
            ..
        } => {
            if work_refs.len() > 1000 {
                return Err(TopicMapError::Invalid("too many viewed work references"));
            }
            for reference in work_refs {
                work(&mut tx, domain_ref, *reference).await?;
            }

            topic_scope(&mut tx, domain_ref, *topic_ref).await?;
            definition(&mut tx, *topic_ref, *definition_ref).await?;
            let r = receipt(&mut tx, idempotency, &digest, "viewed", Some(*topic_ref), 1).await?;
            sqlx::query("INSERT INTO linggan_topic_map_viewed(view_ref,domain_ref,topic_ref,definition_ref,receipt_ref,observed_work_refs) VALUES($1,$2,$3,$4,$5,$6)").bind(Uuid::new_v4()).bind(domain_ref).bind(topic_ref).bind(definition_ref).bind(r.receipt_ref).bind(work_refs).execute(&mut *tx).await?;
            r
        }
    };
    tx.commit().await?;
    Ok(result)
}

pub async fn append_work_annotation(
    database: &Database,
    request: &TopicMapAnnotationRequest,
) -> Result<TopicMapReceipt, TopicMapError> {
    key(&request.idempotency_key)?;
    text(&request.rationale, 2000)?;
    text(&request.method_version, 120)?;
    if !MAIN_STAGES.contains(&request.main_stage.as_str())
        && !OTHER_STAGES[..3].contains(&request.main_stage.as_str())
        || request
            .involved_stages
            .iter()
            .any(|v| !MAIN_STAGES.contains(&v.as_str()))
        || request
            .overlays
            .iter()
            .any(|v| !OVERLAYS.contains(&v.as_str()))
        || !matches!(
            request.path.as_str(),
            "family" | "adult" | "both" | "unknown"
        )
        || request.evidence_citations.is_empty()
        || request.evidence_citations.len() > 30
    {
        return Err(TopicMapError::Invalid("invalid evidenced annotation"));
    }
    if request.topic_ref.is_some() != request.definition_ref.is_some() {
        return Err(TopicMapError::Invalid(
            "topic and definition must travel together",
        ));
    }
    // Revalidate citation identities against the canonical fragment owner at the write boundary.
    let scope: linggan_contracts::creator_discovery::CreatorScope =
        serde_json::from_value(serde_json::json!({"domain":request.domain_ref}))
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
    let data = linggan_evidence::creator_discovery::load(database, &scope)
        .await
        .map_err(|e| TopicMapError::Source(e.to_string()))?;
    let source = data
        .works
        .iter()
        .find(|w| w.work_ref == request.work_public_ref)
        .ok_or(TopicMapError::NotFound)?;
    if request.evidence_citations.iter().any(|citation| {
        !source.fragments.iter().any(|fragment| {
            fragment.fragment_id == citation.fragment_id
                && fragment.source_ref == citation.source_ref
                && fragment.field == citation.field
        })
    }) {
        return Err(TopicMapError::Invalid(
            "citation is not a current canonical work fragment",
        ));
    }
    let mut tx = database.pool().begin().await?;
    let r = append_work_annotation_in(&mut tx, request).await?;
    tx.commit().await?;
    Ok(r)
}

/// Internal acceptance seam. Caller has validated typed output against canonical current fragments;
/// this keeps the derived result, invocation settlement and annotation atomic.
pub(crate) async fn append_work_annotation_in(
    tx: &mut Transaction<'_, Postgres>,
    request: &TopicMapAnnotationRequest,
) -> Result<TopicMapReceipt, TopicMapError> {
    let digest = hash(request)?;
    lock(tx, "topic-map:commands").await?;
    if let Some(r) = replay(tx, &request.idempotency_key, &digest).await? {
        return Ok(r);
    }
    work(tx, request.domain_ref, request.work_public_ref).await?;
    if let (Some(topic), Some(def)) = (request.topic_ref, request.definition_ref) {
        topic_scope(tx, request.domain_ref, topic).await?;
        definition(tx, topic, def).await?;
    }
    let reference = Uuid::new_v4();
    let r = receipt(
        tx,
        &request.idempotency_key,
        &digest,
        "annotation",
        Some(reference),
        1,
    )
    .await?;
    sqlx::query("INSERT INTO linggan_topic_map_work_annotation(annotation_ref,domain_ref,work_public_ref,topic_ref,definition_ref,method_version,main_stage,involved_stages,overlays,path,rationale,evidence_citations,receipt_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)").bind(reference).bind(request.domain_ref).bind(request.work_public_ref).bind(request.topic_ref).bind(request.definition_ref).bind(&request.method_version).bind(&request.main_stage).bind(&request.involved_stages).bind(&request.overlays).bind(&request.path).bind(&request.rationale).bind(serde_json::to_value(&request.evidence_citations).map_err(|e|TopicMapError::Source(e.to_string()))?).bind(r.receipt_ref).execute(&mut **tx).await?;
    Ok(r)
}

/// Proposals share Topic identity/definitions, but never impersonate an adjudicated release.
/// Only the research acceptor calls this after exact current-source citation validation.
pub(crate) async fn accept_topic_map_candidates_in(
    tx: &mut Transaction<'_, Postgres>,
    domain_ref: Uuid,
    work_ref: Uuid,
    proposals: &[TopicMapCandidateRequest],
) -> Result<Vec<Uuid>, TopicMapError> {
    work(tx, domain_ref, work_ref).await?;
    if proposals.len() > 8 {
        return Err(TopicMapError::Invalid("too many candidate discussions"));
    }
    let mut accepted = Vec::new();
    for proposal in proposals {
        text(&proposal.label, 120)?;
        if proposal.evidence_citations.is_empty() {
            return Err(TopicMapError::Invalid("candidate requires evidence"));
        }
        let label = proposal.label.trim();
        text(&proposal.definition_text, 2000)?;
        let identity = crate::topic_map_core::concept_identity(
            &proposal.definition_text,
            &proposal.inclusion_criteria,
            &proposal.exclusion_criteria,
        );
        let stable = hash(&(domain_ref, &identity))?;
        lock(tx, &format!("topic-map:candidate:{stable}")).await?;
        let existing=sqlx::query("SELECT t.topic_ref,d.definition_ref FROM linggan_topic_workspace t JOIN LATERAL(SELECT * FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true JOIN LATERAL(SELECT * FROM linggan_topic_map_binding WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE b.domain_ref=$1 AND (($2::uuid IS NOT NULL AND t.topic_ref=$2) OR ($2::uuid IS NULL AND EXISTS(SELECT 1 FROM linggan_topic_map_concept_rule rule WHERE rule.definition_ref=d.definition_ref AND rule.identity_hash=$3 AND d.definition_ref=ANY($4)))) AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=t.topic_ref) ORDER BY t.created_at,t.topic_ref LIMIT 1").bind(domain_ref).bind(proposal.topic_ref).bind(&identity).bind(&proposal.reuse_definition_refs).fetch_optional(&mut **tx).await?;
        let (topic_ref, definition_ref) = if let Some(row) = existing {
            (row.get("topic_ref"), row.get("definition_ref"))
        } else {
            if proposal.topic_ref.is_some() {
                return Err(TopicMapError::Invalid(
                    "classified topic outside current domain",
                ));
            }
            let topic = Uuid::new_v4();
            let definition = Uuid::new_v4();
            let run = Uuid::new_v4();
            let pack = Uuid::new_v4();
            let canonical = format!("candidate-{}", topic.simple());
            sqlx::query("INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key)VALUES($1,$2,$3)").bind(topic).bind(format!("domain-{}",domain_ref.simple())).bind(canonical).execute(&mut **tx).await?;
            sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state)VALUES($1,$2,1,$3,$4,'candidate')").bind(definition).bind(topic).bind(label).bind(&proposal.definition_text).execute(&mut **tx).await?;
            sqlx::query("INSERT INTO linggan_topic_classification_run(classification_run_ref,definition_ref,run_kind,run_state,adjudication_note)VALUES($1,$2,'machine_proposed','completed',$3)").bind(run).bind(definition).bind("引用有效材料的机器候选；未作为人工裁定或正式定义发布。").execute(&mut **tx).await?;
            sqlx::query("INSERT INTO linggan_topic_material_pack(material_pack_ref,classification_run_ref,source_boundary)VALUES($1,$2,$3)").bind(pack).bind(run).bind("仅当前已接纳作品；少量样本可形成候选，不能外推市场。").execute(&mut **tx).await?;
            sqlx::query("INSERT INTO linggan_topic_material_member(classification_run_ref,work_public_ref,role,rationale,ordinal)VALUES($1,$2,$3,$4,1)").bind(run).bind(work_ref).bind(match proposal.evidence_role.as_str(){"challenge"=>"challenge","context"=>"boundary",_=>"support"}).bind("本次证据讨论该候选，观点角色与精确依据见讨论归属记录。").execute(&mut **tx).await?;
            sqlx::query("INSERT INTO linggan_topic_map_concept_rule(definition_ref,domain_ref,identity_hash,inclusion_criteria,exclusion_criteria,method_version,invocation_ref)VALUES($1,$2,$3,$4,$5,$6,$7)").bind(definition).bind(domain_ref).bind(&identity).bind(&proposal.inclusion_criteria).bind(&proposal.exclusion_criteria).bind(crate::topic_map_core::METHOD_VERSION).bind(proposal.invocation_ref).execute(&mut **tx).await?;
            let idempotency = format!("topic-map-candidate:{}", topic.simple());
            let r = receipt(tx, &idempotency, &stable, "candidate", Some(topic), 1).await?;
            sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref)VALUES($1,$2,$3,NULL,1,$4)").bind(Uuid::new_v4()).bind(topic).bind(domain_ref).bind(r.receipt_ref).execute(&mut **tx).await?;
            (topic, definition)
        };
        sqlx::query("INSERT INTO linggan_topic_map_membership(membership_ref,domain_ref,topic_ref,definition_ref,work_public_ref,method_version,evidence_citations)VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(definition_ref,work_public_ref)DO NOTHING").bind(Uuid::new_v4()).bind(domain_ref).bind(topic_ref).bind(definition_ref).bind(work_ref).bind(crate::topic_map_research_analysis::METHOD_VERSION).bind(serde_json::to_value(&proposal.evidence_citations).map_err(|e|TopicMapError::Source(e.to_string()))?).execute(&mut **tx).await?;
        accepted.push(topic_ref);
    }
    Ok(accepted)
}
