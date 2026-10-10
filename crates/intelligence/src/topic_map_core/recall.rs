//! Chinese/Latin lexical recall fused with the existing qualified local WeMM space.
use crate::{
    model_settings::ModelError, pi_adapter::PiAdapter, topic_map_research_analysis::Discussion,
};
use linggan_evidence::creator_discovery::hash;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
const TEMPLATE: &str = "topic-map.concept-embedding.v1";

pub(crate) async fn catalog(db: &Database, domain: Uuid) -> Result<Value, ModelError> {
    let unavailable = super::unavailable_definitions(db, Some(domain)).await?;
    let mut catalog = catalog_with(db.pool(), domain).await?;
    if let Some(rows) = catalog.as_array_mut() {
        rows.retain(|r| {
            r["definitionRef"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .is_some_and(|id| !unavailable.contains(&id))
        });
    }
    Ok(catalog)
}
pub(crate) async fn catalog_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
) -> Result<Value, ModelError> {
    catalog_with(&mut **tx, domain).await
}
async fn catalog_with<'e, E: sqlx::Executor<'e, Database = sqlx::Postgres>>(
    executor: E,
    domain: Uuid,
) -> Result<Value, ModelError> {
    let rows=sqlx::query("SELECT t.topic_ref,d.definition_ref,d.version,d.display_name,d.definition_text,c.inclusion_criteria,c.exclusion_criteria FROM linggan_topic_workspace t JOIN LATERAL(SELECT *FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true JOIN LATERAL(SELECT *FROM linggan_topic_map_binding WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)b ON true LEFT JOIN linggan_topic_map_concept_rule c ON c.definition_ref=d.definition_ref WHERE b.domain_ref=$1 AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=t.topic_ref) ORDER BY t.topic_ref")
        .bind(domain).fetch_all(executor).await?;
    Ok(json!(rows.iter().map(|r|json!({"topicRef":r.get::<Uuid,_>("topic_ref"),"definitionRef":r.get::<Uuid,_>("definition_ref"),"version":r.get::<i32,_>("version"),"label":r.get::<String,_>("display_name"),"definition":r.get::<String,_>("definition_text"),"inclusionCriteria":r.get::<Option<Vec<String>>,_>("inclusion_criteria").unwrap_or_default(),"exclusionCriteria":r.get::<Option<Vec<String>>,_>("exclusion_criteria").unwrap_or_default()})).collect::<Vec<_>>()))
}
pub(crate) async fn catalog_version(db: &Database, domain: Uuid) -> Result<String, ModelError> {
    Ok(hash(&serde_json::json!(catalog_with(db.pool(),domain).await?.as_array().into_iter().flatten().map(|c|serde_json::json!({"topicRef":c["topicRef"],"definitionRef":c["definitionRef"]})).collect::<Vec<_>>()).to_string()))
}
fn encode_text(label: &str, definition: &str, included: &[String], excluded: &[String]) -> String {
    format!(
        "主题：{}\n讨论范围：{}\n纳入：{}\n排除：{}",
        label.trim(),
        definition.trim(),
        included.join("；"),
        excluded.join("；")
    )
}
pub(super) fn concept_text(c: &Value) -> String {
    let array = |key: &str| {
        c[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect::<Vec<_>>()
    };
    encode_text(
        c["label"].as_str().unwrap_or(""),
        c["definition"].as_str().unwrap_or(""),
        &array("inclusionCriteria"),
        &array("exclusionCriteria"),
    )
}
pub(super) fn terms(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut previous = None;
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            word.push(ch);
            previous = None;
        } else {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            if !ch.is_ascii() && ch.is_alphanumeric() {
                if let Some(p) = previous {
                    out.push(format!("{p}{ch}"));
                }
                previous = Some(ch);
            } else {
                previous = None;
            }
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}
pub(super) fn lexical_scores(query: &str, documents: &[String]) -> Vec<f64> {
    let documents: Vec<Vec<String>> = documents.iter().map(|s| terms(s)).collect();
    let query: HashSet<_> = terms(query).into_iter().collect();
    let average = (documents.iter().map(Vec::len).sum::<usize>() as f64
        / documents.len().max(1) as f64)
        .max(1.0);
    let mut df = HashMap::<&str, usize>::new();
    for doc in &documents {
        for term in doc.iter().map(String::as_str).collect::<HashSet<_>>() {
            *df.entry(term).or_default() += 1;
        }
    }
    documents
        .iter()
        .map(|doc| {
            let mut tf = HashMap::<&str, usize>::new();
            for t in doc {
                *tf.entry(t).or_default() += 1;
            }
            query
                .iter()
                .map(|term| {
                    let frequency = *tf.get(term.as_str()).unwrap_or(&0) as f64;
                    let docs = *df.get(term.as_str()).unwrap_or(&0) as f64;
                    let idf = (1.0 + (documents.len() as f64 - docs + 0.5) / (docs + 0.5)).ln();
                    idf * frequency * 2.2
                        / (frequency + 1.2 * (0.25 + 0.75 * doc.len() as f64 / average))
                })
                .sum()
        })
        .collect()
}
fn valid_vector(values: &[f64]) -> bool {
    values.len() == 512
        && values.iter().all(|v| v.is_finite())
        && (values.iter().map(|v| v * v).sum::<f64>() - 1.0).abs() < 0.03
}

pub(crate) async fn recall_topics(
    db: &Database,
    adapter: &PiAdapter,
    units: &[(String, Discussion)],
    all: &Value,
) -> Result<(Value, Value), ModelError> {
    let topics = all.as_array().cloned().unwrap_or_default();
    let texts: Vec<_> = topics.iter().map(concept_text).collect();
    let queries: Vec<_> = units
        .iter()
        .map(|(_, d)| {
            encode_text(
                &d.label,
                &d.definition,
                &d.inclusion_criteria,
                &d.exclusion_criteria,
            )
        })
        .collect();
    let LocalIndex {
        vectors,
        profile,
        reason,
    } = local_index(db, adapter, &queries, &texts).await?;
    let mut selected = HashMap::<usize, f64>::new();
    for query in &queries {
        let lexical = lexical_scores(query, &texts);
        let mut lexical_rank: Vec<_> = lexical
            .iter()
            .enumerate()
            .filter(|(_, s)| **s > 0.0)
            .map(|(i, s)| (i, *s))
            .collect();
        lexical_rank.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut fused = HashMap::<usize, f64>::new();
        for (rank, (index, _)) in lexical_rank.iter().enumerate() {
            *fused.entry(*index).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
        }
        if let Some(vector) = vectors.get(&hash(query)) {
            let mut ranked: Vec<_> = texts
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    vectors
                        .get(&hash(t))
                        .map(|v| (i, v.iter().zip(vector).map(|(a, b)| a * b).sum::<f64>()))
                })
                .collect();
            ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            for (rank, (index, _)) in ranked.iter().enumerate() {
                *fused.entry(*index).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
            }
        }
        if topics.len() <= 6 {
            for index in 0..topics.len() {
                fused.entry(index).or_insert(0.0);
            }
        }
        let mut ranked: Vec<_> = fused.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        for (index, score) in ranked.into_iter().take(6) {
            selected
                .entry(index)
                .and_modify(|s| *s = s.max(score))
                .or_insert(score);
        }
    }
    let mut selected: Vec<_> = selected.into_iter().collect();
    selected.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let candidates: Vec<_> = selected
        .into_iter()
        .map(|(i, s)| {
            let mut t = topics[i].clone();
            t["recallScore"] = json!(s);
            t
        })
        .collect();
    let indexed = texts
        .iter()
        .filter(|t| vectors.contains_key(&hash(t)))
        .count();
    let complete = profile.is_some()
        && indexed == topics.len()
        && queries.iter().all(|q| vectors.contains_key(&hash(q)));
    Ok((
        json!(candidates),
        json!({"method":"bm25-cjk+reciprocal-rank-fusion","embeddingModel":"Tencent/WeMM-Embedding-2B","embeddingProfileRef":profile,"templateVersion":TEMPLATE,"catalogTopics":topics.len(),"indexedTopics":indexed,"candidateLimitPerUnit":6,"state":if complete{"hybrid"}else{"lexical_partial"},"reason":if complete{None}else{Some(reason)},"similarityIsMembership":false}),
    ))
}
struct LocalIndex {
    vectors: HashMap<String, Vec<f64>>,
    profile: Option<Uuid>,
    reason: &'static str,
}
async fn local_index(
    db: &Database,
    adapter: &PiAdapter,
    queries: &[String],
    texts: &[String],
) -> Result<LocalIndex, ModelError> {
    let profile_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_study_embedding_profile')IS NOT NULL",
    )
    .fetch_one(db.pool())
    .await?;
    let profile = if profile_ready {
        crate::comment_study_embedding::active_profile(db).await?
    } else {
        None
    };
    let mut vectors = HashMap::<String, Vec<f64>>::new();
    let mut reason = "no_qualified_local_embedding";
    if let Some(profile) = profile {
        let rows=sqlx::query("SELECT text_hash,embedding::text AS values FROM linggan_topic_map_embedding WHERE profile_ref=$1 AND template_version=$2")
            .bind(profile).bind(TEMPLATE).fetch_all(db.pool()).await?;
        for r in rows {
            if let Ok(v) = serde_json::from_str::<Vec<f64>>(&r.get::<String, _>("values")) {
                if valid_vector(&v) {
                    vectors.insert(r.get("text_hash"), v);
                }
            }
        }
        let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_media_processing_work w JOIN linggan_media_processing_job j USING(job_ref) JOIN linggan_media_processing_concurrency c ON c.processor_kind=j.processor_kind WHERE w.state='leased')")
            .fetch_one(db.pool()).await?;
        reason = if busy {
            "local_media_processing_busy"
        } else {
            "local_index_partial"
        };
        let mut remaining = if busy { 0 } else { 16 };
        for text in queries.iter().chain(texts.iter()) {
            let key = hash(text);
            if vectors.contains_key(&key) || remaining == 0 {
                continue;
            }
            remaining -= 1;
            let response = match adapter.embed_wemm_document(text).await {
                Ok(r) => r,
                Err(_) => {
                    reason = "local_embedding_unavailable";
                    break;
                }
            };
            let Some(vector) = response
                .values
                .as_ref()
                .filter(|vs| vs.len() == 1)
                .and_then(|vs| vs.first())
                .filter(|v| response.ok && valid_vector(v))
            else {
                reason = "local_embedding_invalid";
                break;
            };
            sqlx::query("INSERT INTO linggan_topic_map_embedding(profile_ref,template_version,text_hash,embedding)VALUES($1,$2,$3,$4::text::public.vector)ON CONFLICT DO NOTHING")
                .bind(profile).bind(TEMPLATE).bind(&key).bind(serde_json::to_string(vector).map_err(|_|ModelError::InvalidOutput)?).execute(db.pool()).await?;
            vectors.insert(key, vector.clone());
        }
    }
    Ok(LocalIndex {
        vectors,
        profile,
        reason,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_start_from_sustain_and_preserves_unicode() {
        let docs = vec![
            "主题：启动困难；开始前迟迟无法行动".into(),
            "主题：持续注意；已经开始后反复走神".into(),
            "主题：午饭餐厅".into(),
        ];
        let scores = lexical_scores("开始前无法行动的启动困难", &docs);
        assert!(scores[0] > scores[1] && scores[0] > scores[2]);
        assert!(terms("ADHD 开始困难🙂").contains(&"adhd".into()));
    }
    #[test]
    fn rejects_invalid_vector_spaces() {
        assert!(!valid_vector(&[1.0, 0.0]));
        let mut v = vec![0.0; 512];
        v[0] = 1.0;
        assert!(valid_vector(&v));
        v[2] = f64::NAN;
        assert!(!valid_vector(&v));
    }
}
