use super::*;
use crate::local_web::ops_native_api::{ResponsibilityAudit, persist_responsibility};
use serde::Serialize;
use sqlx::PgConnection;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Assignment {
    account_ref: Uuid,
    responsibility_version: i64,
    next_member_ref: Uuid,
    group_ref: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DeactivateMemberRequest {
    request_id: Uuid,
    expected_version: i64,
    reason: String,
    // An explicit empty list means a planned offboarding with no active accounts.
    assignments: Option<Vec<Assignment>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    account_ref: Uuid,
    account_name: String,
    platform: String,
    responsibility_version: i64,
}

struct StoredReceipt {
    actor_member_ref: Uuid,
    target_member_ref: Uuid,
    payload_sha256: Vec<u8>,
    outcome: String,
    result_code: String,
    unassigned_account_count: Option<i32>,
    resulting_version: Option<i64>,
    mode: String,
    group_vacancy_ref: Option<Uuid>,
}

async fn read_receipt(
    db: &mut PgConnection,
    request_ref: Uuid,
) -> Result<Option<StoredReceipt>, sqlx::Error> {
    type ReceiptRow = (
        Uuid,
        Uuid,
        Vec<u8>,
        String,
        String,
        Option<i32>,
        Option<i64>,
        String,
        Option<Uuid>,
    );
    Ok(sqlx::query_as::<_, ReceiptRow>(
        "SELECT actor_member_ref,target_member_ref,payload_sha256,outcome,result_code,
                unassigned_account_count,resulting_version,mode,group_vacancy_ref
           FROM linggan_ops_member_deactivation_request WHERE request_ref=$1",
    )
    .bind(request_ref)
    .fetch_optional(db)
    .await?
    .map(
        |(
            actor_member_ref,
            target_member_ref,
            payload_sha256,
            outcome,
            result_code,
            unassigned_account_count,
            resulting_version,
            mode,
            group_vacancy_ref,
        )| StoredReceipt {
            actor_member_ref,
            target_member_ref,
            payload_sha256,
            outcome,
            result_code,
            unassigned_account_count,
            resulting_version,
            mode,
            group_vacancy_ref,
        },
    ))
}

async fn receipt_body(
    db: &mut PgConnection,
    request_ref: Uuid,
    receipt: &StoredReceipt,
    replayed: bool,
) -> Result<Value, sqlx::Error> {
    let transfers: Vec<(Uuid, String, Uuid, String, i64)> =
        if receipt.mode == "planned" && receipt.outcome == "accepted" {
            sqlx::query_as(
                "SELECT change.account_ref, account.account_name, change.next_member_ref,
                        member.display_name, change.resulting_version
                   FROM linggan_ops_account_responsibility_change change
                   JOIN linggan_ops_account account ON account.account_ref=change.account_ref
                   JOIN linggan_ops_member member ON member.member_ref=change.next_member_ref
                  WHERE change.deactivation_request_ref=$1
                  ORDER BY change.account_ref",
            )
            .bind(request_ref)
            .fetch_all(db)
            .await?
        } else {
            vec![]
        };
    let transfers = transfers
        .into_iter()
        .map(|(account, account_name, member, member_name, version)| {
            json!({"accountRef":account,"accountName":account_name,
            "nextMemberRef":member,"nextMemberName":member_name,"resultingVersion":version})
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "requestId": request_ref, "memberRef": receipt.target_member_ref,
        "outcome": receipt.outcome, "code": receipt.result_code, "mode": receipt.mode,
        "unassignedAccountCount": receipt.unassigned_account_count,
        "resultingVersion": receipt.resulting_version, "groupVacancyRef": receipt.group_vacancy_ref,
        "transfers": transfers, "replayed": replayed,
    }))
}

async fn admin(
    state: &LocalWebState,
    headers: &HeaderMap,
) -> Result<AuthenticatedMember, Response> {
    let member = current_member(state, headers).await?;
    if member.role != "admin" {
        return Err(request_error(StatusCode::FORBIDDEN, "ops_auth_forbidden"));
    }
    Ok(member)
}

async fn accounts(
    db: &mut PgConnection,
    member: Uuid,
    lock: bool,
) -> Result<Vec<Account>, sqlx::Error> {
    let query = "SELECT account_ref,account_name,platform,responsibility_version
                   FROM linggan_ops_account
                  WHERE current_responsible_member_ref=$1
                    AND ((source_system='linggan_native' AND native_record_state='active')
                      OR (source_system='content_workbench' AND ledger_state='in_ledger'))
                  ORDER BY account_ref";
    let query = if lock {
        format!("{query} FOR UPDATE")
    } else {
        query.to_owned()
    };
    Ok(
        sqlx::query_as::<_, (Uuid, String, String, i64)>(sqlx::AssertSqlSafe(query))
            .bind(member)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(
                |(account_ref, account_name, platform, responsibility_version)| Account {
                    account_ref,
                    account_name,
                    platform,
                    responsibility_version,
                },
            )
            .collect(),
    )
}

async fn group(db: &mut PgConnection, member: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT team_group.group_ref FROM (
             SELECT group_ref,action FROM linggan_ops_group_membership_event
              WHERE member_ref=$1 ORDER BY occurred_at DESC,event_ref DESC LIMIT 1
         ) membership JOIN linggan_ops_group team_group USING(group_ref)
         WHERE membership.action='joined' AND team_group.state='active'",
    )
    .bind(member)
    .fetch_optional(db)
    .await
}

async fn departing_group(
    db: &mut PgConnection,
    member: Uuid,
    role: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    if role != "lead" {
        return Ok(None);
    }
    let Some(group_ref) = group(db, member).await? else {
        return Ok(None);
    };
    let other_lead: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM linggan_ops_member member
              JOIN LATERAL (SELECT group_ref,action FROM linggan_ops_group_membership_event
                WHERE member_ref=member.member_ref ORDER BY occurred_at DESC,event_ref DESC LIMIT 1) current ON true
             WHERE member.member_ref<>$1 AND member.state='active' AND member.role='lead'
               AND current.group_ref=$2 AND current.action='joined'
        )",
    ).bind(member).bind(group_ref).fetch_one(db).await?;
    Ok((!other_lead).then_some(group_ref))
}

pub(super) async fn preview(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(member_ref): Path<Uuid>,
) -> Response {
    if let Err(response) = admin(&state, &headers).await {
        return response;
    }
    let Some(database) = state.database.database() else {
        return unavailable();
    };
    let result = async {
        let mut db = database.pool().acquire().await?;
        let member: Option<(String, String, i64)> =
            sqlx::query_as("SELECT role,state,version FROM linggan_ops_member WHERE member_ref=$1")
                .bind(member_ref)
                .fetch_optional(&mut *db)
                .await?;
        let Some((role, state, version)) = member else {
            return Ok::<_, sqlx::Error>(None);
        };
        Ok(Some(json!({
            "memberRef": member_ref, "memberState":state, "memberVersion":version,
            "accounts": accounts(&mut db, member_ref, false).await?,
            "groupVacancyRef": departing_group(&mut db, member_ref, &role).await?,
        })))
    }
    .await;
    match result {
        Ok(Some(preview)) => management_json(StatusCode::OK, preview),
        Ok(None) => request_error(StatusCode::NOT_FOUND, "ops_member_not_found"),
        Err(_) => unavailable(),
    }
}

pub(super) async fn receipt(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(request_ref): Path<Uuid>,
) -> Response {
    if let Err(response) = admin(&state, &headers).await {
        return response;
    }
    let Some(database) = state.database.database() else {
        return unavailable();
    };
    let result = async {
        let mut db = database.pool().acquire().await?;
        let Some(receipt) = read_receipt(&mut db, request_ref).await? else {
            return Ok::<_, sqlx::Error>(None);
        };
        receipt_body(&mut db, request_ref, &receipt, false)
            .await
            .map(Some)
    }
    .await;
    match result {
        Ok(Some(body)) => management_json(StatusCode::OK, body),
        Ok(None) => request_error(StatusCode::NOT_FOUND, "ops_deactivation_receipt_not_found"),
        Err(_) => unavailable(),
    }
}

pub(super) async fn deactivate_member(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(target): Path<Uuid>,
    Json(mut request): Json<DeactivateMemberRequest>,
) -> Response {
    let actor = match admin(&state, &headers).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    let reason = request.reason.trim();
    if request.expected_version <= 0
        || reason.is_empty()
        || reason.len() > 1000
        || target == actor.member_ref
    {
        return request_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ops_member_deactivation_invalid",
        );
    }
    let planned = request.assignments.is_some();
    if let Some(assignments) = &mut request.assignments {
        assignments.sort_unstable_by_key(|assignment| assignment.account_ref);
        if assignments
            .windows(2)
            .any(|pair| pair[0].account_ref == pair[1].account_ref)
            || assignments
                .iter()
                .any(|a| a.next_member_ref == target || a.responsibility_version <= 0)
        {
            return request_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "ops_member_handover_invalid",
            );
        }
    }
    let Some(database) = state.database.database() else {
        return unavailable();
    };
    let request_ref = request.request_id;
    let mut payload = json!({"targetMemberRef":target,"expectedVersion":request.expected_version,"reason":reason});
    if planned {
        payload["assignments"] = json!(request.assignments);
    }
    let digest: [u8; 32] = Sha256::digest(payload.to_string().as_bytes()).into();
    let result: Result<Response, sqlx::Error> = async {
        let mut tx = database.pool().begin().await?;
        // A request can be replayed concurrently, even from a different target URL.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text,0))")
            .bind(request_ref.to_string())
            .execute(&mut *tx).await?;
        lock_members(&mut tx, actor.member_ref, target, request.assignments.as_deref()).await?;
        if !lock_active_member(&mut tx, &actor).await? {
            return Ok(request_error(StatusCode::FORBIDDEN, "ops_auth_forbidden"));
        }
        if let Some(stored) = read_receipt(&mut tx, request_ref).await? {
            if stored.actor_member_ref != actor.member_ref || stored.payload_sha256 != digest {
                return Ok(request_error(StatusCode::CONFLICT, "ops_member_deactivation_conflict"));
            }
            let body = receipt_body(&mut tx, request_ref, &stored, true).await?;
            return Ok(management_json(status(&stored.outcome), body));
        }
        let member: Option<(String, String, i64)> = sqlx::query_as(
            "SELECT role,state,version FROM linggan_ops_member WHERE member_ref=$1 FOR UPDATE",
        ).bind(target).fetch_optional(&mut *tx).await?;
        let Some((role, member_state, version)) = member else {
            return Ok(request_error(StatusCode::NOT_FOUND, "ops_member_not_found"));
        };
        let mut outcome = "accepted";
        let mut code = "ops_member_deactivated";
        let mut resulting_version = None;
        let mut group_vacancy_ref = None;
        let mut assignments = Vec::new();
        if member_state == "disabled" {
            outcome = "unchanged";
            code = "ops_member_already_disabled";
            resulting_version = Some(version);
        } else if planned && member_state != "active" {
            outcome = "stale";
            code = "ops_member_handover_changed";
        } else if let Some(plan) = request.assignments.as_deref() {
            if let Some(checked) = validate_plan(&mut tx, target, plan).await? {
                assignments = checked;
            } else {
                outcome = "stale";
                code = "ops_member_handover_changed";
            }
        }
        // The initiating administrator is active, locked, and different from the target.
        if outcome == "accepted" {
            if planned {
                group_vacancy_ref = departing_group(&mut tx, target, &role).await?;
            }
            resulting_version = Some(stop_access(&mut tx, target).await?);
        }
        sqlx::query(
            "INSERT INTO linggan_ops_member_deactivation_request
               (request_ref,actor_member_ref,target_member_ref,expected_version,payload_sha256,reason,
                outcome,result_code,unassigned_account_count,resulting_version,mode,group_vacancy_ref)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        ).bind(request_ref).bind(actor.member_ref).bind(target).bind(request.expected_version)
        .bind(digest.as_slice()).bind(reason).bind(outcome).bind(code)
        .bind((planned && outcome == "accepted").then_some(0_i32))
        .bind(resulting_version).bind(if planned {"planned"} else {"immediate"})
        .bind(group_vacancy_ref).execute(&mut *tx).await?;
        for (account, assignment) in assignments {
            persist_responsibility(
                &mut tx, account.account_ref, Some(target), Some(assignment.next_member_ref),
                account.responsibility_version,
                ResponsibilityAudit { actor_ref: actor.member_ref, request_ref: Uuid::new_v4(),
                    reason, deactivation_request_ref: Some(request_ref) },
            ).await?;
        }
        let stored = read_receipt(&mut tx, request_ref).await?.ok_or(sqlx::Error::RowNotFound)?;
        let body = receipt_body(&mut tx, request_ref, &stored, false).await?;
        tx.commit().await?;
        Ok(management_json(status(outcome), body))
    }.await;
    result.unwrap_or_else(|_| unavailable())
}

fn status(outcome: &str) -> StatusCode {
    if matches!(outcome, "accepted" | "unchanged") {
        StatusCode::OK
    } else {
        StatusCode::CONFLICT
    }
}

async fn lock_members(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    target: Uuid,
    plan: Option<&[Assignment]>,
) -> Result<(), sqlx::Error> {
    let mut refs = vec![actor, target];
    if let Some(plan) = plan {
        refs.extend(plan.iter().map(|assignment| assignment.next_member_ref));
    }
    refs.sort_unstable();
    refs.dedup();
    for member_ref in refs {
        let sql = if member_ref == target {
            "SELECT member_ref FROM linggan_ops_member WHERE member_ref=$1 FOR UPDATE"
        } else {
            "SELECT member_ref FROM linggan_ops_member WHERE member_ref=$1 FOR SHARE"
        };
        sqlx::query_scalar::<_, Uuid>(sql)
            .bind(member_ref)
            .fetch_optional(&mut **tx)
            .await?;
    }
    Ok(())
}

async fn validate_plan<'a>(
    tx: &mut Transaction<'_, Postgres>,
    target: Uuid,
    plan: &'a [Assignment],
) -> Result<Option<Vec<(Account, &'a Assignment)>>, sqlx::Error> {
    // Member locks are held before this check and before any account locks.
    // Account creation takes a shared lock on the author, preventing a phantom assignment.
    for assignment in plan {
        let active = sqlx::query_scalar::<_, Uuid>(
            "SELECT member_ref FROM linggan_ops_member WHERE member_ref=$1 AND state='active'",
        )
        .bind(assignment.next_member_ref)
        .fetch_optional(&mut **tx)
        .await?
        .is_some();
        if !active || group(tx, assignment.next_member_ref).await? != assignment.group_ref {
            return Ok(None);
        }
    }
    let live = accounts(tx, target, true).await?;
    if live.len() != plan.len()
        || live.iter().zip(plan).any(|(actual, expected)| {
            actual.account_ref != expected.account_ref
                || actual.responsibility_version != expected.responsibility_version
        })
    {
        return Ok(None);
    }
    Ok(Some(live.into_iter().zip(plan).collect()))
}

async fn stop_access(tx: &mut Transaction<'_, Postgres>, member: Uuid) -> Result<i64, sqlx::Error> {
    let version = sqlx::query_scalar(
        "UPDATE linggan_ops_member SET state='disabled',disabled_at=scope_001_now(),version=version+1
          WHERE member_ref=$1 RETURNING version",
    ).bind(member).fetch_one(&mut **tx).await?;
    for query in [
        "UPDATE linggan_ops_session SET revoked_at=scope_001_now(),revoked_reason='member_disabled'
          WHERE member_ref=$1 AND revoked_at IS NULL",
        "UPDATE linggan_ops_invite SET revoked_at=scope_001_now(),revoked_reason='member_disabled'
          WHERE member_ref=$1 AND accepted_at IS NULL AND revoked_at IS NULL",
        "UPDATE linggan_ops_password_reset SET revoked_at=scope_001_now(),revoked_reason='member_disabled'
          WHERE member_ref=$1 AND consumed_at IS NULL AND revoked_at IS NULL",
    ] {
        sqlx::query(query).bind(member).execute(&mut **tx).await?;
    }
    Ok(version)
}
