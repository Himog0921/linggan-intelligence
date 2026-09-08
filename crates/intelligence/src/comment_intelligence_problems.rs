//! Incremental derived vocabulary and stable problem identities. Exact definition equality
//! is deliberately a narrow rule; similarity only recalls candidates and never merges IDs.

use crate::comment_intelligence_actions::{
    lock_domain, problem_metadata, problem_snapshot, problem_version, rejected,
};
use linggan_storage_postgres::{Database, StorageError};
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[path = "comment_intelligence_vocabulary.rs"]
mod vocabulary;
pub use vocabulary::{
    DefinitionEmbeddingPort, DefinitionVector, comment_terms, definition_key,
    recall_vector_candidates,
};
#[path = "comment_intelligence_problem_candidates.rs"]
mod candidates;
use candidates::{assign_model_equivalence, validated_candidates};

#[path = "comment_intelligence_problem_relations.rs"]
mod relations;
pub use relations::{
    PROBLEM_RELATION_VERSION, ProblemRelation, RelationDecision, apply_problem_relation_output,
    apply_problem_task_output, prepare_problem_relation, problem_relation_task,
    promote_unmatched_expression, validate_problem_relations,
};
pub(crate) use relations::{apply_problem_relation, record_boundary_decision};

#[path = "comment_problem_worker.rs"]
mod worker;
pub use worker::{
    problem_automation_state, problem_task_status, queue_problem_task, run_problem_relation_once,
    sync_problem_tasks,
};

pub const DICTIONARY_VERSION: &str = "comment-language.v2";
pub const PROBLEM_RULE_VERSION: &str = "problem-exact-definition.v2";

fn statement(error: sqlx::Error) -> StorageError {
    StorageError::Statement(error)
}

/// Bounded worker projection; never called by a tab click. Repeated observations with the
/// same body and accepted analysis do not rewrite terms or create fresh problem members.
pub async fn reconcile_problem_index(
    database: &Database,
    limit: i64,
) -> Result<Value, StorageError> {
    if !(1..=500).contains(&limit) {
        return Err(rejected("CI_INVALID_LIMIT"));
    }
    let rows = sqlx::query("SELECT s.canonical_ref,s.source_ref,s.domain_ref,s.body,s.source_sha256,s.role,a.work_ref AS analysis_ref,a.result FROM linggan_ci_source s LEFT JOIN LATERAL (SELECT a.work_ref,a.result FROM linggan_comment_analysis_work a JOIN linggan_material_comment original ON original.material_ref=a.source_ref WHERE EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=s.domain_ref AND d.is_own_domain) AND original.content_public_ref=s.work_ref AND original.comment_external_id=s.comment_external_id AND a.state IN ('succeeded','no_signal') AND a.result->>'sourceSha256'=s.source_sha256 ORDER BY a.created_at DESC,a.work_ref DESC LIMIT 1) a ON true LEFT JOIN linggan_ci_projection_cursor c ON c.canonical_ref=s.canonical_ref AND c.domain_ref=s.domain_ref WHERE s.body IS NOT NULL AND (c.canonical_ref IS NULL OR c.source_sha256<>s.source_sha256 OR c.dictionary_version<>$1 OR c.analysis_ref IS DISTINCT FROM a.work_ref) ORDER BY s.first_observed_at,s.canonical_ref LIMIT $2")
        .bind(DICTIONARY_VERSION).bind(limit).fetch_all(database.pool()).await.map_err(statement)?;
    let mut projected = 0;
    let mut model_assigned = 0;
    let mut domains = BTreeSet::new();
    for row in rows {
        let canonical: Uuid = row.get("canonical_ref");
        let domain: Uuid = row.get("domain_ref");
        let body: String = row.get("body");
        let source_sha: String = row.get("source_sha256");
        let analysis_ref: Option<Uuid> = row.get("analysis_ref");
        let result: Option<Value> = row.get("result");
        let role: String = row.get("role");
        let clean = crate::comment_cleaning::clean(&body);
        let result_readable = if let Some(result) = &result {
            crate::comment_daily_read::context_readable(database, result)
                .await
                .map_err(|_| rejected("CI_CONTEXT_UNAVAILABLE"))?
        } else {
            false
        };
        let mut tx = database.pool().begin().await.map_err(statement)?;
        lock_domain(&mut tx, domain).await?;
        // The source may have changed or been restricted after candidate enumeration.
        let still_current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ci_source WHERE canonical_ref=$1 AND domain_ref=$2 AND source_sha256=$3)")
            .bind(canonical).bind(domain).bind(&source_sha).fetch_one(&mut *tx).await.map_err(statement)?;
        if !still_current {
            continue;
        }
        sqlx::query("DELETE FROM linggan_ci_term_index WHERE canonical_ref=$1 AND domain_ref=$2")
            .bind(canonical)
            .bind(domain)
            .execute(&mut *tx)
            .await
            .map_err(statement)?;
        if clean.state != "dropped"
            && ![
                "platform",
                "platform_system",
                "system",
                "automatic",
                "summary",
            ]
            .contains(&role.as_str())
        {
            let terms = comment_terms(&clean.text);
            sqlx::query("INSERT INTO linggan_ci_term_index(canonical_ref,domain_ref,term,source_sha256,dictionary_version) SELECT $1,$2,unnest($3::text[]),$4,$5")
                .bind(canonical).bind(domain).bind(terms).bind(&source_sha).bind(DICTIONARY_VERSION).execute(&mut *tx).await.map_err(statement)?;
        }
        sqlx::query("UPDATE linggan_ci_problem_candidate SET state='superseded' WHERE canonical_ref=$1 AND domain_ref=$4 AND (analysis_ref IS DISTINCT FROM $2 OR NOT $3)")
            .bind(canonical).bind(analysis_ref).bind(result_readable && clean.state != "dropped").bind(domain).execute(&mut *tx).await.map_err(statement)?;
        let locked: bool = sqlx::query_scalar("SELECT COALESCE((SELECT locked FROM linggan_ci_source_research WHERE canonical_ref=$1 AND domain_ref=$2),false)")
            .bind(canonical).bind(domain).fetch_one(&mut *tx).await.map_err(statement)?;
        if !locked {
            sqlx::query("DELETE FROM linggan_ci_problem_member WHERE canonical_ref=$1 AND problem_ref IN(SELECT problem_ref FROM linggan_ci_problem WHERE domain_ref=$4) AND origin IN ('exact_definition','model_equivalence','model_expression') AND (analysis_ref IS DISTINCT FROM $2 OR NOT $3)")
                .bind(canonical).bind(analysis_ref).bind(result_readable && clean.state != "dropped").bind(domain).execute(&mut *tx).await.map_err(statement)?;
        }
        if clean.state != "dropped"
            && result_readable
            && ![
                "author",
                "platform",
                "platform_system",
                "system",
                "automatic",
                "summary",
            ]
            .contains(&role.as_str())
        {
            if let (Some(analysis), Some(result)) = (analysis_ref, result.as_ref()) {
                model_assigned +=
                    project_candidates(&mut tx, domain, canonical, analysis, result, &body).await?;
            }
        }
        sqlx::query("INSERT INTO linggan_ci_projection_cursor(canonical_ref,source_sha256,dictionary_version,analysis_ref,domain_ref) VALUES($1,$2,$3,$4,$5) ON CONFLICT(canonical_ref,domain_ref) DO UPDATE SET source_sha256=EXCLUDED.source_sha256,dictionary_version=EXCLUDED.dictionary_version,analysis_ref=EXCLUDED.analysis_ref,updated_at=scope_001_now()")
            .bind(canonical).bind(source_sha).bind(DICTIONARY_VERSION).bind(analysis_ref).bind(domain).execute(&mut *tx).await.map_err(statement)?;
        tx.commit().await.map_err(statement)?;
        projected += 1;
        domains.insert(domain);
    }
    let pending_domains: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT domain_ref FROM linggan_ci_problem_candidate WHERE state='unmerged'",
    )
    .fetch_all(database.pool())
    .await
    .map_err(statement)?;
    domains.extend(pending_domains);
    let mut assigned = 0;
    for domain in domains {
        assigned += assign_exact_definitions(database, domain, limit).await?;
    }
    Ok(
        json!({"projected":projected,"assigned":assigned+model_assigned,"modelAssigned":model_assigned,"dictionaryVersion":DICTIONARY_VERSION,"problemRuleVersion":PROBLEM_RULE_VERSION}),
    )
}

async fn project_candidates(
    connection: &mut sqlx::PgConnection,
    domain: Uuid,
    canonical: Uuid,
    analysis: Uuid,
    result: &Value,
    body: &str,
) -> Result<u64, StorageError> {
    let mut model_assigned = 0;
    // The evidence's source is the analyzed observation, which can predate a
    // subsequent likes-only observation of the same immutable identity/body.
    let analyzed_source = result["sourceRef"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok());
    if let Some(analyzed_source) = analyzed_source {
        for (ordinal, name, meaning, evidence, proposal) in
            validated_candidates(result, analyzed_source, body)
        {
            let key = definition_key(&name, &meaning);
            let candidate:Uuid = sqlx::query_scalar("INSERT INTO linggan_ci_problem_candidate(candidate_ref,canonical_ref,domain_ref,analysis_ref,ordinal,name,meaning,definition_key,evidence,proposal,state) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'unmerged') ON CONFLICT(analysis_ref,ordinal) DO UPDATE SET proposal=CASE WHEN linggan_ci_problem_candidate.state IN ('assigned','resolved') THEN linggan_ci_problem_candidate.proposal ELSE EXCLUDED.proposal END RETURNING candidate_ref")
            .bind(Uuid::new_v4()).bind(canonical).bind(domain).bind(analysis).bind(ordinal as i32).bind(&name).bind(&meaning).bind(key).bind(&evidence).bind(if proposal.is_null() { None } else { Some(&proposal) })
            .fetch_one(&mut *connection).await.map_err(statement)?;
            let pending: bool = sqlx::query_scalar(
                "SELECT state='unmerged' FROM linggan_ci_problem_candidate WHERE candidate_ref=$1",
            )
            .bind(candidate)
            .fetch_one(&mut *connection)
            .await
            .map_err(statement)?;
            if !pending {
                continue;
            }
            // Reuse only definition-specific human decisions at the current target revision.
            // Resolving one expression never locks every problem in this comment.
            let decisions = sqlx::query("SELECT DISTINCT ON(d.target_ref) d.target_ref,d.target_definition_revision,d.relation,d.reason FROM linggan_ci_problem_boundary_decision d JOIN linggan_ci_problem p ON p.problem_ref=d.target_ref AND p.definition_revision=d.target_definition_revision AND p.redirect_ref IS NULL WHERE d.domain_ref=$1 AND d.definition_key=$2 AND d.origin='manual' ORDER BY d.target_ref,d.created_at DESC,d.decision_ref DESC")
                .bind(domain).bind(definition_key(&name, &meaning)).fetch_all(&mut *connection).await.map_err(statement)?;
            let mut decided = false;
            for decision in decisions {
                if decision.get::<String, _>("relation") == "same" {
                    apply_problem_relation(
                        connection,
                        domain,
                        candidate,
                        decision.get("target_ref"),
                        decision.get("target_definition_revision"),
                        "same",
                        &decision.get::<String, _>("reason"),
                        "boundary_reuse",
                        None,
                    )
                    .await?;
                    decided = true;
                    model_assigned += 1;
                    break;
                }
            }
            if !decided && !proposal.is_null() {
                if assign_model_equivalence(
                    connection, domain, canonical, analysis, candidate, &evidence, &proposal,
                )
                .await?
                {
                    model_assigned += 1;
                }
            }
        }
    }
    Ok(model_assigned)
}

async fn assign_exact_definitions(
    database: &Database,
    domain: Uuid,
    limit: i64,
) -> Result<u64, StorageError> {
    let mut tx = database.pool().begin().await.map_err(statement)?;
    lock_domain(&mut tx, domain).await?;
    let keys = sqlx::query_scalar::<_,String>("SELECT c.definition_key FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) WHERE c.domain_ref=$1 AND c.state='unmerged' AND c.proposal IS NULL AND EXISTS(SELECT 1 FROM linggan_comment_analysis_work legacy WHERE legacy.work_ref=c.analysis_ref AND legacy.rule_version<>'comment-research.v4') AND NOT COALESCE((SELECT locked FROM linggan_ci_source_research h WHERE h.canonical_ref=c.canonical_ref AND h.domain_ref=c.domain_ref),false) AND NOT EXISTS(SELECT 1 FROM linggan_ci_problem p WHERE p.domain_ref=c.domain_ref AND p.definition->>'exactDefinitionKey'=c.definition_key AND (p.redirect_ref IS NOT NULL OR COALESCE((p.definition->>'humanEdited')::boolean,false))) GROUP BY c.definition_key HAVING count(DISTINCT c.canonical_ref)>=3 OR EXISTS(SELECT 1 FROM linggan_ci_problem p WHERE p.domain_ref=$1 AND p.definition->>'exactDefinitionKey'=c.definition_key AND p.redirect_ref IS NULL) ORDER BY max(c.created_at),c.definition_key LIMIT $2")
        .bind(domain).bind(limit).fetch_all(&mut *tx).await.map_err(statement)?;
    let mut assigned = 0;
    for key in keys {
        let existing: Option<Uuid> = sqlx::query_scalar("SELECT problem_ref FROM linggan_ci_problem WHERE domain_ref=$1 AND definition->>'exactDefinitionKey'=$2 AND redirect_ref IS NULL AND NOT COALESCE((definition->>'humanEdited')::boolean,false) ORDER BY created_at,problem_ref LIMIT 1")
            .bind(domain).bind(&key).fetch_optional(&mut *tx).await.map_err(statement)?;
        let candidates = sqlx::query("SELECT DISTINCT ON(c.canonical_ref) c.candidate_ref,c.canonical_ref,c.analysis_ref,c.name,c.meaning,c.evidence,a.result FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) JOIN linggan_comment_analysis_work a ON a.work_ref=c.analysis_ref AND a.result->>'sourceSha256'=s.source_sha256 WHERE c.domain_ref=$1 AND c.definition_key=$2 AND c.state='unmerged' AND c.proposal IS NULL AND a.rule_version<>'comment-research.v4' AND NOT COALESCE((SELECT locked FROM linggan_ci_source_research h WHERE h.canonical_ref=c.canonical_ref AND h.domain_ref=c.domain_ref),false) ORDER BY c.canonical_ref,c.created_at DESC,c.candidate_ref")
            .bind(domain).bind(&key).fetch_all(&mut *tx).await.map_err(statement)?;
        let mut readable_candidates = Vec::new();
        for candidate in candidates {
            let result: Value = candidate.get("result");
            if crate::comment_daily_read::context_readable(database, &result)
                .await
                .map_err(|_| rejected("CI_CONTEXT_UNAVAILABLE"))?
            {
                readable_candidates.push(candidate);
            }
        }
        let candidates = readable_candidates;
        if candidates.is_empty() || (existing.is_none() && candidates.len() < 3) {
            continue;
        }
        if existing.is_none() {
            let reserved: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ci_problem WHERE domain_ref=$1 AND (definition->>'exactDefinitionKey'=$2 OR (lower(btrim(name))=lower(btrim($3)) AND lower(btrim(meaning))=lower(btrim($4)))))")
                .bind(domain).bind(&key).bind(candidates[0].get::<String,_>("name")).bind(candidates[0].get::<String,_>("meaning"))
                .fetch_one(&mut *tx).await.map_err(statement)?;
            if reserved {
                continue;
            }
        }
        let (problem, before) = if let Some(problem) = existing {
            (problem, problem_snapshot(&mut tx, domain, problem).await?)
        } else {
            let problem = Uuid::new_v4();
            let definition = json!({"lifecycle":"emerging","lifecycleBasis":"observed_expression_count.v1","exactDefinitionKey":key,"ruleVersion":PROBLEM_RULE_VERSION,"boundary":"仅定义文字完全一致；相似表达保留候选，未据此断言语义等价", "examples":candidates.iter().take(3).map(|r|r.get::<Uuid,_>("canonical_ref")).collect::<Vec<_>>(),"counterexamples":[]});
            sqlx::query("INSERT INTO linggan_ci_problem(problem_ref,domain_ref,name,meaning,definition,origin) VALUES($1,$2,$3,$4,$5,'exact_definition')")
                .bind(problem).bind(domain).bind(candidates[0].get::<String,_>("name")).bind(candidates[0].get::<String,_>("meaning")).bind(definition)
                .execute(&mut *tx).await.map_err(statement)?;
            (problem, json!({}))
        };
        for candidate in candidates {
            let inserted = sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,evidence,analysis_ref) VALUES($1,$2,'exact_definition',$3,$4) ON CONFLICT(problem_ref,canonical_ref) DO NOTHING")
                .bind(problem).bind(candidate.get::<Uuid,_>("canonical_ref")).bind(candidate.get::<Value,_>("evidence")).bind(candidate.get::<Uuid,_>("analysis_ref"))
                .execute(&mut *tx).await.map_err(statement)?;
            assigned += inserted.rows_affected();
            sqlx::query("UPDATE linggan_ci_problem_candidate SET state='assigned',problem_ref=$2 WHERE candidate_ref=$1")
                .bind(candidate.get::<Uuid,_>("candidate_ref")).bind(problem).execute(&mut *tx).await.map_err(statement)?;
        }
        sqlx::query("UPDATE linggan_ci_problem p SET definition=definition || jsonb_build_object('lifecycle',CASE WHEN (SELECT count(DISTINCT m.canonical_ref) FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) JOIN linggan_comment_analysis_work a ON a.work_ref=m.analysis_ref AND a.result->>'sourceSha256'=s.source_sha256 WHERE m.problem_ref=p.problem_ref AND s.domain_ref=p.domain_ref)>=3 THEN 'stable' ELSE 'emerging' END) WHERE problem_ref=$1")
            .bind(problem).execute(&mut *tx).await.map_err(statement)?;
        if !before.as_object().is_some_and(|v| v.is_empty()) {
            sqlx::query("UPDATE linggan_ci_problem SET revision=revision+1,updated_at=scope_001_now() WHERE problem_ref=$1")
                .bind(problem).execute(&mut *tx).await.map_err(statement)?;
        }
        let after = problem_snapshot(&mut tx, domain, problem).await?;
        problem_version(
            &mut tx,
            problem,
            None,
            "exact_definition_membership",
            "同一问题定义的增量成员；未推断样本内讨论增长",
            &before,
            &after,
        )
        .await?;
    }
    tx.commit().await.map_err(statement)?;
    Ok(assigned)
}

/// Resolve stable aliases and return metadata with currently eligible member references.
/// Counts in the main query must still apply its exact time/text/lens scope.
pub async fn problem_details(
    database: &Database,
    domain: Uuid,
    problem: Uuid,
) -> Result<Value, StorageError> {
    let mut tx = database.pool().begin().await.map_err(statement)?;
    let mut snapshot = problem_metadata(&mut tx, domain, problem).await?;
    let mut current = problem;
    let mut redirects = BTreeSet::new();
    while let Some(next) = snapshot["redirectRef"].as_str() {
        if !redirects.insert(current) || redirects.len() > 64 {
            return Err(rejected("CI_PROBLEM_ALIAS_CYCLE"));
        }
        current = Uuid::parse_str(next).map_err(|_| rejected("CI_INVALID_STATE"))?;
        snapshot = problem_metadata(&mut tx, domain, current).await?;
    }
    let rows = sqlx::query("SELECT s.canonical_ref,s.source_ref,s.work_ref,left(s.body,4000) AS body,m.evidence,m.origin,a.result FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) LEFT JOIN linggan_comment_analysis_work a ON a.work_ref=m.analysis_ref LEFT JOIN linggan_ci_source_research h ON h.canonical_ref=s.canonical_ref AND h.domain_ref=s.domain_ref WHERE m.problem_ref=$1 AND s.domain_ref=$2 AND ((m.origin='manual' AND h.source_sha256=s.source_sha256) OR (m.origin<>'manual' AND a.result->>'sourceSha256'=s.source_sha256)) ORDER BY s.first_observed_at DESC,s.canonical_ref LIMIT 101")
        .bind(current).bind(domain).fetch_all(&mut *tx).await.map_err(statement)?;
    let history = sqlx::query("SELECT revision,kind,reason,created_at::text AS at,before_value->>'name' AS old_name,after_value->>'name' AS new_name FROM linggan_ci_problem_version WHERE problem_ref=$1 ORDER BY revision DESC LIMIT 100")
        .bind(current).fetch_all(&mut *tx).await.map_err(statement)?;
    tx.commit().await.map_err(statement)?;
    let members_truncated = rows.len() > 100;
    let mut members = Vec::new();
    let mut distribution = BTreeMap::<Uuid, i64>::new();
    for row in rows.into_iter().take(100) {
        if row.get::<String, _>("origin") != "manual" {
            let Some(result) = row.get::<Option<Value>, _>("result") else {
                continue;
            };
            if !crate::comment_daily_read::context_readable(database, &result)
                .await
                .map_err(|_| rejected("CI_CONTEXT_UNAVAILABLE"))?
            {
                continue;
            }
        }
        let work: Uuid = row.get("work_ref");
        *distribution.entry(work).or_default() += 1;
        members.push(json!({"canonicalRef":row.get::<Uuid,_>("canonical_ref"),"sourceRef":row.get::<Uuid,_>("source_ref"),"workRef":work,"body":row.get::<String,_>("body"),"evidence":row.get::<Value,_>("evidence")}));
    }
    if members.is_empty() {
        return Err(rejected("CI_PROBLEM_UNAVAILABLE"));
    }
    snapshot["sourceRefs"] = json!(
        members
            .iter()
            .map(|m| m["canonicalRef"].clone())
            .collect::<Vec<_>>()
    );
    snapshot["definition"]["examples"] = json!(
        members
            .iter()
            .take(3)
            .map(|m| m["canonicalRef"].clone())
            .collect::<Vec<_>>()
    );
    let mut source_distribution: Vec<_> = distribution.into_iter().collect();
    source_distribution.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let suggestions = problem_suggestions(database, domain, current, &snapshot, &members).await?;
    Ok(
        json!({"problem":snapshot,"requestedProblemRef":problem,"resolvedProblemRef":current,"redirected":current!=problem,
        "history":history.iter().map(|r|json!({"revision":r.get::<i64,_>("revision"),"kind":r.get::<String,_>("kind"),"reason":r.get::<String,_>("reason"),"at":r.get::<String,_>("at"),"oldName":r.get::<Option<String>,_>("old_name"),"newName":r.get::<Option<String>,_>("new_name")})).collect::<Vec<_>>(),
        "members":members,"membersLimit":100,"membersTruncated":members_truncated,"sourceDistributionScope":"visible_evidence_sample","sourceDistribution":source_distribution.into_iter().take(5).map(|(work,n)|json!({"workRef":work,"comments":n})).collect::<Vec<_>>(),"suggestions":suggestions}),
    )
}

async fn problem_suggestions(
    database: &Database,
    domain: Uuid,
    problem: Uuid,
    snapshot: &Value,
    members: &[Value],
) -> Result<Vec<Value>, StorageError> {
    let candidates = sqlx::query("SELECT p.problem_ref,p.name,p.meaning FROM linggan_ci_problem p WHERE p.domain_ref=$1 AND p.problem_ref<>$2 AND p.redirect_ref IS NULL AND p.name=$3 AND EXISTS(SELECT 1 FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) WHERE m.problem_ref=p.problem_ref AND s.domain_ref=$1) ORDER BY p.created_at,p.problem_ref LIMIT 10")
        .bind(domain).bind(problem).bind(snapshot["name"].as_str().unwrap_or(""))
        .fetch_all(database.pool()).await.map_err(statement)?;
    let mut suggestions = Vec::new();
    for row in candidates {
        let other: Uuid = row.get("problem_ref");
        let evidence = sqlx::query("SELECT s.source_ref,m.origin,a.result FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) LEFT JOIN linggan_comment_analysis_work a ON a.work_ref=m.analysis_ref LEFT JOIN linggan_ci_source_research h ON h.canonical_ref=s.canonical_ref AND h.domain_ref=s.domain_ref WHERE m.problem_ref=$1 AND s.domain_ref=$2 AND ((m.origin='manual' AND h.source_sha256=s.source_sha256) OR (m.origin<>'manual' AND a.result->>'sourceSha256'=s.source_sha256)) ORDER BY s.first_observed_at,s.source_ref LIMIT 30")
            .bind(other).bind(domain).fetch_all(database.pool()).await.map_err(statement)?;
        let mut refs = Vec::new();
        for item in evidence {
            if item.get::<String, _>("origin") != "manual" {
                let Some(result) = item.get::<Option<Value>, _>("result") else {
                    continue;
                };
                if !crate::comment_daily_read::context_readable(database, &result)
                    .await
                    .map_err(|_| rejected("CI_CONTEXT_UNAVAILABLE"))?
                {
                    continue;
                }
            }
            refs.push(item.get::<Uuid, _>("source_ref"));
            if refs.len() == 3 {
                break;
            }
        }
        if refs.is_empty() {
            continue;
        }
        suggestions.push(json!({"kind":"compare_definitions","targetRef":other,"title":"同名问题的定义需要比较","automatic":false,"ruleVersion":PROBLEM_RULE_VERSION,
            "definitionDifference":{"current":snapshot["meaning"],"candidate":row.get::<String,_>("meaning")},
            "currentEvidence":members.iter().take(3).map(|m|m["sourceRef"].clone()).collect::<Vec<_>>(),"candidateEvidence":refs,
            "counterexampleState":"尚无经核验的反例；不能自动合并","canAutoApply":false}));
    }
    Ok(suggestions)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_frequency_terms_remain_literal_and_preserve_community_words() {
        let body = "A娃执行功能 A娃执行功能 ADHD 天线宝宝天线宝宝 家校沟通";
        let terms = comment_terms(body);
        assert_eq!(terms.iter().filter(|t| t.as_str() == "A娃").count(), 1);
        assert!(terms.contains(&"天线宝宝".into()));
        assert!(terms.iter().all(|t| body.contains(t)));
        assert!(comment_terms("@某人").is_empty());
    }
    #[test]
    fn exact_definitions_keep_negation_and_context_significant() {
        assert_ne!(
            definition_key("吃药", "担心风险但愿意尝试"),
            definition_key("吃药", "拒绝药物干预")
        );
        assert_ne!(
            definition_key("坚持", "家庭作业"),
            definition_key("坚持", "运动康复")
        );
        assert_eq!(
            definition_key(" ADHD ", " 持续困难 "),
            definition_key("adhd", "持续困难")
        );
    }
    #[test]
    fn vector_candidates_are_bounded_and_require_same_qualified_model_space() {
        let query = DefinitionVector {
            problem_ref: Uuid::new_v4(),
            model_version: "embedding.fixture.v1".into(),
            dimensions: 2,
            values: vec![1.0, 0.0],
        };
        let mut candidates = Vec::new();
        for _ in 0..20 {
            let mut value = query.clone();
            value.problem_ref = Uuid::new_v4();
            candidates.push(value);
        }
        assert_eq!(
            recall_vector_candidates(&query, &candidates).unwrap().len(),
            10
        );
        candidates[0].model_version = "chat.model".into();
        assert_eq!(
            recall_vector_candidates(&query, &candidates),
            Err("incomparable_embedding")
        );
        let mut invalid = query.clone();
        invalid.values[0] = f64::NAN;
        assert_eq!(
            recall_vector_candidates(&invalid, &[]),
            Err("invalid_embedding")
        );
    }
    #[test]
    fn candidates_require_own_exact_evidence_coordinates() {
        let source = Uuid::new_v4();
        let mut result = json!({"semantic":{"outcome":"interpretable","problems":[{"name":"时间困难","meaning":"执行耗时","evidence":[{"sourceRef":source,"startChar":0,"endChar":2}]}]}});
        assert_eq!(validated_candidates(&result, source, "时间不够").len(), 1);
        result["semantic"]["problems"][0]["evidence"][0]["endChar"] = json!(99);
        assert!(validated_candidates(&result, source, "时间不够").is_empty());
    }
}
