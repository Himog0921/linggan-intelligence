//! Per-stage version switching through the same public Settings commands.
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        prompt::{self, ActivatePrompt, SavePrompt},
        skill_binding::{SwitchSkill, switch_skill},
        skill_package::{Encoding, SkillPackage, UploadFile},
        skill_store::{ImportSkill, import_skill},
    },
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;
pub async fn assign(
    db: &Database,
    adapter: &PiAdapter,
    task: TaskKey,
    version: &str,
    previous: Option<Uuid>,
) -> Uuid {
    let directory = format!("method-{}", task.as_str().replace('_', "-"));
    let skill=import_skill(db,adapter,&ImportSkill{command_ref:Uuid::new_v4(),expected_version_ref:previous,package:SkillPackage{directory:directory.clone(),files:vec![UploadFile{path:"SKILL.md".into(),encoding:Encoding::Utf8,content:format!("---\nname: {directory}\ndescription: Synthetic phase proof\n---\nSKILL {version} {}\n",task.as_str())}]}}).await.unwrap();
    let current = prompt::read(db, task).await.unwrap();
    let revision = current["revision"].as_i64().unwrap();
    let saved = prompt::save(
        db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: task,
            expected_task_revision: revision,
            system_text: format!("METHOD {version} {}", task.as_str()),
            instruction_text: format!("INSTRUCTION {version} {}", task.as_str()),
        },
    )
    .await
    .unwrap();
    let enabled = prompt::activate(
        db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: task,
            expected_task_revision: revision,
            version_ref: saved.version_ref,
        },
    )
    .await
    .unwrap();
    let binding: i64 =
        sqlx::query_scalar("SELECT revision FROM linggan_skill_binding WHERE task_key=$1")
            .bind(task.as_str())
            .fetch_one(db.pool())
            .await
            .unwrap();
    switch_skill(
        db,
        &SwitchSkill {
            command_ref: Uuid::new_v4(),
            task_key: task,
            expected_task_revision: enabled.task_revision,
            expected_binding_revision: binding,
            version_ref: skill.version_ref,
        },
    )
    .await
    .unwrap();
    skill.version_ref
}
