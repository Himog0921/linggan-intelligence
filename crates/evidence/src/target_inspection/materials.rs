//! Narrow adapters to the adopted canonical dossier, never a second material store.
use super::{Error, Scope, scope};
use crate::{research_dossier, work_resource_current::read_work_resource_currents};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFragment {
    pub citation_id: String,
    pub source_ref: Uuid,
    pub source_version: String,
    pub field: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DossierView {
    pub work_ref: Uuid,
    pub knowledge_cutoff: String,
    pub material_revision: String,
    pub dossier_revision: String,
    pub owner_contract: String,
    pub fragments: Vec<SourceFragment>,
    pub partial: bool,
    pub limitations: Vec<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedWork {
    pub work_ref: Uuid,
    pub title: Option<String>,
    pub published_at: Option<String>,
    pub temporal_relation: &'static str,
    pub selection_basis: &'static str,
    pub matched_terms: usize,
    pub material_available: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedWorks {
    pub target_ref: Uuid,
    pub work_ref: Uuid,
    pub knowledge_cutoff: String,
    pub candidates: Vec<RelatedWork>,
    pub scanned_count: usize,
    pub truncated: bool,
    pub retrieval_basis: &'static str,
    pub method_version: &'static str,
}

pub async fn read_dossier(
    database: &Database,
    request: &Scope,
    work: Uuid,
) -> Result<DossierView, Error> {
    let (mut tx, cutoff) = scope::begin(database, request).await?;
    scope::require_work(&mut tx, request, work, &cutoff).await?;
    let result = read_in(&mut tx, &[work], &cutoff)
        .await?
        .remove(&work)
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    Ok(result)
}

pub async fn comparison_materials(
    database: &Database,
    request: &Scope,
    works: &[Uuid],
) -> Result<Vec<DossierView>, Error> {
    let (mut tx, cutoff) = scope::begin(database, request).await?;
    let result = comparison_materials_in(&mut tx, request, works, &cutoff).await?;
    tx.commit().await?;
    Ok(result)
}

/// Existing executor transactions can freeze and recheck this exact source scope atomically.
pub async fn comparison_materials_in(
    tx: &mut Transaction<'_, Postgres>,
    request: &Scope,
    works: &[Uuid],
    cutoff: &str,
) -> Result<Vec<DossierView>, Error> {
    if !(2..=3).contains(&works.len()) || works.iter().collect::<BTreeSet<_>>().len() != works.len()
    {
        return Err(Error::Invalid("select_two_or_three_distinct_works"));
    }
    scope::validate_target(tx, request).await?;
    for work in works {
        scope::require_work(tx, request, *work, cutoff).await?;
    }
    let mut source = read_in(tx, works, cutoff).await?;
    works
        .iter()
        .map(|work| source.remove(work).ok_or(Error::NotFound))
        .collect()
}

async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    cutoff: &str,
) -> Result<BTreeMap<Uuid, DossierView>, Error> {
    let dossiers = research_dossier::read_in(tx, works, cutoff).await?;
    Ok(dossiers
        .into_iter()
        .map(|(work, dossier)| {
            let mut limitations = coverage_limitations(&dossier.coverage, &dossier.media);
            let partial = !limitations.is_empty();
            limitations.push("collected_material_only".into());
            (
                work,
                DossierView {
                    work_ref: work,
                    knowledge_cutoff: cutoff.into(),
                    material_revision: dossier.material_revision,
                    dossier_revision: dossier.revision,
                    owner_contract: dossier.contract.into(),
                    partial,
                    limitations,
                    fragments: dossier
                        .fragments
                        .into_iter()
                        .map(|f| SourceFragment {
                            citation_id: f.fragment_id,
                            source_ref: f.source_ref,
                            source_version: f.source_version,
                            field: f.field,
                            start: f.start,
                            end: f.end,
                            text: f.text,
                        })
                        .collect(),
                },
            )
        })
        .collect())
}

// The processing-job count is not the image inventory. A known source slot
// without readable processing remains missing even when no job has been made.
fn coverage_limitations(coverage: &serde_json::Value, media: &[serde_json::Value]) -> Vec<String> {
    let mut reasons = Vec::new();
    if coverage["body"] != "KNOWN" {
        reasons.push("native_body_not_known".into());
    }
    if coverage["uncertainImages"].as_u64().is_some_and(|n| n > 0) {
        reasons.push("image_reading_uncertain".into());
    }
    if media.iter().any(|item| {
        matches!(
            item["processorKind"].as_str(),
            Some("image_ocr" | "video_frame_ocr" | "asr")
        ) && item["contextOnly"] != true
            && item["state"] != "ACQUIRED"
            && item["state"] != "KNOWN_EMPTY"
    }) {
        reasons.push("some_media_not_available".into());
    }
    let Some(slots) = coverage["mediaSlots"].as_array() else {
        reasons.push("media_coverage_unknown".into());
        return reasons;
    };
    let incomplete = slots.iter().any(|slot| {
        let Some(key) = slot["slotKey"].as_str() else {
            return true;
        };
        let Some(purpose) = slot["purpose"].as_str() else {
            return true;
        };
        media.iter().all(|item| {
            if item["slotKey"] != key || item["contextOnly"] == true {
                return true;
            }
            let processor = item["processorKind"].as_str();
            let matching = match purpose {
                "cover" | "body_image" => processor == Some("image_ocr"),
                "video" => processor == Some("asr"),
                _ => return true,
            };
            if !matching
                || matches!(
                    item["dispositionState"].as_str(),
                    Some("OCR_RETIRED" | "WITHDRAWN_OR_RESTRICTED")
                )
            {
                return true;
            }
            !(item["state"] == "KNOWN_EMPTY"
                || (item["state"] == "ACQUIRED"
                    && if purpose == "video" {
                        item["researchText"]
                            .as_str()
                            .is_some_and(|s| !s.trim().is_empty())
                    } else {
                        item["ocrLayering"]["state"] == "ACCEPTED"
                            && item["ocrLayering"]["cleanCorpusEligible"] == true
                    }))
        })
    });
    if incomplete {
        reasons.push("some_media_not_readable".into());
    }
    reasons
}

pub async fn related_works(
    database: &Database,
    request: &Scope,
    work: Uuid,
) -> Result<RelatedWorks, Error> {
    let (mut tx, cutoff) = scope::begin(database, request).await?;
    scope::require_work(&mut tx, request, work, &cutoff).await?;
    let creator: bool = sqlx::query_scalar(
        "SELECT target_kind='creator' FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(request.target_ref)
    .fetch_one(&mut *tx)
    .await?;
    if !creator {
        return Err(Error::Invalid("similar_old_works_requires_creator"));
    }
    let current = read_work_resource_currents(&mut tx, &[work], &cutoff)
        .await?
        .pop()
        .ok_or(Error::NotFound)?;
    let ids:Vec<Uuid>=sqlx::query_scalar(
        "SELECT DISTINCT author.content_public_ref FROM linggan_material_content_author author \
         JOIN collection_observation_target target ON target.identity_key=author.author_external_id \
           AND target.platform=author.platform AND target.target_kind='creator' \
         WHERE target.target_ref=$1 AND author.content_public_ref<>$3 \
           AND EXISTS(SELECT 1 FROM linggan_material_domain_usage usage \
             JOIN linggan_runtime_capture_package package USING(package_ref) \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             WHERE usage.content_public_ref=author.content_public_ref \
               AND ($2::uuid IS NULL OR usage.domain_ref=$2) \
               AND package.accepted_at<=$4::timestamptz AND receipt.material_admission='ACCEPTED') \
         ORDER BY author.content_public_ref LIMIT 2001",
    ).bind(request.target_ref).bind(request.domain_ref).bind(work).bind(&cutoff).fetch_all(&mut *tx).await?;
    let truncated = ids.len() > 2000;
    let scanned_count = ids.len().min(2000);
    let candidates = read_work_resource_currents(&mut tx, &ids[..scanned_count], &cutoff).await?;
    let current_text = read_in(&mut tx, &[work], &cutoff)
        .await?
        .remove(&work)
        .map(|d| {
            d.fragments
                .into_iter()
                .map(|f| f.text)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let tokens = terms(&current_text);
    let mut ranked = candidates
        .into_iter()
        .filter_map(|candidate| {
            if candidate.author_external_id.is_none()
                || candidate.author_external_id != current.author_external_id
            {
                return None;
            }
            let temporal = match (
                current.published_at_epoch_ms,
                candidate.published_at_epoch_ms,
            ) {
                (Some(now), Some(old)) if old < now => "older",
                (Some(_), Some(_)) => return None,
                _ => "unconfirmed",
            };
            let candidate_terms = terms(&format!(
                "{}\n{}",
                candidate.title.as_deref().unwrap_or(""),
                candidate.body_text.as_deref().unwrap_or("")
            ));
            let matched = tokens.intersection(&candidate_terms).count();
            if matched == 0 {
                return None;
            }
            let score = matched as f64 / tokens.union(&candidate_terms).count().max(1) as f64;
            Some((
                score,
                RelatedWork {
                    work_ref: candidate.public_ref,
                    title: candidate.title,
                    published_at: candidate.published_at,
                    temporal_relation: temporal,
                    selection_basis: "lexical_unverified",
                    matched_terms: matched,
                    material_available: candidate.detail_material_ref.is_some(),
                },
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.work_ref.cmp(&b.1.work_ref)));
    let candidates = ranked
        .into_iter()
        .take(20)
        .map(|(_, candidate)| candidate)
        .collect();
    tx.commit().await?;
    Ok(RelatedWorks {
        target_ref: request.target_ref,
        work_ref: work,
        knowledge_cutoff: cutoff,
        candidates,
        scanned_count,
        truncated,
        retrieval_basis: "native_text_lexical_candidates",
        method_version: "target-related-lexical.v1",
    })
}
fn terms(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut previous = None;
    for value in text.chars().flat_map(char::to_lowercase) {
        if value.is_alphanumeric() {
            if let Some(before) = previous {
                result.insert(format!("{before}{value}"));
            }
            previous = Some(value);
        } else {
            previous = None;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::coverage_limitations;
    use serde_json::json;

    #[test]
    fn known_unprocessed_images_and_unknown_inventory_are_partial() {
        let coverage = json!({"body":"KNOWN","mediaCount":0,"mediaAvailable":0,
            "uncertainImages":0,"mediaSlots":[{"slotKey":"image-1","purpose":"body_image"}]});
        assert_eq!(
            coverage_limitations(&coverage, &[]),
            vec!["some_media_not_readable"]
        );
        assert_eq!(
            coverage_limitations(&json!({"body":"KNOWN"}), &[]),
            vec!["media_coverage_unknown"]
        );
        assert!(coverage_limitations(&json!({"body":"KNOWN","mediaSlots":[]}), &[]).is_empty());
    }

    #[test]
    fn every_known_image_requires_qualified_reading_or_known_empty() {
        let coverage = json!({"body":"KNOWN","mediaSlots":[
            {"slotKey":"cover","purpose":"cover"},{"slotKey":"page-2","purpose":"body_image"}]});
        let cover = json!({"slotKey":"cover","processorKind":"image_ocr","state":"ACQUIRED",
            "ocrLayering":{"state":"ACCEPTED","cleanCorpusEligible":true}});
        let empty = json!({"slotKey":"page-2","processorKind":"image_ocr","state":"KNOWN_EMPTY"});
        assert!(coverage_limitations(&coverage, &[cover.clone(), empty.clone()]).is_empty());
        assert!(!coverage_limitations(&coverage, &[cover.clone()]).is_empty());
        for state in ["QUEUED", "WITHDRAWN_OR_RESTRICTED", "RETIRED"] {
            let mut unread = empty.clone();
            unread["state"] = json!(state);
            assert!(!coverage_limitations(&coverage, &[cover.clone(), unread]).is_empty());
        }
        let mut uncertain = cover.clone();
        uncertain["ocrLayering"]["state"] = json!("PARTIAL");
        assert!(!coverage_limitations(&coverage, &[uncertain, empty]).is_empty());
    }
}
