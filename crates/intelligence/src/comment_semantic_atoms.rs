//! Typed, versioned semantic atoms are derived only from accepted comment results.
//! History is immutable. The current view selects one applicable analysis, never a union of
//! repeated analyses, and applies live source/context qualification before any research use.
use crate::{comment_research::comment_source_hash, model_settings::ModelError};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct AtomValue {
    kind: String,
    ordinal: i32,
    meaning: String,
    context: Option<String>,
    target: Option<String>,
    position: Option<String>,
    evidence: Value,
    context_evidence: Value,
}
fn evidence_valid(evidence: &Value, source: Uuid, body: &str) -> bool {
    evidence.as_array().is_some_and(|items| {
        !items.is_empty()
            && items.len() <= 8
            && items.iter().all(|e| {
                e["sourceRef"]
                    .as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    == Some(source)
                    && e["startChar"]
                        .as_u64()
                        .zip(e["endChar"].as_u64())
                        .is_some_and(|(start, end)| {
                            start < end && end <= body.chars().count() as u64
                        })
            })
    })
}
fn accepted_atoms(result: &Value, source: Uuid, body: &str) -> Vec<AtomValue> {
    if result["semantic"]["outcome"] != "interpretable" {
        return vec![];
    }
    let mut atoms = Vec::new();
    if let Some(values) = result["semantic"]["atoms"].as_array() {
        for value in values {
            let Some(kind) = value["kind"].as_str().filter(|kind| {
                matches!(
                    *kind,
                    "problem" | "need" | "solution" | "stance" | "story" | "quote" | "emotion"
                )
            }) else {
                continue;
            };
            let Some(meaning) = value["meaning"]
                .as_str()
                .filter(|text| !text.trim().is_empty() && text.chars().count() <= 1000)
            else {
                continue;
            };
            let Some(ordinal) = value["ordinal"].as_i64().filter(|n| (1..=100).contains(n)) else {
                continue;
            };
            if !matches!(
                value["basis"].as_str(),
                Some("explicit" | "context_resolved")
            ) || !evidence_valid(&value["evidence"], source, body)
            {
                continue;
            }
            let target = value["target"].as_str().map(str::to_owned);
            let position = value["position"].as_str().map(str::to_owned);
            if kind == "stance"
                && (target.as_ref().is_none_or(|s| s.is_empty())
                    || !position.as_deref().is_some_and(|p| {
                        matches!(p, "support" | "oppose" | "concern" | "mixed" | "neutral")
                    }))
            {
                continue;
            }
            atoms.push(AtomValue {
                target,
                position,
                kind: kind.into(),
                ordinal: ordinal as i32,
                meaning: meaning.into(),
                context: value["context"].as_str().map(str::to_owned),
                evidence: value["evidence"].clone(),
                context_evidence: value["contextEvidence"].clone(),
            });
        }
    } else {
        atoms = legacy_atoms(result, source, body);
    }
    atoms
}

fn legacy_atoms(result: &Value, source: Uuid, body: &str) -> Vec<AtomValue> {
    let mut atoms = Vec::new();
    // v4 adaptation copies only already accepted meaning or exact source spans. It does
    // not claim that a coarse old label is a newly extracted need/solution description.
    for (collection, kind) in [
        ("problems", "problem"),
        ("stances", "stance"),
        ("labels", ""),
    ] {
        for (index, value) in result["semantic"][collection]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let kind = if kind.is_empty() {
                value["label"].as_str().unwrap_or("")
            } else {
                kind
            };
            if !matches!(
                kind,
                "problem" | "need" | "solution" | "stance" | "story" | "quote"
            ) || !evidence_valid(&value["evidence"], source, body)
            {
                continue;
            }
            let meaning = if kind == "problem" {
                value["meaning"].as_str().map(str::to_owned)
            } else if kind == "stance" {
                value["target"]
                    .as_str()
                    .zip(value["position"].as_str())
                    .map(|(t, p)| format!("{t}：{p}"))
            } else {
                Some(
                    value["evidence"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|e| {
                            body.chars()
                                .skip(e["startChar"].as_u64().unwrap() as usize)
                                .take(
                                    (e["endChar"].as_u64().unwrap()
                                        - e["startChar"].as_u64().unwrap())
                                        as usize,
                                )
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join("；"),
                )
            };
            let Some(meaning) =
                meaning.filter(|s| !s.trim().is_empty() && s.chars().count() <= 1000)
            else {
                continue;
            };
            let target = if kind == "stance" {
                value["target"].as_str().map(str::to_owned)
            } else {
                None
            };
            let position = if kind == "stance" {
                value["position"].as_str().map(str::to_owned)
            } else {
                None
            };
            atoms.push(AtomValue {
                target,
                position,
                kind: kind.into(),
                ordinal: (index + 1) as i32,
                meaning,
                context: None,
                evidence: value["evidence"].clone(),
                context_evidence: value["contextEvidence"]
                    .as_array()
                    .map_or(json!([]), |v| json!(v)),
            });
        }
    }

    atoms
}

pub(crate) async fn refresh(db: &Database, limit: i64) -> Result<usize, ModelError> {
    let rows=sqlx::query(r#"
      SELECT a.work_ref AS analysis_ref,a.source_ref,a.result,a.rule_version,m.body_text,s.canonical_ref,s.domain_ref
      FROM linggan_comment_analysis_work a JOIN linggan_material_comment m ON m.material_ref=a.source_ref
      JOIN linggan_ci_source s ON s.work_ref=m.content_public_ref AND s.comment_external_id=m.comment_external_id
      JOIN linggan_comment_research_eligibility_current e ON e.current_analysis_ref=a.work_ref AND e.source_ref=s.source_ref
      WHERE s.domain_ref=(SELECT domain_ref FROM observation_domain WHERE is_own_domain) AND a.state IN('succeeded','no_signal') AND a.result IS NOT NULL AND linggan_ci_analysis_context_readable(a.result)
        AND NOT EXISTS(SELECT 1 FROM linggan_ci_atom_projection p WHERE p.analysis_ref=a.work_ref)
      ORDER BY a.created_at,a.work_ref LIMIT $1
    "#).bind(limit.clamp(1,500)).fetch_all(db.pool()).await?;
    let mut projected = 0;
    for row in rows {
        let source: Uuid = row.get("source_ref");
        let analysis: Uuid = row.get("analysis_ref");
        let result: Value = row.get("result");
        let body: Option<String> = row.get("body_text");
        let Some(body) = body else { continue };
        let atoms = accepted_atoms(&result, source, &body);
        let version = if row.get::<String, _>("rule_version") == "comment-research.v5" {
            "comment-atoms.v5.1"
        } else {
            "comment-atoms.v4-adapter.1"
        };
        let mut tx = db.pool().begin().await?;
        sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
            .fetch_one(&mut *tx)
            .await?;
        for atom in &atoms {
            let definition=comment_source_hash(&json!({"kind":atom.kind,"meaning":atom.meaning,"context":atom.context,"target":atom.target,"position":atom.position,"contract":version}).to_string());
            sqlx::query("INSERT INTO linggan_ci_semantic_atom(atom_ref,analysis_ref,domain_ref,canonical_ref,source_ref,kind,ordinal,atom_contract_version,definition_hash,meaning,context_text,evidence,context_evidence,target,position) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) ON CONFLICT(analysis_ref,kind,ordinal,atom_contract_version) DO NOTHING")
              .bind(Uuid::new_v4()).bind(analysis).bind(row.get::<Uuid,_>("domain_ref")).bind(row.get::<Uuid,_>("canonical_ref")).bind(source)
              .bind(&atom.kind).bind(atom.ordinal).bind(version).bind(definition).bind(&atom.meaning).bind(&atom.context).bind(&atom.evidence).bind(&atom.context_evidence).bind(&atom.target).bind(&atom.position).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO linggan_ci_atom_projection(analysis_ref,atom_contract_version,atom_count) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
          .bind(analysis).bind(version).bind(atoms.len() as i32).execute(&mut *tx).await?;
        tx.commit().await?;
        projected += 1;
    }
    Ok(projected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_types_share_an_ordinal_without_collapsing() {
        let source = Uuid::new_v4();
        let evidence = json!([{"sourceRef":source,"startChar":0,"endChar":2}]);
        let result = json!({"semantic":{"outcome":"interpretable","atoms":[{"kind":"need","ordinal":1,"meaning":"需要帮助","basis":"explicit","evidence":evidence,"contextEvidence":[]},{"kind":"solution","ordinal":1,"meaning":"尝试方法","basis":"explicit","evidence":evidence,"contextEvidence":[]}]}});
        let values = accepted_atoms(&result, source, "求助");
        assert_eq!(values.len(), 2);
        assert_ne!(values[0].kind, values[1].kind);
    }
    #[test]
    fn invalid_or_uncertain_atoms_never_create_members() {
        let source = Uuid::new_v4();
        let result = json!({"semantic":{"outcome":"uncertain","atoms":[{"kind":"problem","ordinal":1,"meaning":"未知","evidence":[]}]}});
        assert!(accepted_atoms(&result, source, "未知").is_empty());
    }
}
