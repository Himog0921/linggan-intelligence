//! Merge and revoke append to the redirect history. Neither command rewrites support.
use super::{ConsolidationDecision, Verdict, publish};
use crate::ai_foundation::store::{Command, FoundationError};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) async fn lock_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
) -> Result<(), FoundationError> {
    publish::lock_domain_in(tx, domain).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('linggan_comment_problem_redirect:'||$1::uuid::text,0))")
        .bind(domain)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(crate) async fn apply_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    invocation: Uuid,
    pairs: &BTreeMap<String, [Uuid; 2]>,
    decisions: &[ConsolidationDecision],
) -> Result<u64, FoundationError> {
    lock_in(tx, domain).await?;
    let mut merged = 0;
    let mut same: Vec<_> = decisions
        .iter()
        .filter(|d| d.verdict == Verdict::Same)
        .collect();
    same.sort_unstable_by_key(|d| &d.pair_key);
    for decision in same {
        let pair = pairs
            .get(&decision.pair_key)
            .ok_or(FoundationError::Invalid)?;
        // Only merge the two definitions the model actually compared. An earlier decision may
        // have retired one of them; do not transfer the decision to a different definition.
        let ranked: Vec<Uuid> = sqlx::query_scalar(
            "SELECT current.problem_ref
             FROM linggan_comment_problem_current current
             JOIN linggan_comment_problem_definition definition USING(problem_ref)
             WHERE current.domain_ref=$1 AND current.problem_ref=ANY($2)
             ORDER BY current.author_count DESC,definition.created_at,current.problem_ref",
        )
        .bind(domain)
        .bind(pair.as_slice())
        .fetch_all(&mut **tx)
        .await?;
        if let [winner, loser] = ranked.as_slice() {
            merged += sqlx::query(
                "INSERT INTO linggan_comment_problem_redirect(redirect_ref,from_problem_ref,to_problem_ref,decision_invocation_ref)
                 VALUES(gen_random_uuid(),$1,$2,$3)",
            )
            .bind(loser)
            .bind(winner)
            .bind(invocation)
            .execute(&mut **tx)
            .await?
            .rows_affected();
        }
    }
    Ok(merged)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokeRedirect {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub redirect_ref: Uuid,
}

pub async fn revoke_redirect(
    db: &Database,
    request: &RevokeRedirect,
) -> Result<Value, FoundationError> {
    let mut command = Command::begin(
        db,
        request.command_ref,
        "comment_problem_redirect_revoke",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_in(&mut command.tx, request.domain_ref).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_problem_redirect redirect
         JOIN linggan_comment_problem_definition definition ON definition.problem_ref=redirect.from_problem_ref
         WHERE redirect.redirect_ref=$1 AND definition.domain_ref=$2)",
    )
    .bind(request.redirect_ref)
    .bind(request.domain_ref)
    .fetch_one(&mut *command.tx)
    .await?;
    if !exists {
        return Err(FoundationError::NotFound);
    }
    let inserted = sqlx::query(
        "INSERT INTO linggan_comment_problem_redirect_revocation(redirect_ref,command_ref)
         VALUES($1,$2) ON CONFLICT (redirect_ref) DO NOTHING",
    )
    .bind(request.redirect_ref)
    .bind(request.command_ref)
    .execute(&mut *command.tx)
    .await?;
    command.finish(json!({"redirectRef":request.redirect_ref,"revoked":true,"changed":inserted.rows_affected()==1})).await
}
