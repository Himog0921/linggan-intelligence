//! One target comment as a research unit reads it.
//!
//! The source gate is the V1 study gate with two differences that the plan fixes: no Research
//! Dossier context is read (L1 is the work context), and a commenter whose author id is unknown is
//! allowed. Such a comment can support a Problem but never adds to its author count. A comment by
//! the work's own author stays excluded, and so does one whose work author is unknown, because
//! the creator's voice could not be told apart.
use crate::{
    ai_foundation::store::FoundationError,
    comment_cleaning::{CLEANER_VERSION, CleanComment, clean},
    comment_study_source::{
        StudySourceRole,
        context::{read_parents, read_titles},
        gate,
    },
    research_text::content_hash,
};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const TARGET_CONTRACT: &str = "comment-understanding.target.v1";

/// The parent comment, context only: it can explain a pronoun but is never evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentContext {
    None,
    Available { research_text: String },
    Unavailable,
}

impl ParentContext {
    /// The model-facing form.
    pub fn payload(&self) -> Value {
        match self {
            Self::None => json!({"state":"none"}),
            Self::Available { research_text } => {
                json!({"state":"available","text":research_text})
            }
            Self::Unavailable => json!({"state":"unavailable"}),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Target {
    pub source_ref: Uuid,
    pub work_ref: Uuid,
    pub comment_external_id: String,
    pub thread_key: String,
    pub author_external_id: Option<String>,
    pub raw_text: String,
    pub cleaned: CleanComment,
    pub parent: ParentContext,
}

impl Target {
    /// Identity of exactly what was frozen for this request.
    pub fn input_hash(&self) -> String {
        content_hash(
            &json!({
                "contract": TARGET_CONTRACT,
                "commentKey": {"workRef": self.work_ref, "commentExternalId": self.comment_external_id},
                "sourceRef": self.source_ref,
                "rawSha256": content_hash(&self.raw_text),
                "cleanerVersion": CLEANER_VERSION,
                "parent": self.parent_identity(),
            })
            .to_string(),
        )
    }

    /// What decides whether two researches read the same thing: the text, how it was cleaned, the
    /// parent that was used and the L1 it was read against. It is a source record, not a validity
    /// condition: a later L1 or cleaner version never invalidates a stored result.
    pub fn fingerprint(&self, l1_request_ref: Uuid) -> String {
        content_hash(
            &json!({
                "contract": TARGET_CONTRACT,
                "commentKey": {"workRef": self.work_ref, "commentExternalId": self.comment_external_id},
                "researchSha256": content_hash(&self.cleaned.text),
                "cleanerVersion": CLEANER_VERSION,
                "parent": self.parent_identity(),
                "l1RequestRef": l1_request_ref,
                "method": super::CONTRACT,
            })
            .to_string(),
        )
    }

    fn parent_identity(&self) -> Value {
        match &self.parent {
            ParentContext::None => json!({"state":"none"}),
            ParentContext::Available { research_text } => {
                json!({"state":"available","researchSha256":content_hash(research_text)})
            }
            ParentContext::Unavailable => json!({"state":"unavailable"}),
        }
    }
}

const SOURCE_SQL: &str = "SELECT c.material_ref,c.content_public_ref,c.comment_external_id,\
 c.root_comment_external_id,c.parent_comment_external_id,c.body_state,c.body_text,\
 c.author_external_id,author.author_external_id AS work_author_external_id,\
 EXISTS(SELECT 1 FROM linggan_material_comment_restriction r \
  WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id) AS source_restricted \
 FROM linggan_material_comment c JOIN linggan_runtime_capture_package p USING(package_ref) \
 LEFT JOIN LATERAL linggan_material_content_author_at(c.content_public_ref,$3::timestamptz) author ON true \
 WHERE c.material_ref=$1 AND c.content_public_ref=$2 AND p.accepted_at<=$3::timestamptz \
 AND c.created_at<=$3::timestamptz AND linggan_opportunity_observed_at(c.observed_at) IS NOT NULL \
 AND EXISTS(SELECT 1 FROM linggan_material_domain_usage u WHERE u.content_public_ref=c.content_public_ref \
  AND u.domain_ref=$4 AND u.role=$5) \
 AND c.material_ref=(SELECT newer.material_ref FROM linggan_material_comment newer \
  JOIN linggan_runtime_capture_package np USING(package_ref) \
  WHERE newer.content_public_ref=c.content_public_ref AND newer.comment_external_id=c.comment_external_id \
  AND np.accepted_at<=$3::timestamptz AND newer.created_at<=$3::timestamptz \
  AND linggan_opportunity_observed_at(newer.observed_at) IS NOT NULL \
  ORDER BY linggan_opportunity_observed_at(newer.observed_at) DESC,newer.created_at DESC,newer.material_ref DESC LIMIT 1)";

/// `Ok(Err(code))` is a comment that may not be researched, with the gate's reason; the code is
/// one of the source gate's exclusion codes or `source_unavailable`.
pub async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    source_ref: Uuid,
    role: StudySourceRole,
    as_of: &str,
) -> Result<Result<Target, &'static str>, sqlx::Error> {
    let row = sqlx::query(SOURCE_SQL)
        .bind(source_ref)
        .bind(work)
        .bind(as_of)
        .bind(domain)
        .bind(role.as_str())
        .fetch_optional(&mut **tx)
        .await?;
    let Some(row) = row else {
        return Ok(Err("source_unavailable"));
    };
    let raw: Option<String> = row.try_get("body_text")?;
    let known = |value: Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let author = known(row.try_get("author_external_id")?);
    let work_author = known(row.try_get("work_author_external_id")?);
    let cleaned = clean(raw.as_deref().unwrap_or(""));
    let excluded = gate::exclusion([
        row.try_get("source_restricted")?,
        row.try_get::<String, _>("body_state")? != "KNOWN" || raw.is_none(),
        false,
        !matches!(cleaned.state.as_str(), "direct" | "context"),
        false, // A missing work author does not establish creator voice.
        false,
        author.is_some() && author == work_author,
    ]);
    if let Some(code) = excluded {
        return Ok(Err(code));
    }
    let comment_external_id: String = row.try_get("comment_external_id")?;
    let parent = match row.try_get::<Option<String>, _>("parent_comment_external_id")? {
        None => ParentContext::None,
        Some(id) => {
            let mut parents = read_parents(tx, &[(work, id.clone())], as_of).await?;
            match parents
                .remove(&(work, id))
                .filter(|parent| parent["sourceState"] == "known")
                .and_then(|parent| parent["researchText"].as_str().map(str::to_owned))
            {
                Some(research_text) => ParentContext::Available { research_text },
                None => ParentContext::Unavailable,
            }
        }
    };
    Ok(Ok(Target {
        source_ref,
        work_ref: work,
        thread_key: row.try_get("root_comment_external_id")?,
        comment_external_id,
        author_external_id: author,
        raw_text: raw.unwrap_or_default(),
        cleaned,
        parent,
    }))
}

/// What a stored research needs from its comment to feed a downstream reader, whichever method
/// produced it.
#[derive(Debug, Clone)]
pub struct CurrentSource {
    pub source_ref: Uuid,
    pub raw_text: String,
    pub raw_sha256: String,
    pub comment_external_id: String,
    pub work_ref: Uuid,
    pub work_title: String,
    pub input_hash: String,
}

/// A V2 research may feed a reader only while its comment can still be researched. Nothing else
/// is compared: a later L1 or cleaner version never invalidates what was stored.
pub async fn current_source_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<CurrentSource, FoundationError> {
    let row = sqlx::query("SELECT domain_ref,work_ref,source_ref,observation_role,input_hash,to_char(knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff FROM linggan_comment_research_request WHERE request_ref=$1 AND method_version=$2")
        .bind(request)
        .bind(super::CONTRACT)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(FoundationError::NotFound)?;
    let role = match row.try_get::<String, _>("observation_role")?.as_str() {
        "primary" => StudySourceRole::Primary,
        _ => StudySourceRole::Reference,
    };
    let (domain, work): (Uuid, Uuid) = (row.try_get("domain_ref")?, row.try_get("work_ref")?);
    let cutoff: String = row.try_get("cutoff")?;
    let target = read_in(tx, domain, work, row.try_get("source_ref")?, role, &cutoff)
        .await?
        .map_err(|_| FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]))?;
    let title = read_titles(tx, &[work], &cutoff)
        .await?
        .remove(&work)
        .and_then(|title| title["displayTitle"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "未命名作品".to_owned());
    Ok(CurrentSource {
        source_ref: target.source_ref,
        raw_sha256: content_hash(&target.raw_text),
        raw_text: target.raw_text,
        comment_external_id: target.comment_external_id,
        work_ref: work,
        work_title: title,
        input_hash: row.try_get("input_hash")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(raw: &str, parent: ParentContext) -> Target {
        Target {
            source_ref: Uuid::from_u128(1),
            work_ref: Uuid::from_u128(2),
            comment_external_id: "c1".into(),
            thread_key: "c1".into(),
            author_external_id: Some("A".into()),
            raw_text: raw.into(),
            cleaned: clean(raw),
            parent,
        }
    }

    #[test]
    fn fingerprint_follows_text_parent_and_l1_but_not_the_observation_row() {
        let base = target("孩子写作业要催", ParentContext::None);
        let l1 = Uuid::from_u128(9);
        assert_eq!(base.fingerprint(l1), base.fingerprint(l1));
        let mut other_observation = base.clone();
        other_observation.source_ref = Uuid::from_u128(77);
        assert_eq!(base.fingerprint(l1), other_observation.fingerprint(l1));
        assert_ne!(base.input_hash(), other_observation.input_hash());
        assert_ne!(base.fingerprint(l1), base.fingerprint(Uuid::from_u128(10)));
        let with_parent = target(
            "孩子写作业要催",
            ParentContext::Available {
                research_text: "怎么办".into(),
            },
        );
        assert_ne!(base.fingerprint(l1), with_parent.fingerprint(l1));
        assert_ne!(
            with_parent.fingerprint(l1),
            target("孩子写作业要催", ParentContext::Unavailable).fingerprint(l1)
        );
    }

    #[test]
    fn the_parent_reaches_the_model_only_as_context() {
        assert_eq!(ParentContext::None.payload(), json!({"state":"none"}));
        assert_eq!(
            ParentContext::Unavailable.payload(),
            json!({"state":"unavailable"})
        );
        let parent = ParentContext::Available {
            research_text: "怎么办".into(),
        };
        assert_eq!(
            parent.payload(),
            json!({"state":"available","text":"怎么办"})
        );
    }
}
