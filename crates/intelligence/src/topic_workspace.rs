use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
use std::fmt::Write;
use thiserror::Error;
use uuid::Uuid;

const MAX_MEMBERS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicMaterialRole {
    Support,
    Challenge,
    Boundary,
}

impl TopicMaterialRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Support => "support",
            Self::Challenge => "challenge",
            Self::Boundary => "boundary",
        }
    }

    fn parse(value: &str) -> Result<Self, TopicWorkspaceError> {
        match value {
            "support" => Ok(Self::Support),
            "challenge" => Ok(Self::Challenge),
            "boundary" => Ok(Self::Boundary),
            _ => Err(TopicWorkspaceError::CorruptProjection),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMaterialMemberImport {
    pub work_public_ref: Uuid,
    pub role: TopicMaterialRole,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicWorkspaceImport {
    pub idempotency_key: String,
    pub domain_key: String,
    pub canonical_key: String,
    pub display_name: String,
    pub definition_text: String,
    pub expected_version: Option<i32>,
    pub adjudication_note: String,
    pub source_boundary: String,
    pub members: Vec<TopicMaterialMemberImport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicWorkspaceReceipt {
    pub receipt_ref: Uuid,
    pub topic_ref: Uuid,
    pub definition_ref: Uuid,
    pub classification_run_ref: Uuid,
    pub material_pack_ref: Uuid,
    pub definition_version: i32,
    pub imported_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicDefinition {
    pub definition_ref: Uuid,
    pub version: i32,
    pub display_name: String,
    pub definition_text: String,
    pub lifecycle_state: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicClassificationRun {
    pub classification_run_ref: Uuid,
    pub run_kind: String,
    pub run_state: String,
    pub adjudication_note: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMaterialPack {
    pub material_pack_ref: Uuid,
    pub source_boundary: String,
    pub frozen_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMaterialMember {
    pub work_public_ref: Uuid,
    pub role: TopicMaterialRole,
    pub rationale: String,
    pub ordinal: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicWorkspace {
    pub topic_ref: Uuid,
    pub domain_key: String,
    pub canonical_key: String,
    pub definition: TopicDefinition,
    pub classification_run: TopicClassificationRun,
    pub material_pack: TopicMaterialPack,
    pub members: Vec<TopicMaterialMember>,
}

/// The IDs minted for one immutable Definition/Run/Pack version travel together. Keeping
/// them as one private value prevents the receipt write from depending on a long positional
/// parameter list whose members must remain in lockstep.
struct WorkspaceVersionRefs {
    topic_ref: Uuid,
    definition_ref: Uuid,
    run_ref: Uuid,
    pack_ref: Uuid,
    version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TopicWorkspaceError {
    #[error("invalid Topic workspace request: {0}")]
    InvalidRequest(&'static str),
    #[error("unknown Work Resource {0}")]
    UnknownWorkResource(Uuid),
    #[error("idempotency key was already used for different content")]
    IdempotencyConflict,
    #[error("Topic definition version conflict: expected {expected:?}, actual {actual:?}")]
    VersionConflict {
        expected: Option<i32>,
        actual: Option<i32>,
    },
    #[error("Topic workspace projection is internally inconsistent")]
    CorruptProjection,
    #[error("Topic workspace database unavailable: {0}")]
    Database(String),
}

pub async fn topic_workspace_schema_is_ready(
    database: &Database,
) -> Result<bool, TopicWorkspaceError> {
    let ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_topic_workspace') IS NOT NULL \
         AND to_regclass('linggan_topic_import_receipt') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
    .map_err(database_error)?;
    Ok(ready)
}

pub async fn import_topic_workspace(
    database: &Database,
    request: &TopicWorkspaceImport,
) -> Result<TopicWorkspaceReceipt, TopicWorkspaceError> {
    validate_import(request)?;
    let request_sha256 = request_hash(request)?;
    let mut transaction = database.pool().begin().await.map_err(database_error)?;
    lock_import_scope(&mut transaction, request).await?;
    if let Some(receipt) = replay_receipt(&mut transaction, request, &request_sha256).await? {
        transaction.commit().await.map_err(database_error)?;
        return Ok(receipt);
    }
    ensure_materials_exist(&mut transaction, &request.members).await?;
    let (topic_ref, actual_version) = resolve_topic(&mut transaction, request).await?;
    if request.expected_version != actual_version {
        return Err(TopicWorkspaceError::VersionConflict {
            expected: request.expected_version,
            actual: actual_version,
        });
    }
    let version = actual_version.unwrap_or(0) + 1;
    let receipt = insert_workspace_version(
        &mut transaction,
        request,
        topic_ref,
        version,
        &request_sha256,
    )
    .await?;
    transaction.commit().await.map_err(database_error)?;
    Ok(receipt)
}

pub async fn read_topic_workspace(
    database: &Database,
    canonical_key: &str,
) -> Result<Option<TopicWorkspace>, TopicWorkspaceError> {
    if !valid_key(canonical_key, 96) {
        return Err(TopicWorkspaceError::InvalidRequest("invalid canonical key"));
    }
    let row = sqlx::query(
        "SELECT topic.topic_ref,topic.domain_key,topic.canonical_key,definition.definition_ref, \
         definition.version,definition.display_name,definition.definition_text, \
         definition.lifecycle_state,definition.created_at::text AS definition_created_at, \
         run.classification_run_ref,run.run_kind,run.run_state,run.adjudication_note, \
         run.completed_at::text AS completed_at,pack.material_pack_ref,pack.source_boundary, \
         pack.frozen_at::text AS frozen_at FROM linggan_topic_workspace topic \
         JOIN LATERAL (SELECT * FROM linggan_topic_definition candidate \
           WHERE candidate.topic_ref=topic.topic_ref ORDER BY version DESC LIMIT 1) definition ON true \
         JOIN linggan_topic_classification_run run USING(definition_ref) \
         JOIN linggan_topic_material_pack pack USING(classification_run_ref) \
         WHERE topic.canonical_key=$1",
    )
    .bind(canonical_key)
    .fetch_optional(database.pool())
    .await
    .map_err(database_error)?;
    let Some(row) = row else { return Ok(None) };
    let run_ref: Uuid = row.get("classification_run_ref");
    let members = read_members(database, run_ref).await?;
    Ok(Some(workspace_from_row(&row, members)))
}

fn validate_import(request: &TopicWorkspaceImport) -> Result<(), TopicWorkspaceError> {
    if !valid_idempotency_key(&request.idempotency_key) {
        return Err(TopicWorkspaceError::InvalidRequest(
            "invalid idempotency key",
        ));
    }
    if !valid_key(&request.domain_key, 64) || !valid_key(&request.canonical_key, 96) {
        return Err(TopicWorkspaceError::InvalidRequest("invalid Topic key"));
    }
    validate_text(&request.display_name, 120, "invalid display name")?;
    validate_text(&request.definition_text, 2000, "invalid definition")?;
    validate_text(
        &request.adjudication_note,
        2000,
        "invalid adjudication note",
    )?;
    validate_text(&request.source_boundary, 2000, "invalid source boundary")?;
    if request.members.len() < 2 || request.members.len() > MAX_MEMBERS {
        return Err(TopicWorkspaceError::InvalidRequest(
            "invalid material count",
        ));
    }
    validate_members(&request.members)
}

fn validate_members(members: &[TopicMaterialMemberImport]) -> Result<(), TopicWorkspaceError> {
    let mut refs = BTreeSet::new();
    let mut has_support = false;
    let mut has_counterweight = false;
    for member in members {
        if !refs.insert(member.work_public_ref) {
            return Err(TopicWorkspaceError::InvalidRequest(
                "duplicate Work Resource",
            ));
        }
        validate_text(&member.rationale, 1000, "invalid member rationale")?;
        has_support |= member.role == TopicMaterialRole::Support;
        has_counterweight |= matches!(
            member.role,
            TopicMaterialRole::Challenge | TopicMaterialRole::Boundary
        );
    }
    if !has_support || !has_counterweight {
        return Err(TopicWorkspaceError::InvalidRequest(
            "pack requires support and challenge or boundary",
        ));
    }
    Ok(())
}

fn validate_text(
    value: &str,
    maximum: usize,
    message: &'static str,
) -> Result<(), TopicWorkspaceError> {
    let length = value.trim().chars().count();
    if length == 0 || length > maximum {
        Err(TopicWorkspaceError::InvalidRequest(message))
    } else {
        Ok(())
    }
}

fn valid_key(value: &str, maximum: usize) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    value.len() >= 2
        && value.len() <= maximum
        && first.is_ascii_lowercase()
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
}

fn valid_idempotency_key(value: &str) -> bool {
    (8..=128).contains(&value.len())
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric()
                || (index > 0 && matches!(character, '.' | '_' | ':' | '-'))
        })
}

fn request_hash(request: &TopicWorkspaceImport) -> Result<String, TopicWorkspaceError> {
    let encoded = serde_json::to_vec(request)
        .map_err(|error| TopicWorkspaceError::Database(error.to_string()))?;
    let mut hash = String::with_capacity(64);
    for byte in Sha256::digest(encoded) {
        write!(&mut hash, "{byte:02x}")
            .map_err(|error| TopicWorkspaceError::Database(error.to_string()))?;
    }
    Ok(hash)
}

async fn lock_import_scope(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
) -> Result<(), TopicWorkspaceError> {
    // Both values are global unique identities in the database. Serializing only one leaks
    // the other unique constraint as a raw database error under concurrent requests. Sorting
    // gives every import the same lock order, so requests sharing both identities cannot deadlock.
    let mut identities = [
        format!("topic:canonical:{}", request.canonical_key),
        format!("topic:idempotency:{}", request.idempotency_key),
    ];
    identities.sort_unstable();
    for identity in identities {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(identity)
            .execute(&mut **transaction)
            .await
            .map_err(database_error)?;
    }
    Ok(())
}

async fn replay_receipt(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
    request_sha256: &str,
) -> Result<Option<TopicWorkspaceReceipt>, TopicWorkspaceError> {
    let row = sqlx::query(
        "SELECT receipt.receipt_ref,receipt.request_sha256,receipt.topic_ref, \
         receipt.definition_ref,receipt.classification_run_ref,receipt.material_pack_ref, \
         receipt.imported_at::text AS imported_at,definition.version \
         FROM linggan_topic_import_receipt receipt \
         JOIN linggan_topic_definition definition USING(definition_ref) WHERE idempotency_key=$1",
    )
    .bind(&request.idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    match row {
        None => Ok(None),
        Some(row) if row.get::<String, _>("request_sha256") == request_sha256 => {
            Ok(Some(receipt_from_row(&row)))
        }
        Some(_) => Err(TopicWorkspaceError::IdempotencyConflict),
    }
}

async fn ensure_materials_exist(
    transaction: &mut Transaction<'_, Postgres>,
    members: &[TopicMaterialMemberImport],
) -> Result<(), TopicWorkspaceError> {
    let requested: Vec<Uuid> = members
        .iter()
        .map(|member| member.work_public_ref)
        .collect();
    let admitted: Vec<Uuid> = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE public_ref = ANY($1)",
    )
    .bind(&requested)
    .fetch_all(&mut **transaction)
    .await
    .map_err(database_error)?;
    let admitted: BTreeSet<Uuid> = admitted.into_iter().collect();
    match requested
        .into_iter()
        .find(|reference| !admitted.contains(reference))
    {
        Some(reference) => Err(TopicWorkspaceError::UnknownWorkResource(reference)),
        None => Ok(()),
    }
}

async fn resolve_topic(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
) -> Result<(Uuid, Option<i32>), TopicWorkspaceError> {
    let row = sqlx::query(
        "SELECT topic.topic_ref,topic.domain_key,max(definition.version) AS version \
         FROM linggan_topic_workspace topic LEFT JOIN linggan_topic_definition definition \
         USING(topic_ref) WHERE topic.canonical_key=$1 GROUP BY topic.topic_ref,topic.domain_key",
    )
    .bind(&request.canonical_key)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    if let Some(row) = row {
        if row.get::<String, _>("domain_key") != request.domain_key {
            return Err(TopicWorkspaceError::InvalidRequest(
                "Topic domain cannot change",
            ));
        }
        return Ok((row.get("topic_ref"), row.get("version")));
    }
    let topic_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key) VALUES($1,$2,$3)",
    )
    .bind(topic_ref)
    .bind(&request.domain_key)
    .bind(&request.canonical_key)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok((topic_ref, None))
}

async fn insert_workspace_version(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
    topic_ref: Uuid,
    version: i32,
    request_sha256: &str,
) -> Result<TopicWorkspaceReceipt, TopicWorkspaceError> {
    let refs = WorkspaceVersionRefs {
        topic_ref,
        definition_ref: Uuid::new_v4(),
        run_ref: Uuid::new_v4(),
        pack_ref: Uuid::new_v4(),
        version,
    };
    insert_definition(transaction, request, &refs).await?;
    insert_run_and_pack(transaction, request, &refs).await?;
    insert_members(transaction, request, refs.run_ref).await?;
    insert_receipt(transaction, request, request_sha256, &refs).await
}

async fn insert_definition(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
    refs: &WorkspaceVersionRefs,
) -> Result<(), TopicWorkspaceError> {
    sqlx::query(
        "INSERT INTO linggan_topic_definition \
         (definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) \
         VALUES($1,$2,$3,$4,$5,'provisional')",
    )
    .bind(refs.definition_ref)
    .bind(refs.topic_ref)
    .bind(refs.version)
    .bind(request.display_name.trim())
    .bind(request.definition_text.trim())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn insert_run_and_pack(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
    refs: &WorkspaceVersionRefs,
) -> Result<(), TopicWorkspaceError> {
    sqlx::query(
        "INSERT INTO linggan_topic_classification_run \
         (classification_run_ref,definition_ref,run_kind,run_state,adjudication_note) \
         VALUES($1,$2,'human_adjudicated','completed',$3)",
    )
    .bind(refs.run_ref)
    .bind(refs.definition_ref)
    .bind(request.adjudication_note.trim())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    sqlx::query(
        "INSERT INTO linggan_topic_material_pack \
         (material_pack_ref,classification_run_ref,source_boundary) VALUES($1,$2,$3)",
    )
    .bind(refs.pack_ref)
    .bind(refs.run_ref)
    .bind(request.source_boundary.trim())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn insert_members(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
    run_ref: Uuid,
) -> Result<(), TopicWorkspaceError> {
    for (index, member) in request.members.iter().enumerate() {
        let ordinal = i32::try_from(index + 1).map_err(|_| {
            TopicWorkspaceError::InvalidRequest("material count exceeds integer range")
        })?;
        sqlx::query(
            "INSERT INTO linggan_topic_material_member \
             (classification_run_ref,work_public_ref,role,rationale,ordinal) \
             VALUES($1,$2,$3,$4,$5)",
        )
        .bind(run_ref)
        .bind(member.work_public_ref)
        .bind(member.role.as_str())
        .bind(member.rationale.trim())
        .bind(ordinal)
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    }
    Ok(())
}

async fn insert_receipt(
    transaction: &mut Transaction<'_, Postgres>,
    request: &TopicWorkspaceImport,
    request_sha256: &str,
    refs: &WorkspaceVersionRefs,
) -> Result<TopicWorkspaceReceipt, TopicWorkspaceError> {
    let row = sqlx::query(
        "INSERT INTO linggan_topic_import_receipt \
         (receipt_ref,idempotency_key,request_sha256,topic_ref,definition_ref, \
          classification_run_ref,material_pack_ref) VALUES($1,$2,$3,$4,$5,$6,$7) \
         RETURNING receipt_ref,topic_ref,definition_ref,classification_run_ref, \
         material_pack_ref,imported_at::text AS imported_at",
    )
    .bind(Uuid::new_v4())
    .bind(&request.idempotency_key)
    .bind(request_sha256)
    .bind(refs.topic_ref)
    .bind(refs.definition_ref)
    .bind(refs.run_ref)
    .bind(refs.pack_ref)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(TopicWorkspaceReceipt {
        receipt_ref: row.get("receipt_ref"),
        topic_ref: row.get("topic_ref"),
        definition_ref: row.get("definition_ref"),
        classification_run_ref: row.get("classification_run_ref"),
        material_pack_ref: row.get("material_pack_ref"),
        definition_version: refs.version,
        imported_at: row.get("imported_at"),
    })
}

async fn read_members(
    database: &Database,
    run_ref: Uuid,
) -> Result<Vec<TopicMaterialMember>, TopicWorkspaceError> {
    let rows = sqlx::query(
        "SELECT work_public_ref,role,rationale,ordinal FROM linggan_topic_material_member \
         WHERE classification_run_ref=$1 ORDER BY ordinal",
    )
    .bind(run_ref)
    .fetch_all(database.pool())
    .await
    .map_err(database_error)?;
    rows.iter()
        .map(|row| {
            Ok(TopicMaterialMember {
                work_public_ref: row.get("work_public_ref"),
                role: TopicMaterialRole::parse(&row.get::<String, _>("role"))?,
                rationale: row.get("rationale"),
                ordinal: row.get("ordinal"),
            })
        })
        .collect()
}

fn workspace_from_row(
    row: &sqlx::postgres::PgRow,
    members: Vec<TopicMaterialMember>,
) -> TopicWorkspace {
    TopicWorkspace {
        topic_ref: row.get("topic_ref"),
        domain_key: row.get("domain_key"),
        canonical_key: row.get("canonical_key"),
        definition: TopicDefinition {
            definition_ref: row.get("definition_ref"),
            version: row.get("version"),
            display_name: row.get("display_name"),
            definition_text: row.get("definition_text"),
            lifecycle_state: row.get("lifecycle_state"),
            created_at: row.get("definition_created_at"),
        },
        classification_run: TopicClassificationRun {
            classification_run_ref: row.get("classification_run_ref"),
            run_kind: row.get("run_kind"),
            run_state: row.get("run_state"),
            adjudication_note: row.get("adjudication_note"),
            completed_at: row.get("completed_at"),
        },
        material_pack: TopicMaterialPack {
            material_pack_ref: row.get("material_pack_ref"),
            source_boundary: row.get("source_boundary"),
            frozen_at: row.get("frozen_at"),
        },
        members,
    }
}

fn receipt_from_row(row: &sqlx::postgres::PgRow) -> TopicWorkspaceReceipt {
    TopicWorkspaceReceipt {
        receipt_ref: row.get("receipt_ref"),
        topic_ref: row.get("topic_ref"),
        definition_ref: row.get("definition_ref"),
        classification_run_ref: row.get("classification_run_ref"),
        material_pack_ref: row.get("material_pack_ref"),
        definition_version: row.get("version"),
        imported_at: row.get("imported_at"),
    }
}

fn database_error(error: sqlx::Error) -> TopicWorkspaceError {
    TopicWorkspaceError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request() -> TopicWorkspaceImport {
        TopicWorkspaceImport {
            idempotency_key: "topic:test:0001".to_owned(),
            domain_key: "adhd-family".to_owned(),
            canonical_key: "task-initiation-difficulty".to_owned(),
            display_name: "任务启动困难".to_owned(),
            definition_text: "一个可审查的暂定定义".to_owned(),
            expected_version: None,
            adjudication_note: "人工逐条裁定".to_owned(),
            source_boundary: "有限来源，不代表总体".to_owned(),
            members: vec![
                TopicMaterialMemberImport {
                    work_public_ref: Uuid::new_v4(),
                    role: TopicMaterialRole::Support,
                    rationale: "支持材料".to_owned(),
                },
                TopicMaterialMemberImport {
                    work_public_ref: Uuid::new_v4(),
                    role: TopicMaterialRole::Boundary,
                    rationale: "边界材料".to_owned(),
                },
            ],
        }
    }

    #[test]
    fn pack_requires_a_counterweight() {
        let mut request = base_request();
        request.members[1].role = TopicMaterialRole::Support;
        assert_eq!(
            validate_import(&request),
            Err(TopicWorkspaceError::InvalidRequest(
                "pack requires support and challenge or boundary"
            ))
        );
    }

    #[test]
    fn pack_cannot_repeat_a_work_resource() {
        let mut request = base_request();
        request.members[1].work_public_ref = request.members[0].work_public_ref;
        assert_eq!(
            validate_import(&request),
            Err(TopicWorkspaceError::InvalidRequest(
                "duplicate Work Resource"
            ))
        );
    }
}
