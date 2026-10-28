//! Immutable definitions and independently reviewed membership publication.
use super::*;
pub(crate) async fn publish_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    invocation: Uuid,
    context: &Value,
) -> Result<(), FoundationError> {
    let domain: Uuid = sqlx::query_scalar(
        "SELECT domain_ref FROM linggan_comment_research_request WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_one(&mut **tx)
    .await?;
    lock_publication_in(tx, domain).await?;
    let qualified = definitions_in(tx, domain).await?;
    if context["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| {
            !qualified
                .as_array()
                .unwrap()
                .iter()
                .any(|q| q["definitionRef"] == c["definitionRef"])
        })
    {
        return Err(denied("comment_publication_withheld"));
    }
    let resolution: Resolution = serde_json::from_value(context["proposal"].clone())
        .map_err(|_| FoundationError::Invalid)?;
    for decision in resolution.decisions {
        let definition = if decision.status == "matched" {
            decision.definition_ref
        } else if decision.status == "new" {
            let p = decision.problem.ok_or(FoundationError::Invalid)?;
            let identity = problem_identity(&p);
            let support = &context["supports"][decision.signal_ordinal.to_string()];
            let supporting_request: Uuid = serde_json::from_value(support["requestRef"].clone())
                .map_err(|_| FoundationError::Invalid)?;
            let supporting_ordinal = support["signalOrdinal"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            comment_research::frozen_in(tx, supporting_request).await?;
            if author_in(tx, supporting_request).await? == author_in(tx, request).await?
                || support["identity"] != identity
            {
                return Err(denied("comment_publication_withheld"));
            }
            let prior = proposals_in(tx, domain).await?;
            let pending_support = prior.iter().any(|(r, p)| {
                *r == supporting_request
                    && p.decisions.iter().any(|d| {
                        d.signal_ordinal == supporting_ordinal
                            && d.problem
                                .as_ref()
                                .is_some_and(|p| problem_identity(p) == identity)
                    })
            });
            // A parallel review may already have admitted this same pair. Its immutable
            // membership remains valid support only while that definition is qualified.
            let admitted_support: Option<Uuid> = sqlx::query_scalar("SELECT definition_ref FROM linggan_comment_problem_membership WHERE request_ref=$1 AND signal_ordinal=$2")
                .bind(supporting_request).bind(supporting_ordinal as i32).fetch_optional(&mut **tx).await?;
            let qualified_support = admitted_support.is_some_and(|id| {
                qualified
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["definitionRef"] == json!(id) && d["identityHash"] == identity)
            });
            if !pending_support && !qualified_support {
                return Err(denied("comment_publication_withheld"));
            }
            let selected = if let Some(existing) = qualified
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["identityHash"] == identity)
            {
                serde_json::from_value(existing["definitionRef"].clone())
                    .map_err(|_| FoundationError::Invalid)?
            } else {
                let definition = Uuid::new_v4();
                sqlx::query("INSERT INTO linggan_comment_problem_definition(definition_ref,problem_ref,domain_ref,identity_hash,label,actor,goal,barrier,context,source_request_ref,source_signal_ordinal,review_invocation_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)").bind(definition).bind(Uuid::new_v4()).bind(domain).bind(&identity).bind(p.label).bind(p.actor).bind(p.goal).bind(p.barrier).bind(p.context).bind(request).bind(decision.signal_ordinal as i32).bind(invocation).execute(&mut **tx).await?;
                definition
            };
            insert_membership(
                tx,
                supporting_request,
                supporting_ordinal,
                selected,
                invocation,
            )
            .await?;
            Some(selected)
        } else {
            None
        };
        if let Some(definition) = definition {
            insert_membership(tx, request, decision.signal_ordinal, definition, invocation).await?;
        }
    }
    Ok(())
}
async fn insert_membership(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    ordinal: usize,
    definition: Uuid,
    invocation: Uuid,
) -> Result<(), FoundationError> {
    let existing:Option<Uuid>=sqlx::query_scalar("SELECT definition_ref FROM linggan_comment_problem_membership WHERE request_ref=$1 AND signal_ordinal=$2").bind(request).bind(ordinal as i32).fetch_optional(&mut **tx).await?;
    if existing.is_some_and(|id| id != definition) {
        return Err(denied("comment_publication_withheld"));
    }
    sqlx::query("INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,review_invocation_ref) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(request).bind(ordinal as i32).bind(definition).bind(invocation).execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn lock_publication_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
) -> Result<(), FoundationError> {
    // Serialize publication within a domain while retaining withdrawn immutable definitions.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("comment-problem-publication:{domain}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}
