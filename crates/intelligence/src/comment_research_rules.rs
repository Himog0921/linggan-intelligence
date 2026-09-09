//! Immutable, inspectable comment-research rule revisions. Program safeguards live in code;
//! persisted revisions contain only the bounded research direction, field definitions and examples.
use crate::{comment_packet::CONTEXT_SELECTOR_VERSION, model_settings::ModelError};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const PROGRAM_CONTRACT_VERSION: &str = "comment-research.program.v1";
pub const V4_RULE_REVISION_REF: Uuid = Uuid::from_u128(0x6e9cc99c_5f57_4a22_9d91_7c4a085e5004);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuleVersion {
    #[serde(rename = "comment-research.v4")]
    V4,
    #[serde(rename = "comment-research.v5")]
    V5,
}

impl RuleVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V4 => "comment-research.v4",
            Self::V5 => "comment-research.v5",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomicKind {
    Problem,
    Need,
    Solution,
    Stance,
    Story,
    Quote,
    Emotion,
}

impl AtomicKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Problem => "problem",
            Self::Need => "need",
            Self::Solution => "solution",
            Self::Stance => "stance",
            Self::Story => "story",
            Self::Quote => "quote",
            Self::Emotion => "emotion",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExamplePolarity {
    Positive,
    Negative,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleCreationSource {
    SystemSeed,
    UserCandidate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulePurpose {
    pub title: String,
    pub instruction: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditableFieldDefinition {
    pub kind: AtomicKind,
    pub name: String,
    pub definition: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleExample {
    pub field: AtomicKind,
    pub polarity: ExamplePolarity,
    pub comment: String,
    pub expected: String,
}

/// The durable rule object. All program-owned versions are read from a fixed code map and
/// revalidated after every database read, so a JSON row cannot loosen evidence or identity rules.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleSnapshot {
    pub rule_revision_ref: Uuid,
    pub rule_version: RuleVersion,
    pub parent_rule_revision_ref: Option<Uuid>,
    pub purpose: RulePurpose,
    pub field_definitions: Vec<EditableFieldDefinition>,
    pub examples: Vec<RuleExample>,
    pub program_contract_version: String,
    pub schema_version: String,
    pub validator_version: String,
    pub selector_version: String,
    pub canonical_hash: String,
    pub creation_source: RuleCreationSource,
    pub created_at: String,
}

/// The command intentionally has no system/schema/validator/selector fields. Those controls are
/// fixed by [`program_contract`] and cannot be edited through a JSON payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRuleRevision {
    pub rule_revision_ref: Uuid,
    pub parent_rule_revision_ref: Uuid,
    pub purpose: RulePurpose,
    pub field_definitions: Vec<EditableFieldDefinition>,
    pub examples: Vec<RuleExample>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ActivateRuleRevision {
    pub expected_revision: i32,
    pub rule_revision_ref: Uuid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramContract {
    pub schema_version: &'static str,
    pub validator_version: &'static str,
    pub selector_version: &'static str,
}

pub const fn program_contract(rule_version: RuleVersion) -> ProgramContract {
    ProgramContract {
        schema_version: match rule_version {
            RuleVersion::V4 => "comment-extraction.schema.v4",
            RuleVersion::V5 => "comment-extraction.schema.v5",
        },
        validator_version: match rule_version {
            RuleVersion::V4 => "comment-packet-validator.v4",
            RuleVersion::V5 => "comment-packet-validator.v5",
        },
        selector_version: CONTEXT_SELECTOR_VERSION,
    }
}

pub fn builtin_v4() -> RuleSnapshot {
    let mut snapshot = RuleSnapshot {
        rule_revision_ref: V4_RULE_REVISION_REF,
        rule_version: RuleVersion::V4,
        parent_rule_revision_ref: None,
        purpose: RulePurpose {
            title: "逐条评论表达提取".into(),
            instruction: "只提取目标评论直接表达或经上下文消解的研究信号；不归并问题，不把作品内容当作评论者表达。".into(),
        },
        field_definitions: vec![
            field(AtomicKind::Need, "需求", "评论者明确提出的需求、期待或待解决事项。"),
            field(AtomicKind::Solution, "方案", "评论者自述已经采用、正在采用或建议的具体方案。"),
            field(AtomicKind::Story, "经历", "评论者描述的个人经历、场景或过程。"),
            field(AtomicKind::Quote, "典型表达", "保留能够代表评论者原意的短表达，不替代原文证据。"),
        ],
        examples: vec![
            example(AtomicKind::Need, ExamplePolarity::Positive, "我想知道第一步怎么做", "提取 need：希望获得开始步骤。"),
            example(AtomicKind::Solution, ExamplePolarity::Negative, "收藏了", "不把收藏动作当作 solution 或 need。"),
        ],
        program_contract_version: PROGRAM_CONTRACT_VERSION.into(),
        schema_version: program_contract(RuleVersion::V4).schema_version.into(),
        validator_version: program_contract(RuleVersion::V4).validator_version.into(),
        selector_version: program_contract(RuleVersion::V4).selector_version.into(),
        canonical_hash: String::new(),
        creation_source: RuleCreationSource::SystemSeed,
        created_at: "migration_seed".into(),
    };
    snapshot.canonical_hash = canonical_hash(&snapshot);
    snapshot
}

pub async fn read_active_rule(db: &Database) -> Result<RuleSnapshot, ModelError> {
    let row = sqlx::query("SELECT a.rule_revision_ref,r.rule_version,r.parent_rule_revision_ref,r.purpose,r.field_definitions,r.examples,r.program_contract_version,r.schema_version,r.validator_version,r.selector_version,r.canonical_hash,r.creation_source,r.created_at::text AS created_at FROM linggan_comment_research_rule_active a JOIN linggan_comment_research_rule_revision r USING(rule_revision_ref) WHERE a.singleton")
        .fetch_optional(db.pool())
        .await?
        .ok_or(ModelError::SchemaMissing)?;
    snapshot_from_row(&row)
}

/// Reads the effective pointer while retaining its row lock through the caller's transaction.
/// Batch/preflight/shadow manifest reservation must use this path so the selected revision and
/// its fingerprint are one atomic decision, rather than two pool reads separated by a switch.
pub(crate) async fn read_active_rule_in(
    connection: &mut PgConnection,
) -> Result<RuleSnapshot, ModelError> {
    let row = sqlx::query("SELECT a.rule_revision_ref,r.rule_version,r.parent_rule_revision_ref,r.purpose,r.field_definitions,r.examples,r.program_contract_version,r.schema_version,r.validator_version,r.selector_version,r.canonical_hash,r.creation_source,r.created_at::text AS created_at FROM linggan_comment_research_rule_active a JOIN linggan_comment_research_rule_revision r USING(rule_revision_ref) WHERE a.singleton FOR UPDATE OF a")
        .fetch_optional(connection)
        .await?
        .ok_or(ModelError::SchemaMissing)?;
    snapshot_from_row(&row)
}

pub async fn read_rule(db: &Database, rule_revision_ref: Uuid) -> Result<RuleSnapshot, ModelError> {
    let row = sqlx::query("SELECT rule_revision_ref,rule_version,parent_rule_revision_ref,purpose,field_definitions,examples,program_contract_version,schema_version,validator_version,selector_version,canonical_hash,creation_source,created_at::text AS created_at FROM linggan_comment_research_rule_revision WHERE rule_revision_ref=$1")
        .bind(rule_revision_ref)
        .fetch_optional(db.pool())
        .await?
        .ok_or(ModelError::NotFound)?;
    snapshot_from_row(&row)
}

pub async fn create_candidate(
    db: &Database,
    command: &CreateRuleRevision,
) -> Result<RuleSnapshot, ModelError> {
    let parent = read_rule(db, command.parent_rule_revision_ref).await?;
    let mut snapshot = candidate_snapshot(command, parent.rule_revision_ref);
    validate_rule_snapshot(&snapshot)?;
    snapshot.canonical_hash = canonical_hash(&snapshot);
    let result = sqlx::query("INSERT INTO linggan_comment_research_rule_revision(rule_revision_ref,rule_version,parent_rule_revision_ref,purpose,field_definitions,examples,program_contract_version,schema_version,validator_version,selector_version,canonical_hash,creation_source) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(rule_revision_ref) DO NOTHING")
        .bind(snapshot.rule_revision_ref)
        .bind(snapshot.rule_version.as_str())
        .bind(snapshot.parent_rule_revision_ref)
        .bind(json!(snapshot.purpose))
        .bind(json!(snapshot.field_definitions))
        .bind(json!(snapshot.examples))
        .bind(&snapshot.program_contract_version)
        .bind(&snapshot.schema_version)
        .bind(&snapshot.validator_version)
        .bind(&snapshot.selector_version)
        .bind(&snapshot.canonical_hash)
        .bind("user_candidate")
        .execute(db.pool())
        .await?;
    let persisted = read_rule(db, command.rule_revision_ref).await?;
    if result.rows_affected() == 0 && !same_immutable_content(&persisted, &snapshot) {
        return Err(ModelError::Conflict);
    }
    Ok(persisted)
}

pub(crate) async fn activate_rule_after_policy(
    db: &Database,
    command: &ActivateRuleRevision,
) -> Result<RuleSnapshot, ModelError> {
    let snapshot = read_rule(db, command.rule_revision_ref).await?;
    if snapshot.rule_version != RuleVersion::V5
        || snapshot.creation_source != RuleCreationSource::UserCandidate
    {
        return Err(ModelError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    activate_rule_after_policy_in(&mut tx, command).await?;
    tx.commit().await?;
    Ok(snapshot)
}

/// The shadow adopter holds its replay receipt and the active-rule pointer in one transaction.
/// Keep the candidate/version predicate here so no caller can move the pointer by reproducing a
/// looser update query.
pub(crate) async fn activate_rule_after_policy_in(
    connection: &mut PgConnection,
    command: &ActivateRuleRevision,
) -> Result<(), ModelError> {
    let changed = sqlx::query("UPDATE linggan_comment_research_rule_active active SET rule_revision_ref=$2,revision=revision+1,updated_at=scope_001_now() WHERE singleton AND revision=$1 AND EXISTS(SELECT 1 FROM linggan_comment_research_rule_revision candidate WHERE candidate.rule_revision_ref=$2 AND candidate.rule_version='comment-research.v5' AND candidate.creation_source='user_candidate' AND candidate.parent_rule_revision_ref=active.rule_revision_ref)")
        .bind(command.expected_revision)
        .bind(command.rule_revision_ref)
        .execute(connection)
        .await?;
    if changed.rows_affected() != 1 {
        return Err(ModelError::Conflict);
    }
    Ok(())
}

fn field(kind: AtomicKind, name: &str, definition: &str) -> EditableFieldDefinition {
    EditableFieldDefinition {
        kind,
        name: name.into(),
        definition: definition.into(),
    }
}

fn example(
    field: AtomicKind,
    polarity: ExamplePolarity,
    comment: &str,
    expected: &str,
) -> RuleExample {
    RuleExample {
        field,
        polarity,
        comment: comment.into(),
        expected: expected.into(),
    }
}

fn candidate_snapshot(command: &CreateRuleRevision, parent: Uuid) -> RuleSnapshot {
    let contract = program_contract(RuleVersion::V5);
    RuleSnapshot {
        rule_revision_ref: command.rule_revision_ref,
        rule_version: RuleVersion::V5,
        parent_rule_revision_ref: Some(parent),
        purpose: command.purpose.clone(),
        field_definitions: command.field_definitions.clone(),
        examples: command.examples.clone(),
        program_contract_version: PROGRAM_CONTRACT_VERSION.into(),
        schema_version: contract.schema_version.into(),
        validator_version: contract.validator_version.into(),
        selector_version: contract.selector_version.into(),
        canonical_hash: String::new(),
        creation_source: RuleCreationSource::UserCandidate,
        created_at: String::new(),
    }
}

fn snapshot_from_row(row: &sqlx::postgres::PgRow) -> Result<RuleSnapshot, ModelError> {
    let snapshot = RuleSnapshot {
        rule_revision_ref: row.get("rule_revision_ref"),
        rule_version: serde_json::from_value(json!(row.get::<String, _>("rule_version")))
            .map_err(|_| ModelError::Invalid)?,
        parent_rule_revision_ref: row.get("parent_rule_revision_ref"),
        purpose: serde_json::from_value(row.get("purpose")).map_err(|_| ModelError::Invalid)?,
        field_definitions: serde_json::from_value(row.get("field_definitions"))
            .map_err(|_| ModelError::Invalid)?,
        examples: serde_json::from_value(row.get("examples")).map_err(|_| ModelError::Invalid)?,
        program_contract_version: row.get("program_contract_version"),
        schema_version: row.get("schema_version"),
        validator_version: row.get("validator_version"),
        selector_version: row.get("selector_version"),
        canonical_hash: row.get("canonical_hash"),
        creation_source: serde_json::from_value(json!(row.get::<String, _>("creation_source")))
            .map_err(|_| ModelError::Invalid)?,
        created_at: row.get("created_at"),
    };
    validate_rule_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn same_immutable_content(actual: &RuleSnapshot, expected: &RuleSnapshot) -> bool {
    actual.rule_revision_ref == expected.rule_revision_ref
        && actual.rule_version == expected.rule_version
        && actual.parent_rule_revision_ref == expected.parent_rule_revision_ref
        && actual.purpose == expected.purpose
        && actual.field_definitions == expected.field_definitions
        && actual.examples == expected.examples
        && actual.program_contract_version == expected.program_contract_version
        && actual.schema_version == expected.schema_version
        && actual.validator_version == expected.validator_version
        && actual.selector_version == expected.selector_version
        && actual.canonical_hash == expected.canonical_hash
        && actual.creation_source == expected.creation_source
}

pub fn validate_rule_snapshot(snapshot: &RuleSnapshot) -> Result<(), ModelError> {
    let contract = program_contract(snapshot.rule_version);
    if snapshot.program_contract_version != PROGRAM_CONTRACT_VERSION
        || snapshot.schema_version != contract.schema_version
        || snapshot.validator_version != contract.validator_version
        || snapshot.selector_version != contract.selector_version
        || !valid_text(&snapshot.purpose.title, 100)
        || !valid_text(&snapshot.purpose.instruction, 2_000)
        || snapshot.field_definitions.is_empty()
        || snapshot.field_definitions.len() > 7
        || snapshot.examples.len() > 20
    {
        return Err(ModelError::Invalid);
    }
    let kinds: BTreeSet<_> = snapshot
        .field_definitions
        .iter()
        .map(|field| field.kind)
        .collect();
    if kinds.len() != snapshot.field_definitions.len()
        || snapshot
            .field_definitions
            .iter()
            .any(|field| !valid_text(&field.name, 100) || !valid_text(&field.definition, 1_000))
        || snapshot.examples.iter().any(|example| {
            !kinds.contains(&example.field)
                || !valid_text(&example.comment, 1_000)
                || !valid_text(&example.expected, 1_000)
        })
        || (snapshot.rule_version == RuleVersion::V4
            && kinds.iter().any(|kind| {
                !matches!(
                    kind,
                    AtomicKind::Need | AtomicKind::Solution | AtomicKind::Story | AtomicKind::Quote
                )
            }))
        || (snapshot.rule_version == RuleVersion::V4
            && snapshot.creation_source != RuleCreationSource::SystemSeed)
        || (snapshot.rule_version == RuleVersion::V5
            && snapshot.creation_source != RuleCreationSource::UserCandidate)
        || (!snapshot.canonical_hash.is_empty()
            && snapshot.canonical_hash != canonical_hash(snapshot))
    {
        return Err(ModelError::Invalid);
    }
    Ok(())
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}

pub fn canonical_hash(snapshot: &RuleSnapshot) -> String {
    let bytes = canonical_json(&canonical_payload(snapshot));
    Sha256::digest(bytes.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn canonical_payload(snapshot: &RuleSnapshot) -> Value {
    json!({
        "ruleVersion": snapshot.rule_version,
        "parentRuleRevisionRef": snapshot.parent_rule_revision_ref,
        "purpose": snapshot.purpose,
        "fieldDefinitions": snapshot.field_definitions,
        "examples": snapshot.examples,
        "programContractVersion": snapshot.program_contract_version,
        "schemaVersion": snapshot.schema_version,
        "validatorVersion": snapshot.validator_version,
        "selectorVersion": snapshot.selector_version,
        "creationSource": snapshot.creation_source,
    })
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).expect("JSON strings serialize"),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let pairs = values
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("JSON keys serialize"),
                        canonical_json(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{pairs}}}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_hash_is_stable_and_excludes_display_metadata() {
        let mut rule = builtin_v4();
        let hash = rule.canonical_hash.clone();
        rule.created_at = "later".into();
        rule.rule_revision_ref = Uuid::new_v4();
        assert_eq!(hash, canonical_hash(&rule));
        rule.purpose.instruction.push('。');
        assert_ne!(hash, canonical_hash(&rule));
    }

    #[test]
    fn candidate_command_cannot_supply_program_owned_rules() {
        let command = json!({"ruleRevisionRef":Uuid::new_v4(),"parentRuleRevisionRef":V4_RULE_REVISION_REF,"purpose":{"title":"方向","instruction":"只研究评论中的具体困难。"},"fieldDefinitions":[{"kind":"problem","name":"困难","definition":"评论者直接表达的困难。"}],"examples":[],"schemaVersion":"attacker"});
        assert!(serde_json::from_value::<CreateRuleRevision>(command).is_err());
    }
}
