//! Fixed offline Pi loader invocation. The subprocess receives upload bytes, never secrets.
use crate::{
    ai_foundation::skill_package::{PreparedPackage, SkillInspection},
    model_settings::ModelError,
    pi_adapter::PiAdapter,
    pi_offline::OfflineHelper,
};
impl PiAdapter {
    pub async fn inspect_skill_package(
        &self,
        package: &PreparedPackage,
    ) -> Result<SkillInspection, ModelError> {
        let request = serde_json::json!({"contract":"ai-skill-package.v1","directory":package.directory,
            "files":package.files.iter().filter_map(|f|f.text.as_ref().map(|text|serde_json::json!({"path":f.path,"content":text}))).collect::<Vec<_>>()});
        let result: SkillInspection = self.call_offline(OfflineHelper::Skill, &request).await?;
        if result.contract != "ai-skill-inspection.v1"
            || result.name != package.directory
            || result.description.trim().is_empty()
            || result.description.encode_utf16().count() > 1024
        {
            return Err(ModelError::InvalidOutput);
        }
        Ok(result)
    }
}
