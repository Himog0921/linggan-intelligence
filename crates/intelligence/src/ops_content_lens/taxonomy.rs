use super::LensDefinitionError;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Deserialize)]
struct Entry {
    key: String,
    name: String,
    definition: String,
    include: String,
    confusable_with: String,
    example_ids: Vec<String>,
}

#[derive(Deserialize)]
struct Metadata {
    general_rules: Vec<String>,
}

#[derive(Deserialize)]
struct Taxonomy {
    #[serde(rename = "_meta")]
    meta: Metadata,
    situation: Vec<Entry>,
    moment: Vec<Entry>,
    offer: Vec<Entry>,
    audience: Vec<Entry>,
}

impl Taxonomy {
    fn validate(&self, first_version: bool) -> Result<(), LensDefinitionError> {
        use LensDefinitionError::InvalidTaxonomy;
        if self.meta.general_rules.is_empty()
            || self
                .meta
                .general_rules
                .iter()
                .any(|rule| rule.trim().is_empty())
        {
            return Err(InvalidTaxonomy("general_rules"));
        }
        for (name, entries, initial_keys) in [
            ("situation", &self.situation, SITUATION),
            ("moment", &self.moment, MOMENT),
            ("offer", &self.offer, OFFER),
            ("audience", &self.audience, AUDIENCE),
        ] {
            let mut keys = BTreeSet::new();
            for entry in entries {
                if entry.key.is_empty()
                    || !entry
                        .key
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                    || !keys.insert(entry.key.as_str())
                    || [
                        &entry.name,
                        &entry.definition,
                        &entry.include,
                        &entry.confusable_with,
                    ]
                    .iter()
                    .any(|v| v.trim().is_empty())
                    || entry.example_ids.iter().any(|id| id.trim().is_empty())
                {
                    return Err(InvalidTaxonomy(name));
                }
            }
            if !keys.contains("other")
                || !keys.contains("unclear")
                || ((first_version || name == "audience")
                    && keys != initial_keys.iter().copied().collect())
            {
                return Err(InvalidTaxonomy(name));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyReceipt {
    pub definition_ref: Uuid,
    pub version: i32,
}

pub async fn load_taxonomy(
    database: &Database,
    bytes: &[u8],
) -> Result<TaxonomyReceipt, LensDefinitionError> {
    let body: Value =
        serde_json::from_slice(bytes).map_err(|_| LensDefinitionError::InvalidTaxonomy("json"))?;
    let taxonomy: Taxonomy = serde_json::from_value(body.clone())
        .map_err(|_| LensDefinitionError::InvalidTaxonomy("structure"))?;
    let mut tx = database.pool().begin().await?;
    // Serialise taxonomy version assignment and pointer changes, including the first publication.
    sqlx::query("LOCK TABLE linggan_ops_lens_definition IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let last: Option<i32> = sqlx::query_scalar(
        "SELECT max(version) FROM linggan_ops_lens_definition WHERE kind='taxonomy'",
    )
    .fetch_one(&mut *tx)
    .await?;
    taxonomy.validate(last.is_none())?;
    let (definition_ref, version) = match sqlx::query_as::<_,(Uuid,i32)>(
        "SELECT definition_ref,version FROM linggan_ops_lens_definition WHERE kind='taxonomy' AND body=$1")
        .bind(&body).fetch_optional(&mut *tx).await? {
        Some(existing) => existing,
        None => sqlx::query_as("INSERT INTO linggan_ops_lens_definition (definition_ref,kind,version,body,created_by)
            VALUES (gen_random_uuid(),'taxonomy',$1,$2,'local_machine') RETURNING definition_ref,version")
            .bind(last.unwrap_or(0)+1).bind(&body).fetch_one(&mut *tx).await?,
    };
    sqlx::query("INSERT INTO linggan_ops_lens_definition_current AS current (kind,definition_ref,switched_by)
        VALUES ('taxonomy',$1,'local_machine') ON CONFLICT (kind) DO UPDATE SET definition_ref=EXCLUDED.definition_ref,
        switched_at=scope_001_now(), switched_by=EXCLUDED.switched_by WHERE current.definition_ref<>EXCLUDED.definition_ref")
        .bind(definition_ref).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(TaxonomyReceipt {
        definition_ref,
        version,
    })
}

const MOMENT: &[&str] = &[
    "parent_family_state",
    "whole_parenting_approach",
    "adhd_nature_prognosis",
    "peer_social",
    "discipline_communication",
    "schooling_teacher",
    "emotion_meltdown",
    "attention_execution",
    "self_esteem_identity",
    "homework_study",
    "holiday_break",
    "motivation_drive",
    "identification_treatment",
    "defiance_conduct",
    "screens_gaming",
    "other",
    "unclear",
];

const OFFER: &[&str] = &[
    "stance_advocacy",
    "mechanism_explainer",
    "vent_resonance",
    "experience_lesson",
    "actionable_method",
    "help_seeking_question",
    "dilemma_discussion",
    "urgency_warning",
    "serial_diary_log",
    "expert_digest",
    "model_family_practice",
    "checklist_tool_resource",
    "other",
    "unclear",
];

const SITUATION: &[&str] = &[
    "puzzled_or_misreading_child",
    "daily_management_struggle",
    "parent_depleted",
    "choosing_parenting_route",
    "proactive_skill_building",
    "long_break_at_home",
    "retrospective_turnaround",
    "accompany_home_intervention",
    "trajectory_anxiety",
    "post_diagnosis_orienting",
    "child_peer_struggle",
    "school_friction",
    "facing_stigma",
    "family_misaligned",
    "suspecting_identifying",
    "other",
    "unclear",
];

const AUDIENCE: &[&str] = &[
    "parent_adhd_child",
    "adult_adhd",
    "general_parenting",
    "other",
    "unclear",
];

#[cfg(test)]
pub(crate) mod tests;
