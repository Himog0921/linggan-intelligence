//! Apply an administrator's decision to a frozen label and publish its scoped snapshot atomically.
use super::{
    AnalysisInput, LabelDimension, LensDefinitionError as Error, SnapshotReceipt, SnapshotScope,
    snapshot::build_in,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Adjudication {
    pub command_ref: Uuid,
    pub snapshot_ref: Uuid,
    pub title_key: String,
    pub dimension: LabelDimension,
    pub value: String,
    pub reason: Option<String>,
}

pub async fn adjudicate(
    db: &Database,
    member: Uuid,
    request: Adjudication,
) -> Result<SnapshotReceipt, Error> {
    if !super::example_titles::is_key(&request.title_key)
        || request
            .reason
            .as_ref()
            .is_some_and(|r| r.chars().count() > 300)
    {
        return Err(Error::InvalidScope);
    }
    let payload = json!({"memberRef":member,"decision":request});
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    // Rare human writes: queue before taking the MVCC snapshot so two decisions cannot
    // both replace the same frozen value. Reads remain available throughout.
    sqlx::query("LOCK TABLE linggan_ops_lens_adjudication IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let replay: Option<(String, Value, Value)> = sqlx::query_as(
        "SELECT operation,payload,receipt FROM linggan_ai_command WHERE command_ref=$1",
    )
    .bind(request.command_ref)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((op, previous, receipt)) = replay {
        return if op == "ops_lens_adjudicate" && previous == payload {
            serde_json::from_value(receipt).map_err(|_| Error::InvalidDefinition)
        } else {
            Err(Error::AdjudicationConflict)
        };
    }
    let (Json(input),Json(taxonomy)):(Json<AnalysisInput>,Json<Value>)=sqlx::query_as(
        "SELECT s.analysis_input,d.body FROM linggan_ops_lens_snapshot s JOIN linggan_ops_lens_definition d ON d.definition_ref=s.taxonomy_ref WHERE s.snapshot_ref=$1"
    ).bind(request.snapshot_ref).fetch_optional(&mut *tx).await?.ok_or(Error::SnapshotNotFound)?;
    let observed = input
        .publications
        .values()
        .find(|p| p.title_key == request.title_key)
        .and_then(|p| p.labels.get(&request.dimension))
        .ok_or(Error::InvalidScope)?;
    let dim = match request.dimension {
        LabelDimension::Moment => "moment",
        LabelDimension::Offer => "offer",
        LabelDimension::Situation => "situation",
        LabelDimension::Audience => "audience",
    };
    if !taxonomy[dim].as_array().is_some_and(|choices| {
        choices
            .iter()
            .any(|entry| entry["key"].as_str() == Some(&request.value))
    }) {
        return Err(Error::InvalidScope);
    }
    let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT FROM linggan_ops_lens_effective_label l
      JOIN linggan_ops_lens_definition_current t ON t.kind='taxonomy' AND t.definition_ref=l.taxonomy_ref
      JOIN linggan_ai_task_setting s ON s.task_key='ops_title_label' AND s.current_config_ref=l.task_config_ref
      WHERE l.taxonomy_ref=$1 AND l.task_config_ref=$2 AND l.title_key=$3 AND l.dimension=$4 AND l.run_ref=$5
        AND l.adjudication_ref IS NOT DISTINCT FROM $6)")
        .bind(input.method.taxonomy_ref).bind(input.method.task_config_ref).bind(&request.title_key).bind(dim)
        .bind(observed.run_ref).bind(observed.adjudication_ref).fetch_one(&mut *tx).await?;
    if !current {
        return Err(Error::AdjudicationConflict);
    }
    sqlx::query("INSERT INTO linggan_ops_lens_adjudication(adjudication_ref,title_key,taxonomy_ref,dimension,value,member_ref,reason,created_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,clock_timestamp())")
        .bind(request.command_ref).bind(&request.title_key).bind(input.method.taxonomy_ref).bind(dim)
        .bind(&request.value).bind(member).bind(request.reason.as_deref().map(str::trim).filter(|s|!s.is_empty())).execute(&mut *tx).await?;
    let scope = SnapshotScope {
        domain_ref: input.domain_ref,
        from: input.from,
        to: input.to,
    };
    let receipt = build_in(&mut tx, &scope, "ops_title_label").await?;
    let applied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT FROM linggan_ops_lens_snapshot s, jsonb_each(s.analysis_input->'publications') p
      WHERE s.snapshot_ref=$1 AND p.value->>'titleKey'=$2 AND p.value->'labels'->$3->>'adjudicationRef'=$4)")
        .bind(receipt.snapshot_ref).bind(&request.title_key).bind(dim).bind(request.command_ref.to_string()).fetch_one(&mut *tx).await?;
    if !applied {
        return Err(Error::AdjudicationConflict);
    }
    sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,'ops_lens_adjudicate',$2,$3)")
        .bind(request.command_ref).bind(payload).bind(Json(&receipt)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(receipt)
}
