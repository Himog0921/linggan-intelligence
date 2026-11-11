//! Separate read model: comment matches never become the source author's topic membership.
use super::*;
use linggan_storage_postgres::Database;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Query {
    pub domain_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub request_ref: Option<Uuid>,
    pub topic_ref: Option<Uuid>,
}
pub async fn read(db: &Database, query: &Query) -> Result<Value, FoundationError> {
    let rows = sqlx::query(include_str!("read.sql"))
        .bind(query.domain_ref)
        .bind(query.work_ref)
        .bind(query.request_ref)
        .fetch_all(db.pool())
        .await?;
    let catalog = crate::topic_map_core::catalog(db, query.domain_ref)
        .await
        .map_err(|e| match e {
            crate::model_settings::ModelError::Database(e) => FoundationError::Database(e),
            _ => FoundationError::Invalid,
        })?;
    let mut items = Vec::new();
    for row in rows {
        let request: Uuid = row.get("request_ref");
        let mut tx = db.pool().begin().await?;
        let input = match load_in(&mut tx, request).await {
            Ok(v) => Some(v),
            Err(FoundationError::Database(e)) => return Err(e.into()),
            Err(_) => None,
        };
        tx.commit().await?;
        let core: Option<Value> = row.get("core_json");
        let state: String = row.get("assignment_state");
        let context: Option<Value> = row.get("context");
        let versions = catalog_versions(
            &context
                .as_ref()
                .map(|c| c["candidates"].clone())
                .unwrap_or(Value::Null),
        );
        let mut signals = Vec::new();
        if let Some(input) = &input {
            let units = crate::topic_map_core::units_for_discussions(input, &discussions(input));
            for (fragment, (id, _)) in input.fragments.iter().zip(units.iter()) {
                let accepted = core
                    .as_ref()
                    .and_then(|v| v["units"].as_array())
                    .and_then(|units| units.iter().find(|u| u["unitId"] == *id));
                let valid = accepted.is_some_and(|u| assignment_current(u, &versions, &catalog));
                let assignment=accepted.filter(|_|valid).map(|u|json!({"status":u["status"],"matches":u["assignments"],"reason":u["reason"]}));
                if let Some(topic) = query.topic_ref {
                    if !assignment.as_ref().is_some_and(|a| {
                        a["matches"]
                            .as_array()
                            .is_some_and(|m| m.iter().any(|m| m["topicRef"] == json!(topic)))
                    }) {
                        continue;
                    }
                }
                signals.push(json!({"signalOrdinal":fragment.structure["signalOrdinal"],"kind":fragment.structure["signal"]["kind"],"proposition":fragment.structure["signal"]["proposition"],"problemFrame":fragment.structure["signal"]["problemFrame"],"eligibilityState":fragment.structure["signal"]["eligibilityState"],"eligibilityReason":fragment.structure["signal"]["eligibilityReason"],
                    "evidence":{"sourceRef":fragment.source_ref,"start":fragment.start,"end":fragment.end,"text":fragment.text},"assignment":assignment,"assignmentState":if accepted.is_some()&&!valid{"unavailable"}else{state.as_str()}}));
            }
        }
        if query.topic_ref.is_some() && signals.is_empty() {
            continue;
        }
        items.push(json!({"requestRef":request,"windowRef":row.get::<Option<Uuid>,_>("window_ref"),"workRef":row.get::<Uuid,_>("work_ref"),"sourceRef":row.get::<Uuid,_>("source_ref"),"commentExternalId":row.get::<String,_>("comment_external_id"),"sourceAvailable":input.is_some(),"assignmentState":if input.is_none() || signals.iter().any(|s|s["assignmentState"]=="unavailable"){"unavailable"}else{state.as_str()},"signals":signals}));
    }
    Ok(json!({"domainRef":query.domain_ref,"items":items}))
}
