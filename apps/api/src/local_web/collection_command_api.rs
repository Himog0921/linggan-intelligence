//! JSON transport for the existing Collection commands. Domain owners still decide effects.
//!
//! Old management commands have no durable request receipt. A process epoch prevents their
//! unknown requests being re-executed after a restart; absence is reported as unconfirmed.
//! Existing monitor-rule/acquisition receipts remain the durable business authorities.
use super::*;
use axum::{extract::Form, http::StatusCode};
use sha2::{Digest, Sha256};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

const MAX_IDENTITIES: usize = 20_000;

struct Entry {
    fingerprint: String,
    action: String,
    answer: Option<(StatusCode, Value)>,
}
static EPOCH: OnceLock<Uuid> = OnceLock::new();
static COMMANDS: OnceLock<Mutex<HashMap<Uuid, Entry>>> = OnceLock::new();
fn epoch() -> Uuid {
    *EPOCH.get_or_init(Uuid::new_v4)
}
fn commands() -> &'static Mutex<HashMap<Uuid, Entry>> {
    COMMANDS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/collection/commands/context", get(context))
        .route(
            "/api/collection/commands/receipts/{request}",
            get(read_receipt),
        )
        .route("/api/collection/commands/{action}", post(execute))
}
async fn context() -> Json<Value> {
    Json(json!({"epoch":epoch()}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Command {
    request_ref: Uuid,
    epoch: Uuid,
    fields: serde_json::Map<String, Value>,
}
#[derive(Deserialize)]
struct ReceiptQuery {
    action: String,
}

fn answer(
    id: Uuid,
    action: &str,
    outcome: &str,
    code: Option<&str>,
    location: Option<&str>,
) -> Value {
    json!({"requestRef":id,"action":action,"outcome":outcome,"code":code,"location":location})
}
fn respond(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}
fn reject(id: Uuid, action: &str, code: &str) -> (StatusCode, Value) {
    let status = if code.contains("unavailable")
        || code.contains("not_connected")
        || code.contains("database")
        || code.ends_with("_failed")
    {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::CONFLICT
    };
    (
        status,
        answer(
            id,
            action,
            if status.is_server_error() {
                "unconfirmed"
            } else {
                "rejected"
            },
            Some(code),
            None,
        ),
    )
}
fn known_action(action: &str) -> bool {
    matches!(
        action,
        "new"
            | "rules"
            | "monitoring"
            | "archive"
            | "retire-materials"
            | "patrol-toggle"
            | "detail-gaps-exact"
            | "monitor-rules-retire"
            | "delete"
            | "domain"
            | "batch"
            | "domain-save"
            | "domain-target"
            | "runtime-register"
            | "runtime-rename"
            | "runtime-claim-window"
            | "runtime-close-window"
            | "runtime-retire"
            | "runtime-claims"
            | "runtime-accepting"
            | "runtime-account-bindings"
    )
}

async fn read_receipt(Path(id): Path<Uuid>, Query(q): Query<ReceiptQuery>) -> Response {
    let cache = commands()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match cache.get(&id) {
        Some(entry) if entry.action != q.action => {
            let (status, value) = reject(id, &q.action, "request_identity_conflict");
            respond(status, value)
        }
        Some(entry) => match &entry.answer {
            Some((status, value)) => respond(*status, value.clone()),
            None => respond(
                StatusCode::ACCEPTED,
                answer(id, &q.action, "pending", None, None),
            ),
        },
        None => respond(
            StatusCode::OK,
            answer(
                id,
                &q.action,
                "unconfirmed",
                Some("command_result_unconfirmed"),
                None,
            ),
        ),
    }
}

async fn execute(
    State(state): State<LocalWebState>,
    Path(action): Path<String>,
    Json(input): Json<Command>,
) -> Response {
    let id = input.request_ref;
    if !known_action(&action) {
        return respond(
            StatusCode::NOT_FOUND,
            answer(id, &action, "rejected", Some("command_unknown"), None),
        );
    }
    if input.epoch != epoch() {
        return respond(
            StatusCode::OK,
            answer(
                id,
                &action,
                "unconfirmed",
                Some("command_server_changed"),
                None,
            ),
        );
    }
    let fingerprint = Sha256::digest(
        serde_json::to_vec(&json!({"action":action,"fields":input.fields})).unwrap_or_default(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();
    {
        let mut cache = commands()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = cache.get(&id) {
            if entry.fingerprint != fingerprint {
                let (status, value) = reject(id, &action, "request_identity_conflict");
                return respond(status, value);
            }
            return match &entry.answer {
                Some((status, value)) => respond(*status, value.clone()),
                None => respond(
                    StatusCode::ACCEPTED,
                    answer(id, &action, "pending", None, None),
                ),
            };
        }
        // Never evict a used identity and accidentally make it executable again.
        if cache.len() >= MAX_IDENTITIES {
            let (status, value) = reject(id, &action, "command_capacity_unavailable");
            return respond(status, value);
        }
        cache.insert(
            id,
            Entry {
                fingerprint,
                action: action.clone(),
                answer: None,
            },
        );
    }
    let action_copy = action.clone();
    // Completing the command is independent of whether the browser receives this response.
    let operation = tokio::spawn(async move {
        let result = run(&state, &action_copy, id, Value::Object(input.fields)).await;
        let mut cache = commands()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = cache.get_mut(&id) {
            entry.answer = Some(result.clone());
        }
        result
    });
    match operation.await {
        Ok((status, value)) => respond(status, value),
        Err(_) => respond(
            StatusCode::OK,
            answer(
                id,
                &action,
                "unconfirmed",
                Some("command_result_unconfirmed"),
                None,
            ),
        ),
    }
}

fn normalize(mut fields: Value) -> Value {
    if let Some(values) = fields.as_object_mut() {
        // Legacy checkbox wires use presence; JSON carries an explicit boolean.
        // Omit unchecked options so false cannot enable background work.
        for key in ["automatic_enabled", "creator_follow_details"] {
            if values.get(key).is_some_and(|value| {
                value == &Value::Bool(false) || value.as_str() == Some("false")
            }) {
                values.remove(key);
            }
        }
        for (key, value) in values.iter_mut() {
            if matches!(key.as_str(), "expected_revision" | "valid_for_hours") {
                if let Some(parsed) = value.as_str().and_then(|s| s.parse::<i64>().ok()) {
                    *value = json!(parsed);
                }
            } else if key == "accepting" {
                if let Some(parsed) = value.as_str().and_then(|s| s.parse::<bool>().ok()) {
                    *value = json!(parsed);
                }
            } else if value.is_number() || value.is_boolean() {
                *value = Value::String(value.to_string());
            }
        }
    }
    fields
}

fn pairs_body(fields: &Value) -> Bytes {
    let mut pairs = Vec::new();
    if let Some(fields) = fields.as_object() {
        for (key, value) in fields {
            let values = value
                .as_array()
                .map_or_else(|| vec![value], |items| items.iter().collect());
            for value in values {
                if value.is_null() {
                    continue;
                }
                let value = value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string());
                pairs.push(format!(
                    "{}={}",
                    encode_query_value(key),
                    encode_query_value(&value)
                ));
            }
        }
    }
    Bytes::from(pairs.join("&"))
}

async fn run(state: &LocalWebState, action: &str, id: Uuid, fields: Value) -> (StatusCode, Value) {
    if action.starts_with("runtime-") || action.starts_with("domain-") {
        return match collection_management_api::command(state, action, &fields).await {
            Ok(location) => (
                StatusCode::OK,
                answer(id, action, "applied", None, location.as_deref()),
            ),
            Err(code) => reject(id, action, &code),
        };
    }
    let mut fields = normalize(fields);
    if action == "rules" {
        fields["idempotency_key"] = json!(id);
    }
    macro_rules! form {
        ($owner:ident, $kind:ty) => {{
            let Ok(value) = serde_json::from_value::<$kind>(fields.clone()) else {
                return reject(id, action, "command_fields_invalid");
            };
            $owner(State(state.clone()), Form(value))
                .await
                .into_response()
        }};
    }
    let response = match action {
        "new" => form!(collection_target_create, NewTargetForm),
        "rules" => form!(collection_target_rule_command, MonitorRuleWire),
        "monitoring" => form!(collection_target_toggle_monitoring, MonitoringForm),
        "archive" => form!(collection_target_deep_archive, TargetArchiveForm),
        "monitor-rules-retire" => form!(collection_monitor_rule_retire, MonitorRuleRetireForm),
        "retire-materials" => {
            collection_retire_materials(State(state.clone()), pairs_body(&fields))
                .await
                .into_response()
        }
        "detail-gaps-exact" => {
            #[derive(serde::Deserialize)]
            struct ExactGapFields {
                row_target_ref: Uuid,
                domain_ref: Option<Uuid>,
                content_public_ref: Vec<Uuid>,
            }
            let Ok(form) = serde_json::from_value::<ExactGapFields>(fields.clone()) else {
                return reject(id,action,"command_fields_invalid");
            };
            let Some(db)=state.database.database() else {
                return reject(id,action,"read_model_not_connected");
            };
            let scope=linggan_evidence::target_inspection::Scope{
                target_ref:form.row_target_ref,
                domain_ref:form.domain_ref,
                knowledge_cutoff:None,
            };
            return match linggan_evidence::target_inspection::request_detail_gaps(
                db,&scope,&form.content_public_ref,
            ).await {
                Ok(receipt) if receipt.work_order_ref.is_some() => (
                    StatusCode::OK,answer(id,action,"applied",Some("detail_gaps_queued"),None),
                ),
                Ok(receipt) => {
                    let code=format!("detail_gaps_{}",receipt.admission);
                    reject(id,action,&code)
                }
                Err(linggan_evidence::target_inspection::Error::Invalid(_)) => 
                    reject(id,action,"detail_gap_selection_stale"),
                Err(linggan_evidence::target_inspection::Error::Scope) =>
                    reject(id,action,"target_domain_unassigned"),
                Err(linggan_evidence::target_inspection::Error::NotFound) =>
                    reject(id,action,"detail_gaps_target_missing"),
                Err(_) => reject(id,action,"detail_gaps_unavailable"),
            };
        }
        "patrol-toggle" => {
            collection_target_patrol_toggle(State(state.clone()), pairs_body(&fields))
                .await
                .into_response()
        }
        "delete" => collection_target_delete(State(state.clone()), pairs_body(&fields))
            .await
            .into_response(),
        "domain" => collection_target_assign_domain(State(state.clone()), pairs_body(&fields))
            .await
            .into_response(),
        "batch" => collection_targets_batch(State(state.clone()), pairs_body(&fields))
            .await
            .into_response(),
        _ => return reject(id, action, "command_unknown"),
    };
    let Some(location) = response
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
    else {
        return reject(id, action, "command_answer_unavailable");
    };
    let query = location
        .split_once('?')
        .map(|(_, value)| value.split('#').next().unwrap_or(value))
        .unwrap_or("");
    let code = parse_form_pairs(query.as_bytes())
        .into_iter()
        .find_map(|(key, value)| (key == "error").then_some(value));
    if let Some(code) = code.as_deref() {
        if !matches!(
            code,
            "target_domain_assigned"
                | "archive_requested"
                | "creator_detail_requested"
                | "keyword_detail_requested"
                | "keyword_detail_complete"
                | "monitor_rule_retired"
                | "monitor_rule_paused"
                | "monitor_rule_resumed"
                | "monitor_rule_saved"
                | "monitor_rule_command_replayed"
                | "target_deleted"
                | "patrol_paused"
                | "patrol_resumed"
                | "material_retirement_done"
                | "archive_baseline_complete"
        ) {
            let (status, mut value) = reject(id, action, code);
            value["location"] = json!(location);
            return (status, value);
        }
    }
    (
        StatusCode::OK,
        answer(id, action, "applied", code.as_deref(), Some(location)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    #[test]
    fn unchecked_json_options_remain_absent_in_the_legacy_presence_wire() {
        let fields = normalize(json!({"automatic_enabled":false,"creator_follow_details":false}));
        assert!(fields.get("automatic_enabled").is_none());
        assert!(fields.get("creator_follow_details").is_none());
        let fields = normalize(json!({"automatic_enabled":true,"creator_follow_details":true}));
        assert_eq!(fields["automatic_enabled"], "true");
        assert_eq!(fields["creator_follow_details"], "true");
    }
    async fn send(app: Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
        let reply = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::HOST, "127.0.0.1:8080")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = reply.status();
        let bytes = to_bytes(reply.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    #[tokio::test]
    async fn an_old_process_epoch_is_unconfirmed_and_never_reserves_or_executes() {
        let id = Uuid::new_v4();
        let (status, r) = send(
            app(),
            "POST",
            "/api/collection/commands/new",
            json!({"requestRef":id,"epoch":Uuid::new_v4(),"fields":{}}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(r["outcome"], "unconfirmed");
        assert!(!commands().lock().unwrap().contains_key(&id));
    }
    #[tokio::test]
    async fn rejected_request_replays_the_same_receipt_and_conflicting_intent_cannot_replace_it() {
        let id = Uuid::new_v4();
        let request = json!({"requestRef":id,"epoch":epoch(),"fields":{}});
        let first = send(
            app(),
            "POST",
            "/api/collection/commands/new",
            request.clone(),
        )
        .await;
        assert_eq!(first.0, StatusCode::CONFLICT);
        assert_eq!(first.1["code"], "command_fields_invalid");
        assert_eq!(
            send(app(), "POST", "/api/collection/commands/new", request).await,
            first
        );
        let receipt_path = format!("/api/collection/commands/receipts/{id}?action=new");
        assert_eq!(send(app(), "GET", &receipt_path, Value::Null).await, first);
        let other = send(
            app(),
            "POST",
            "/api/collection/commands/new",
            json!({"requestRef":id,"epoch":epoch(),"fields":{"identity":"different"}}),
        )
        .await;
        assert_eq!(other.1["code"], "request_identity_conflict");
        assert_eq!(send(app(), "GET", &receipt_path, Value::Null).await, first);
    }
    #[tokio::test]
    async fn missing_receipt_is_unknown_and_foreign_origin_cannot_touch_commands() {
        let id = Uuid::new_v4();
        let r = send(
            app(),
            "GET",
            &format!("/api/collection/commands/receipts/{id}?action=new"),
            Value::Null,
        )
        .await;
        assert_eq!(r.1["outcome"], "unconfirmed");
        let reply = app()
            .oneshot(
                Request::builder()
                    .uri("/api/collection/commands/context")
                    .header(header::HOST, "evil.example:8080")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(reply.status(), StatusCode::FORBIDDEN);
    }
}
