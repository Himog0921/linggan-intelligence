//! Validate model equivalence against server-frozen candidate definitions and evidence.

use super::{problem_metadata, problem_version, rejected, statement};
use linggan_storage_postgres::StorageError;
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

fn candidate_proposal(result: &Value, candidate: &Value) -> Value {
    let Some(problem) = candidate["candidateRef"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return if candidate["boundaryMatch"] == true || !candidate["candidateRef"].is_null() {
            json!({"accepted":false,"rejectionReason":"missing_mapped_candidate"})
        } else {
            Value::Null
        };
    };
    let Some(revision) = candidate["candidateRevision"].as_i64().filter(|r| *r > 0) else {
        return json!({"candidateRef":problem,"accepted":false,"rejectionReason":"invalid_definition_revision"});
    };
    let snapshot = result["candidateSnapshot"].as_array().and_then(|items| {
        items.iter().find(|item| {
            item["problemRef"] == json!(problem) && item["revision"] == json!(revision)
        })
    });
    let Some(snapshot) = snapshot else {
        return json!({"candidateRef":problem,"candidateRevision":revision,"accepted":false,"rejectionReason":"not_in_server_candidate_snapshot"});
    };
    let reason = candidate["equivalenceReason"].as_str().unwrap_or("");
    let valid_reason = !reason.trim().is_empty()
        && reason.chars().count() <= 1000
        && !reason.chars().any(|c| c.is_control());
    let sources = snapshot["sourceRefs"].as_array();
    let refs = result
        .pointer("/contextRefs/researchSourceRefs")
        .and_then(Value::as_array);
    let valid_sources = sources.is_some_and(|sources| {
        !sources.is_empty()
            && sources.len() <= 30
            && sources.iter().all(|source| {
                source
                    .as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .is_some()
                    && refs.is_some_and(|refs| refs.contains(source))
            })
    });
    let accepted = candidate["serverValidatedCandidate"] == true
        && candidate["boundaryMatch"] == true
        && valid_reason
        && valid_sources
        && candidate["retrievalMethod"] == "lexical_terms.v1";
    json!({"candidateRef":problem,"candidateRevision":revision,"equivalenceReason":reason,"boundaryMatch":candidate["boundaryMatch"],
        "retrievalMethod":candidate["retrievalMethod"],"serverValidatedCandidate":candidate["serverValidatedCandidate"],
        "definitionFingerprint":snapshot["definitionFingerprint"],"sourceRefs":snapshot["sourceRefs"],"accepted":accepted,
        "rejectionReason":if accepted {Value::Null}else{json!("candidate_contract_not_verified")}})
}

pub(super) async fn assign_model_equivalence(
    connection: &mut PgConnection,
    domain: Uuid,
    canonical: Uuid,
    analysis: Uuid,
    candidate: Uuid,
    evidence: &Value,
    proposal: &Value,
) -> Result<bool, StorageError> {
    if proposal["accepted"] != true {
        return Ok(false);
    }
    let problem = proposal["candidateRef"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| rejected("CI_INVALID_STATE"))?;
    let current = match problem_metadata(connection, domain, problem).await {
        Ok(current) => current,
        Err(StorageError::Statement(sqlx::Error::Protocol(code)))
            if code == "CI_PROBLEM_UNAVAILABLE" =>
        {
            reject_proposal(
                connection,
                candidate,
                "candidate_domain_or_identity_unavailable",
            )
            .await?;
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let fingerprint=crate::comment_research::comment_source_hash(&json!({"name":current["name"],"meaning":current["meaning"],"definition":current["definition"]}).to_string());
    if !current["redirectRef"].is_null()
        || current["definitionRevision"] != proposal["candidateRevision"]
        || proposal["definitionFingerprint"] != json!(fingerprint)
    {
        reject_proposal(connection, candidate, "candidate_definition_changed").await?;
        return Ok(false);
    }
    let sources: Vec<Uuid> = serde_json::from_value(proposal["sourceRefs"].clone())
        .map_err(|_| rejected("CI_INVALID_STATE"))?;
    let readable: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_readable WHERE material_ref=ANY($1)",
    )
    .bind(&sources)
    .fetch_one(&mut *connection)
    .await
    .map_err(statement)?;
    if readable != sources.len() as i64 {
        reject_proposal(connection, candidate, "candidate_evidence_unavailable").await?;
        return Ok(false);
    }
    let locked:bool=sqlx::query_scalar("SELECT COALESCE((SELECT locked FROM linggan_ci_source_research WHERE canonical_ref=$1 AND domain_ref=$2),false)")
        .bind(canonical).bind(domain).fetch_one(&mut *connection).await.map_err(statement)?;
    if locked {
        reject_proposal(connection, candidate, "human_membership_locked").await?;
        return Ok(false);
    }
    let inserted=sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,evidence,analysis_ref) VALUES($1,$2,'model_equivalence',$3,$4) ON CONFLICT(problem_ref,canonical_ref) DO NOTHING")
        .bind(problem).bind(canonical).bind(evidence).bind(analysis).execute(&mut *connection).await.map_err(statement)?;
    sqlx::query("UPDATE linggan_ci_problem_candidate SET state='assigned',problem_ref=$2 WHERE candidate_ref=$1")
        .bind(candidate).bind(problem).execute(&mut *connection).await.map_err(statement)?;
    if inserted.rows_affected() > 0 {
        sqlx::query("UPDATE linggan_ci_problem SET revision=revision+1,updated_at=scope_001_now() WHERE problem_ref=$1")
            .bind(problem).execute(&mut *connection).await.map_err(statement)?;
        let mut after = problem_metadata(connection, domain, problem).await?;
        after["addedMembers"] = json!([canonical]);
        after["candidateRef"] = json!(candidate);
        problem_version(
            connection,
            problem,
            None,
            "model_equivalence_membership",
            proposal["equivalenceReason"].as_str().unwrap_or(""),
            &current,
            &after,
        )
        .await?;
    }
    Ok(inserted.rows_affected() > 0)
}

async fn reject_proposal(
    connection: &mut PgConnection,
    candidate: Uuid,
    reason: &str,
) -> Result<(), StorageError> {
    sqlx::query("UPDATE linggan_ci_problem_candidate SET proposal=proposal || jsonb_build_object('accepted',false,'rejectionReason',$2::text) WHERE candidate_ref=$1")
        .bind(candidate).bind(reason).execute(&mut *connection).await.map_err(statement)?;
    Ok(())
}

pub(super) fn validated_candidates(
    result: &Value,
    source_ref: Uuid,
    body: &str,
) -> Vec<(usize, String, String, Value, Value)> {
    if result.pointer("/semantic/outcome").and_then(Value::as_str) != Some("interpretable") {
        return Vec::new();
    }
    let Some(candidates) = result
        .pointer("/semantic/problems")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (ordinal, candidate) in candidates.iter().take(12).enumerate() {
        let (Some(name), Some(meaning), Some(evidence)) = (
            candidate["name"].as_str(),
            candidate["meaning"].as_str(),
            candidate["evidence"].as_array(),
        ) else {
            continue;
        };
        if name.trim().is_empty()
            || meaning.trim().is_empty()
            || name.chars().count() > 120
            || meaning.chars().count() > 1000
            || name.chars().any(|c| c.is_control())
            || meaning.chars().any(|c| c.is_control())
            || evidence.is_empty()
            || evidence.len() > 8
        {
            continue;
        }
        if !evidence.iter().all(|e| {
            e["sourceRef"]
                .as_str()
                .and_then(|r| Uuid::parse_str(r).ok())
                == Some(source_ref)
                && e["startChar"]
                    .as_i64()
                    .zip(e["endChar"].as_i64())
                    .is_some_and(|(a, b)| a >= 0 && b > a && (b as usize) <= body.chars().count())
        }) {
            continue;
        }
        out.push((
            ordinal,
            name.trim().to_owned(),
            meaning.trim().to_owned(),
            json!(evidence),
            candidate_proposal(result, candidate),
        ));
    }
    out
}
