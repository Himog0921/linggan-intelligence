//! Read current V2 results and trace their evidence without exposing commenter IDs.
use super::{current_source_in, start::read_only_tx, unit, work_context};
use crate::ai_foundation::store::FoundationError;
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSignal {
    kind: String,
    proposition: String,
    evidence_span: Span,
}

#[derive(Deserialize)]
struct Span {
    start: usize,
    end: usize,
}

impl StoredSignal {
    fn evidence(&self, raw: &str) -> Result<String, FoundationError> {
        let span = &self.evidence_span;
        if span.start >= span.end || span.end > raw.chars().count() {
            return Err(FoundationError::Invalid);
        }
        Ok(raw
            .chars()
            .skip(span.start)
            .take(span.end - span.start)
            .collect())
    }
}

/// The current problem list. `summary` is one sentence the server writes from the stored
/// definition (no separate description is stored, so it is built from actor, goal and barrier);
/// a withdrawn definition shows no summary, as it shows no goal or barrier.
pub async fn read_problems(db: &Database, domain: Uuid) -> Result<Value, FoundationError> {
    let rows: Vec<(Value, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT jsonb_build_object('problemRef',p.problem_ref,'label',CASE WHEN withdrawn.problem_ref IS NULL THEN p.label ELSE '需求来源已撤回' END,'state',p.state,
           'authorCount',p.author_count,'commentCount',p.comment_count,'workCount',p.work_count,
           'lastResearchedAt',p.last_researched_at),
           CASE WHEN withdrawn.problem_ref IS NULL THEN p.actor END,
           CASE WHEN withdrawn.problem_ref IS NULL THEN p.goal END,
           CASE WHEN withdrawn.problem_ref IS NULL THEN p.barrier END
         FROM linggan_comment_problem_current p JOIN linggan_comment_problem_definition d USING(problem_ref)
         LEFT JOIN linggan_comment_problem_withdrawn_source withdrawn USING(problem_ref)
         WHERE p.domain_ref=$1 AND d.origin_invocation_ref IS NOT NULL
         AND (withdrawn.problem_ref IS NULL OR EXISTS(
           SELECT 1 FROM linggan_comment_problem_active_redirect edge WHERE edge.to_problem_ref=p.problem_ref
         ))
         ORDER BY p.author_count DESC,p.last_researched_at DESC NULLS LAST,p.problem_ref",
    ).bind(domain).fetch_all(db.pool()).await?;
    let problems: Vec<Value> = rows
        .into_iter()
        .map(|(mut problem, actor, goal, barrier)| {
            problem["summary"] = json!(problem_summary(
                actor.as_deref(),
                goal.as_deref(),
                barrier.as_deref()
            ));
            problem
        })
        .collect();
    Ok(json!({"domainRef":domain,"problems":problems}))
}

fn summary_part(value: Option<&str>) -> Option<&str> {
    value
        .map(|text| {
            text.trim()
                .trim_end_matches(['。', '.', '；', ';', '，', ',', '！', '!', '、'])
                .trim()
        })
        .filter(|text| !text.is_empty())
}

/// One sentence from a definition: "<actor>希望<goal>，但<barrier>。" with any missing part left out.
fn problem_summary(
    actor: Option<&str>,
    goal: Option<&str>,
    barrier: Option<&str>,
) -> Option<String> {
    let actor = summary_part(actor).unwrap_or("");
    match (summary_part(goal), summary_part(barrier)) {
        (Some(goal), Some(barrier)) => Some(format!("{actor}希望{goal}，但{barrier}。")),
        (Some(goal), None) => Some(format!("{actor}希望{goal}。")),
        (None, Some(barrier)) if actor.is_empty() => Some(format!("{barrier}。")),
        (None, Some(barrier)) => Some(format!("{actor}：{barrier}。")),
        (None, None) => None,
    }
}

#[cfg(test)]
mod summary_tests {
    use super::problem_summary;

    #[test]
    fn a_summary_is_one_sentence_from_the_definition_and_absent_without_goal_or_barrier() {
        assert_eq!(
            problem_summary(
                Some("小学生家长"),
                Some("孩子按时开始写作业"),
                Some("需要反复催促才动笔。")
            )
            .as_deref(),
            Some("小学生家长希望孩子按时开始写作业，但需要反复催促才动笔。")
        );
        assert_eq!(
            problem_summary(None, Some(" 孩子按时开始写作业 "), Some("  ")).as_deref(),
            Some("希望孩子按时开始写作业。")
        );
        assert_eq!(
            problem_summary(None, None, Some("需要反复催促")).as_deref(),
            Some("需要反复催促。")
        );
        assert_eq!(
            problem_summary(Some("家长"), Some(""), Some("需要反复催促")).as_deref(),
            Some("家长：需要反复催促。")
        );
        assert_eq!(problem_summary(Some("家长"), None, Some("。")), None);
    }
}

pub async fn read_problem(
    db: &Database,
    domain: Uuid,
    requested: Uuid,
) -> Result<Value, FoundationError> {
    let mut tx = read_only_tx(db).await?;
    let detail = problem_in(&mut tx, domain, requested).await?;
    tx.commit().await?;
    Ok(detail)
}

pub(in crate::ai_foundation) async fn problem_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    requested: Uuid,
) -> Result<Value, FoundationError> {
    let (problem, withdrawn, mut detail): (Uuid, bool, Value) = sqlx::query_as(
        "SELECT p.problem_ref,withdrawn.problem_ref IS NOT NULL,jsonb_build_object('domainRef',p.domain_ref,'problemRef',p.problem_ref,
           'label',CASE WHEN withdrawn.problem_ref IS NULL THEN p.label ELSE '需求来源已撤回' END,'state',p.state,'authorCount',p.author_count,'commentCount',p.comment_count,
           'workCount',p.work_count,'lastResearchedAt',p.last_researched_at,
           'actor',CASE WHEN withdrawn.problem_ref IS NULL THEN p.actor END,
           'goal',CASE WHEN withdrawn.problem_ref IS NULL THEN p.goal ELSE '需求来源已撤回' END,
           'barrier',CASE WHEN withdrawn.problem_ref IS NULL THEN p.barrier ELSE '需求来源已撤回' END,
           'context',CASE WHEN withdrawn.problem_ref IS NULL THEN p.context END)
         FROM linggan_comment_problem_resolution r
         JOIN linggan_comment_problem_current p ON p.problem_ref=r.current_ref
         JOIN linggan_comment_problem_definition d ON d.problem_ref=r.problem_ref
         LEFT JOIN linggan_comment_problem_withdrawn_source withdrawn ON withdrawn.problem_ref=p.problem_ref
         WHERE r.problem_ref=$1 AND p.domain_ref=$2 AND d.origin_invocation_ref IS NOT NULL
         AND (withdrawn.problem_ref IS NULL OR EXISTS(
           SELECT 1 FROM linggan_comment_problem_active_redirect edge WHERE edge.to_problem_ref=p.problem_ref
         ))",
    ).bind(requested).bind(domain).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    detail["redirectedFrom"] = json!((problem != requested).then_some(requested));
    detail["evidence"] = if withdrawn {
        json!([])
    } else {
        json!(voices(tx, domain, problem).await?)
    };
    let history: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('redirectRef',r.redirect_ref,
           'fromLabel',CASE WHEN withdrawn_source.problem_ref IS NULL THEN source.label ELSE '需求来源已撤回' END,
           'toLabel',CASE WHEN withdrawn_destination.problem_ref IS NULL THEN destination.label ELSE '需求来源已撤回' END,'createdAt',r.created_at,'revokedAt',v.created_at)
         FROM linggan_comment_problem_redirect r
         JOIN linggan_comment_problem_definition source ON source.problem_ref=r.from_problem_ref
         JOIN linggan_comment_problem_definition destination ON destination.problem_ref=r.to_problem_ref
         JOIN linggan_comment_problem_resolution source_current ON source_current.problem_ref=source.problem_ref
         JOIN linggan_comment_problem_resolution destination_current ON destination_current.problem_ref=destination.problem_ref
         LEFT JOIN linggan_comment_problem_withdrawn_source withdrawn_source ON withdrawn_source.problem_ref=source.problem_ref
         LEFT JOIN linggan_comment_problem_withdrawn_source withdrawn_destination ON withdrawn_destination.problem_ref=destination.problem_ref
         LEFT JOIN linggan_comment_problem_redirect_revocation v USING(redirect_ref)
         WHERE source.domain_ref=$2 AND $1 IN(source_current.current_ref,destination_current.current_ref)
         ORDER BY r.created_at DESC,r.redirect_ref DESC",
    ).bind(problem).bind(domain).fetch_all(&mut **tx).await?;
    detail["history"] = json!(history);
    Ok(detail)
}

async fn voices(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    problem: Uuid,
) -> Result<Vec<Value>, FoundationError> {
    let rows: Vec<(Uuid, Value)> = sqlx::query_as(
        "WITH one_per_comment AS (
           SELECT DISTINCT ON(support.work_ref,support.comment_external_id)
             support.request_ref,support.signal_ordinal,support.researched_at
           FROM linggan_comment_problem_support support
           WHERE support.domain_ref=$1 AND support.problem_ref=$2
           ORDER BY support.work_ref,support.comment_external_id,support.signal_ordinal
         )
         SELECT s.request_ref,result.result->'signals'->s.signal_ordinal
         FROM one_per_comment s JOIN linggan_comment_research_result result USING(request_ref)
         ORDER BY s.researched_at DESC,s.request_ref DESC LIMIT 12",
    )
    .bind(domain)
    .bind(problem)
    .fetch_all(&mut **tx)
    .await?;
    let mut evidence = Vec::new();
    for (request, signal) in rows {
        let source = match current_source_in(tx, request).await {
            Ok(source) => source,
            Err(FoundationError::AdmissionDenied(_) | FoundationError::NotFound) => continue,
            Err(error) => return Err(error),
        };
        let signal: StoredSignal =
            serde_json::from_value(signal).map_err(|_| FoundationError::Invalid)?;
        evidence.push(
            json!({"workRef":source.work_ref,"workTitle":source.work_title,
            "commentExternalId":source.comment_external_id,"requestRef":request,
            "proposition":signal.proposition,"quote":signal.evidence(&source.raw_text)?}),
        );
        if evidence.len() == 3 {
            break;
        }
    }
    Ok(evidence)
}

/// The latest execution attempt, with its current usable result kept distinct.
pub async fn read_latest(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    source: Uuid,
) -> Result<Value, FoundationError> {
    let mut tx = read_only_tx(db).await?;
    let row = sqlx::query(
        "SELECT r.request_ref,r.batch_ref,
           CASE WHEN result.result IS NOT NULL THEN 'succeeded'
                WHEN c.state='cancelled' THEN 'cancelled'
                WHEN result.failure_code IS NOT NULL THEN 'failed' ELSE c.state END AS state,
           coalesce(result.failure_code,c.failure_code) AS failure_code,
           current.request_ref AS current_request,result_now.result,u.l1_request_ref
         FROM linggan_material_comment selected
         JOIN linggan_material_comment prior ON prior.content_public_ref=selected.content_public_ref
           AND prior.comment_external_id=selected.comment_external_id
         JOIN linggan_comment_research_request r ON r.source_ref=prior.material_ref AND r.domain_ref=$1
         JOIN linggan_comment_research_unit_target t ON t.request_ref=r.request_ref
         JOIN linggan_comment_research_unit unit ON unit.unit_ref=t.unit_ref
         JOIN linggan_ai_batch_item_control c ON c.item_ref=unit.item_ref
         LEFT JOIN linggan_comment_research_result result ON result.request_ref=r.request_ref
         LEFT JOIN linggan_comment_current_research current ON current.domain_ref=r.domain_ref
           AND current.work_ref=r.work_ref AND current.comment_external_id=selected.comment_external_id
         LEFT JOIN linggan_comment_research_result result_now ON result_now.request_ref=current.request_ref
         LEFT JOIN linggan_comment_research_unit_target target_now ON target_now.request_ref=current.request_ref
         LEFT JOIN linggan_comment_research_unit u ON u.unit_ref=target_now.unit_ref
         WHERE selected.material_ref=$3 AND selected.content_public_ref=$2 AND r.work_ref=$2
         ORDER BY r.created_at DESC,r.request_ref DESC LIMIT 1",
    ).bind(domain).bind(work).bind(source).fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        return Ok(Value::Null);
    };
    let mut response = json!({
        "batchRef":row.try_get::<Uuid,_>("batch_ref")?,
        "state":row.try_get::<String,_>("state")?,
        "failureCode":row.try_get::<Option<String>,_>("failure_code")?,
        "result":null,
    });
    if let Some(request) = row.try_get::<Option<Uuid>, _>("current_request")? {
        match current_source_in(&mut tx, request).await {
            Ok(source) => {
                let stored: Value = row.try_get("result")?;
                let signals = read_signals(&mut tx, request, &source.raw_text, &stored).await?;
                let l1: Uuid = row.try_get("l1_request_ref")?;
                let context = unit::l1_card_in(&mut tx, l1)
                    .await?
                    .as_ref()
                    .and_then(work_context);
                response["result"] = json!({"requestRef":request,"outcome":stored["outcome"],
                    "reason":stored["reason"],"workContext":context,"signals":signals});
            }
            Err(FoundationError::AdmissionDenied(_) | FoundationError::NotFound) => {}
            Err(error) => return Err(error),
        }
    }
    tx.commit().await?;
    Ok(response)
}

async fn read_signals(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    raw: &str,
    stored: &Value,
) -> Result<Vec<Value>, FoundationError> {
    let signals: Vec<StoredSignal> =
        serde_json::from_value(stored["signals"].clone()).map_err(|_| FoundationError::Invalid)?;
    let problems: std::collections::BTreeMap<i32, Value> = sqlx::query_as::<_, (i32, Value)>(
        "SELECT support.signal_ordinal,jsonb_build_object('problemRef',p.problem_ref,'label',p.label,
           'state',p.state,'authorCount',p.author_count)
         FROM linggan_comment_problem_support support
         JOIN linggan_comment_problem_current p USING(problem_ref)
         WHERE support.request_ref=$1
         AND NOT EXISTS (SELECT 1 FROM linggan_comment_problem_withdrawn_source x WHERE x.problem_ref=p.problem_ref)",
    ).bind(request).fetch_all(&mut **tx).await?.into_iter().collect();
    signals
        .iter()
        .enumerate()
        .map(|(ordinal, signal)| {
            Ok(json!({
                "kind":signal.kind,"proposition":signal.proposition,"quote":signal.evidence(raw)?,
                "problem":problems.get(&(ordinal as i32)),
            }))
        })
        .collect()
}
