//! Import and immutable version identity. Trial/binding admission is a separate command.
use super::{
    skill_package::SkillPackage,
    store::{Command, FoundationError},
};
use crate::{model_settings::ModelError, pi_adapter::PiAdapter};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportSkill {
    pub command_ref: Uuid,
    pub expected_version_ref: Option<Uuid>,
    pub package: SkillPackage,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportReceipt {
    pub skill_ref: Uuid,
    pub version_ref: Uuid,
    pub version_no: i32,
    pub package_sha256: String,
    pub reused: bool,
}
pub async fn import_skill(
    db: &Database,
    adapter: &PiAdapter,
    request: &ImportSkill,
) -> Result<ImportReceipt, FoundationError> {
    let package = request.package.prepare()?;
    // The parser is offline and completes before acquiring database locks.
    let inspection = adapter
        .inspect_skill_package(&package)
        .await
        .map_err(|e| match e {
            ModelError::AdapterUnavailable | ModelError::Timeout => {
                FoundationError::LoaderUnavailable
            }
            _ => FoundationError::Invalid,
        })?;
    let payload = json!({"directory":package.directory,"packageHash":package.package_sha256,
        "sourceHash":package.source_sha256,"expectedVersionRef":request.expected_version_ref});
    let mut command = Command::begin(db, request.command_ref, "skill_import", &payload).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,410133))")
        .bind(&package.directory)
        .execute(&mut *command.tx)
        .await?;
    let skill: Option<Uuid> =
        sqlx::query_scalar("SELECT skill_ref FROM linggan_skill WHERE name=$1")
            .bind(&package.directory)
            .fetch_optional(&mut *command.tx)
            .await?;
    let skill_ref = match skill {
        Some(id) => id,
        None => {
            if request.expected_version_ref.is_some() {
                return Err(FoundationError::Conflict);
            }
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO linggan_skill(skill_ref,name) VALUES($1,$2)")
                .bind(id)
                .bind(&package.directory)
                .execute(&mut *command.tx)
                .await?;
            id
        }
    };
    let current=sqlx::query("SELECT version_ref,version_no,package_sha256 FROM linggan_skill_version WHERE skill_ref=$1 ORDER BY version_no DESC LIMIT 1")
        .bind(skill_ref).fetch_optional(&mut *command.tx).await?;
    if current.as_ref().map(|r| r.get::<Uuid, _>("version_ref")) != request.expected_version_ref {
        return Err(FoundationError::Conflict);
    }
    let same=sqlx::query("SELECT version_ref,version_no FROM linggan_skill_version WHERE skill_ref=$1 AND package_sha256=$2")
        .bind(skill_ref).bind(&package.package_sha256).fetch_optional(&mut *command.tx).await?;
    let receipt = if let Some(row) = same {
        ImportReceipt {
            skill_ref,
            version_ref: row.get("version_ref"),
            version_no: row.get("version_no"),
            package_sha256: package.package_sha256.clone(),
            reused: true,
        }
    } else {
        let version_no = current.as_ref().map_or(Ok(1), |r| {
            r.get::<i32, _>("version_no")
                .checked_add(1)
                .ok_or(FoundationError::Invalid)
        })?;
        let version_ref = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_skill_version(version_ref,skill_ref,directory_name,version_no,description,body,frontmatter,disable_model_invocation,invocation,loader_version,normalization_version,package_sha256,source_sha256) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'pi-agent-core/0.85.1','ai-skill-package.v1',$10,$11)")
            .bind(version_ref).bind(skill_ref).bind(&package.directory).bind(version_no).bind(&inspection.description)
            .bind(&inspection.body).bind(&inspection.frontmatter).bind(inspection.disable_model_invocation).bind(&inspection.invocation)
            .bind(&package.package_sha256).bind(&package.source_sha256).execute(&mut *command.tx).await?;
        for file in &package.files {
            sqlx::query("INSERT INTO linggan_skill_file(version_ref,path,bytes,text_content,sha256) VALUES($1,$2,$3,$4,$5)")
                .bind(version_ref).bind(&file.path).bind(&file.bytes).bind(&file.text).bind(&file.sha256).execute(&mut *command.tx).await?;
        }
        ImportReceipt {
            skill_ref,
            version_ref,
            version_no,
            package_sha256: package.package_sha256.clone(),
            reused: false,
        }
    };
    sqlx::query("INSERT INTO linggan_skill_import(command_ref,version_ref,source_sha256,source_manifest) VALUES($1,$2,$3,$4)")
        .bind(request.command_ref).bind(receipt.version_ref).bind(&package.source_sha256)
        .bind(json!(package.source_manifest))
        .execute(&mut *command.tx).await?;
    command.finish(receipt).await
}
