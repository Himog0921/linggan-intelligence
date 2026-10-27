//! Source-qualified problem and pending-candidate read model.
use super::*;
use linggan_storage_postgres::Database;
pub async fn read_problems(db: &Database, domain: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let mut definitions = definitions_in(&mut tx, domain).await?;
    for definition in definitions.as_array_mut().unwrap() {
        let id: Uuid = serde_json::from_value(definition["definitionRef"].clone())
            .map_err(|_| FoundationError::Invalid)?;
        let rows=sqlx::query("SELECT m.request_ref,m.signal_ordinal,r.work_ref,source.comment_external_id FROM linggan_comment_problem_membership m JOIN linggan_comment_research_request r USING(request_ref) JOIN linggan_material_comment source ON source.material_ref=r.source_ref WHERE m.definition_ref=$1 AND r.domain_ref=$2 ORDER BY r.created_at DESC LIMIT 100").bind(id).bind(domain).fetch_all(&mut *tx).await?;
        let mut evidence = Vec::new();
        for row in rows {
            let request: Uuid = row.get("request_ref");
            let (source, _) = match comment_research::frozen_in(&mut tx, request).await {
                Ok(v) => v,
                Err(FoundationError::AdmissionDenied(_)) => continue,
                Err(e) => return Err(e),
            };
            let semantic = semantic_in(&mut tx, request).await?;
            let ordinal = row.get::<i32, _>("signal_ordinal") as usize;
            let signal = &semantic["signals"][ordinal];
            let start = signal["evidenceSpan"]["start"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            let end = signal["evidenceSpan"]["end"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            let quote: String = source
                .raw_text
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect();
            evidence.push(json!({"requestRef":request,"workRef":row.get::<Uuid,_>("work_ref"),"commentExternalId":row.get::<String,_>("comment_external_id"),"signalOrdinal":ordinal,"proposition":signal["proposition"],"quote":quote}));
        }
        definition["evidence"] = json!(evidence);
    }
    let mut candidates = Vec::new();
    for (request, proposal) in proposals_in(&mut tx, domain).await? {
        let (source, _) = comment_research::frozen_in(&mut tx, request).await?;
        let semantic = semantic_in(&mut tx, request).await?;
        let work: Uuid = sqlx::query_scalar(
            "SELECT work_ref FROM linggan_comment_research_request WHERE request_ref=$1",
        )
        .bind(request)
        .fetch_one(&mut *tx)
        .await?;
        for d in proposal.decisions.into_iter().filter(|d| d.status == "new") {
            let accepted:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_problem_membership WHERE request_ref=$1 AND signal_ordinal=$2)").bind(request).bind(d.signal_ordinal as i32).fetch_one(&mut *tx).await?;
            if accepted {
                continue;
            }
            let signal = &semantic["signals"][d.signal_ordinal];
            let start = signal["evidenceSpan"]["start"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            let end = signal["evidenceSpan"]["end"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            let quote: String = source
                .raw_text
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect();
            candidates.push(json!({"requestRef":request,"signalOrdinal":d.signal_ordinal,"problem":d.problem,"reason":d.reason,"workRef":work,"commentExternalId":source.comment_external_id,"quote":quote,"state":"awaiting_independent_support"}));
        }
    }
    tx.commit().await?;
    Ok(json!({"domainRef":domain,"problems":definitions,"candidates":candidates}))
}

/// The original workflow may have waited for support later supplied by another review.
pub(crate) async fn pending_supported_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<bool, FoundationError> {
    let row=sqlx::query("SELECT r.domain_ref,result.output FROM linggan_comment_research_request r JOIN linggan_comment_research_stage stage USING(request_ref) JOIN linggan_comment_problem_stage_result result USING(item_ref) WHERE r.request_ref=$1 AND stage.stage_number=1").bind(request).fetch_optional(&mut **tx).await?;
    let Some(row) = row else { return Ok(false) };
    let Some(output) = row.get::<Option<Value>, _>("output") else {
        return Ok(false);
    };
    let proposal: Resolution =
        serde_json::from_value(output).map_err(|_| FoundationError::Invalid)?;
    let qualified = definitions_in(tx, row.get("domain_ref")).await?;
    for decision in proposal.decisions.iter().filter(|d| d.status == "new") {
        let definition:Option<Uuid>=sqlx::query_scalar("SELECT definition_ref FROM linggan_comment_problem_membership WHERE request_ref=$1 AND signal_ordinal=$2").bind(request).bind(decision.signal_ordinal as i32).fetch_optional(&mut **tx).await?;
        if !definition.is_some_and(|id| {
            qualified
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["definitionRef"] == json!(id))
        }) {
            return Ok(false);
        }
    }
    Ok(proposal.decisions.iter().any(|d| d.status == "new"))
}
