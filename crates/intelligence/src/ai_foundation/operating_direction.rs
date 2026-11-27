//! Model-authored proposals are separate from reviewed, versioned operating catalogs.
use super::{
    TaskKey,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
};
use crate::topic_map::{TopicMapSnapshot, directions::Catalog};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

mod output;
mod read;
mod source;
pub(crate) use output::{accepted, schema};
pub use read::{latest, readable_candidates_in};
use source::{current_evidence, packet, support_pairs};
pub(crate) const CONTRACT: &str = "operating-direction.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Generate {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub task_key: TaskKey,
    #[serde(default)]
    pub topic_refs: BTreeSet<Uuid>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Topic {
    pub definition_ref: Uuid,
    pub label: String,
    pub work_refs: BTreeSet<Uuid>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Packet {
    pub task: TaskKey,
    pub domain_ref: Uuid,
    pub topics: BTreeMap<Uuid, Topic>,
    pub evidence: BTreeMap<Uuid, Vec<Value>>,
    pub existing: BTreeMap<Uuid, Value>,
}

impl Packet {
    pub(crate) fn prompt_input(&self) -> Value {
        // The immutable packet retains full citation refs. The model only references works
        // and topics, so avoid repeating opaque, unselectable fragment IDs on the wire.
        let evidence: BTreeMap<_, Vec<_>> = self
            .evidence
            .iter()
            .map(|(work, fragments)| {
                (
                    work,
                    fragments
                        .iter()
                        .map(|f| json!({"field":f["field"],"text":f["text"]}))
                        .collect(),
                )
            })
            .collect();
        let topics: BTreeMap<_, _> = self
            .topics
            .iter()
            .map(|(id, t)| (id, json!({"label":t.label,"workRefs":t.work_refs})))
            .collect();
        json!({"task":self.task,"topics":topics,"existing":self.existing,"evidence":evidence})
    }
}

pub async fn generate(
    db: &Database,
    request: Generate,
    snapshot: &TopicMapSnapshot,
) -> Result<Value, FoundationError> {
    if !matches!(
        request.task_key,
        TaskKey::DirectionListDraft | TaskKey::DirectionProposal
    ) || (request.task_key == TaskKey::DirectionProposal && request.topic_refs.is_empty())
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "operating_direction_generate",
        &request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let config = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key=$1 FOR SHARE",
    )
    .bind(request.task_key.as_str())
    .fetch_one(&mut *command.tx)
    .await?;
    let run = Uuid::new_v4();
    let items = [BatchInput {
        item_key: run.to_string(),
        input: InputRef::OperatingDirection { run_ref: run },
    }];
    let hash = batch_store::input_hash(request.task_key, &items, false)?;
    let batch = batch_store::insert_batch(
        &mut command.tx,
        request.command_ref,
        request.task_key,
        config,
        BatchKind::Interactive,
        &items,
        &hash,
    )
    .await?;
    let p = packet(&mut command.tx, &request, snapshot, config, None).await?;
    sqlx::query("INSERT INTO linggan_operating_direction_run(run_ref,domain_ref,batch_ref,frozen_input) VALUES($1,$2,$3,$4)")
        .bind(run).bind(request.domain_ref).bind(batch).bind(sqlx::types::Json(&p)).execute(&mut *command.tx).await?;
    command
        .finish(json!({"runRef":run,"batchRef":batch,"taskKey":request.task_key,"state":"pending"}))
        .await
}

pub(crate) async fn check_source(
    tx: &mut Transaction<'_, Postgres>,
    p: &Packet,
    config: Uuid,
) -> Result<(), FoundationError> {
    let snap = current_snapshot(tx, p.domain_ref).await?;
    check_in_snapshot(tx, p, config, &snap).await
}

async fn current_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
) -> Result<TopicMapSnapshot, FoundationError> {
    crate::topic_map::read_topic_map_in(
        tx,
        &crate::topic_map::TopicMapQuery {
            domain_ref: Some(domain),
            ..Default::default()
        },
        None,
    )
    .await
    .map_err(|e| match e {
        crate::topic_map::TopicMapError::Database(sql) => FoundationError::Database(sql),
        _ => FoundationError::NotFound,
    })
}
async fn check_in_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    p: &Packet,
    config: Uuid,
    snapshot: &TopicMapSnapshot,
) -> Result<(), FoundationError> {
    let req = Generate {
        command_ref: Uuid::nil(),
        task_key: p.task,
        domain_ref: p.domain_ref,
        topic_refs: p.topics.keys().copied().collect(),
    };
    if packet(tx, &req, snapshot, config, Some(p)).await? != *p {
        return Err(FoundationError::AdmissionDenied(vec![
            "direction_source_changed".into(),
        ]));
    }
    Ok(())
}
