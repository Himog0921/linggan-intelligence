//! One full material projection; task adapters bound it only after source selection.
use crate::creator_discovery::{Fragment, hash};
use crate::work_resource_current::WorkResourceCurrent;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

mod relayer;
pub use relayer::relayer_work;

pub const VERSION: &str = "research-dossier.v1";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchDossier {
    pub contract: &'static str,
    pub work_ref: Uuid,
    pub revision: String,
    pub material_revision: String,
    pub as_of: String,
    pub fragments: Vec<Fragment>,
    pub media: Vec<Value>,
    pub coverage: Value,
    pub metadata: Value,
    pub sections: Vec<Value>,
    pub image_text_projection: Value,
}

/// One result per canonical slot/purpose/position. Pending reprocessing does not erase an
/// already readable source. Selection favors admitted image dimensions and available text;
/// it never splices lines from multiple layouts or ranks by average confidence alone.
pub(crate) fn select_media(media: Vec<Value>) -> Vec<Value> {
    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for item in media {
        if item["state"] == "RETIRED" {
            continue;
        }
        let mut position = item["semanticMediaSource"]["position"].clone();
        if let Some(p) = position.as_object_mut() {
            for key in ["processorVersion", "layoutRef", "blobSha256"] {
                p.remove(key);
            }
        }
        let key = json!([
            item["slotKey"],
            item["processorKind"],
            item["sourceScope"],
            position
        ])
        .to_string();
        groups.entry(key).or_default().push(item);
    }
    let mut values = Vec::new();
    for mut candidates in groups.into_values() {
        candidates.sort_by_key(media_time_rank);
        let latest = candidates.last().expect("nonempty media group");
        let mut chosen = if latest["state"] == "WITHDRAWN_OR_RESTRICTED" {
            latest.clone()
        } else {
            candidates
                .iter()
                .filter(|v| v["state"] == "ACQUIRED")
                .max_by_key(|v| {
                    let layer = &v["ocrLayering"];
                    (
                        v["ocrAdoption"].is_object(),
                        layer["imageWidth"].as_i64().unwrap_or(0)
                            * layer["imageHeight"].as_i64().unwrap_or(0),
                        v["researchText"].as_str().map_or(0, |s| s.chars().count()),
                        media_time_rank(v),
                    )
                })
                .unwrap_or(latest)
                .clone()
        };
        if chosen["jobRef"] != latest["jobRef"] {
            chosen["otherProcessing"] = json!({"jobRef":latest["jobRef"],"state":latest["state"],"processorVersion":latest["processorVersion"]});
        }
        chosen["selectionBasis"] =
            json!("readable_source,image_dimensions,text_coverage,stable_time_and_id");
        values.push(chosen);
    }
    values.sort_by_key(|v| {
        (
            v["slotOrdinal"].as_i64().unwrap_or(i64::MAX),
            v["slotKey"].to_string(),
            v["sourceScope"].to_string(),
        )
    });
    values
}

fn research_media(media: &[Value]) -> Vec<&Value> {
    let mut selected: Vec<&Value> = vec![];
    let mut positions = BTreeMap::new();
    for item in media.iter().filter(|m| {
        m["state"] == "ACQUIRED"
            && m["contextOnly"] != true
            && !matches!(
                m["dispositionState"].as_str(),
                Some("OCR_RETIRED" | "WITHDRAWN_OR_RESTRICTED")
            )
    }) {
        let source = &item["semanticMediaSource"];
        let Some(blob) = source["blobSha256"].as_str().filter(|s| !s.is_empty()) else {
            selected.push(item);
            continue;
        };
        let key = json!([
            blob,
            source["inputScope"],
            source["position"],
            item["processorKind"],
            item["sourceScope"]
        ])
        .to_string();
        let rank = |m: &Value| {
            let layer = &m["ocrLayering"];
            (
                layer["layerVersion"] == crate::ocr_layering::VERSION,
                ["coverHeadline", "imageSubstantiveText"]
                    .iter()
                    .map(|k| layer[k].as_str().map_or(0, str::len))
                    .sum::<usize>(),
            )
        };
        if let Some(&index) = positions.get(&key) {
            if rank(item) > rank(selected[index]) {
                selected[index] = item;
            }
        } else {
            positions.insert(key, selected.len());
            selected.push(item);
        }
    }
    selected
}

fn media_time_rank(v: &Value) -> (String, String, String) {
    (
        v["jobCreatedAt"].as_str().unwrap_or("").to_owned(),
        v["jobRef"].to_string(),
        v["derivativeRef"].to_string(),
    )
}

fn native(
    work: &WorkResourceCurrent,
    field: &str,
    text: Option<&str>,
    source: Option<Uuid>,
    version: Option<&str>,
) -> Option<Fragment> {
    let text = text.filter(|text| !text.trim().is_empty())?;
    Some(Fragment {
        fragment_id: format!("{}.{field}", work.public_ref),
        source_ref: source?,
        field: field.into(),
        source_version: hash(&json!([version, hash(text)]).to_string()),
        start: 0,
        end: text.chars().count(),
        text: text.into(),
    })
}

pub(crate) fn media_fragment(
    work: Uuid,
    item: &Value,
    field: &str,
    text: &str,
) -> Option<Fragment> {
    if text.trim().is_empty() {
        return None;
    }
    let source = item["derivativeRef"].as_str()?.parse().ok()?;
    let identity = &item["semanticMediaSource"];
    Some(Fragment {
        fragment_id: format!("{work}.{field}.{}", hash(&identity.to_string())),
        source_ref: source,
        field: field.into(),
        source_version: hash(
            &json!([
                source,
                item["processorVersion"],
                item["ocrLayering"]["layeringRef"],
                hash(text)
            ])
            .to_string(),
        ),
        start: 0,
        end: text.chars().count(),
        text: text.into(),
    })
}

pub(crate) fn assemble(
    work: &WorkResourceCurrent,
    media: Vec<Value>,
    as_of: &str,
) -> ResearchDossier {
    let media = select_media(media);
    let mut fragments: Vec<_> = [
        native(
            work,
            "title",
            work.title
                .as_deref()
                .filter(|_| work.title_state == "KNOWN"),
            work.title_source.material_ref,
            work.title_source.recorded_at.as_deref(),
        ),
        native(
            work,
            "body",
            work.body_text
                .as_deref()
                .filter(|_| work.body_state == "KNOWN"),
            work.body_source.material_ref,
            work.body_source.recorded_at.as_deref(),
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    for item in research_media(&media) {
        let layer = &item["ocrLayering"];
        let fields = [
            (
                "cover_headline",
                layer["coverHeadline"]
                    .as_str()
                    .filter(|_| matches!(layer["state"].as_str(), Some("ACCEPTED" | "PARTIAL"))),
            ),
            (
                "ocr",
                layer["imageSubstantiveText"]
                    .as_str()
                    .filter(|_| layer["cleanCorpusEligible"] == true),
            ),
            (
                "transcript",
                item["researchText"]
                    .as_str()
                    .filter(|_| item["kind"] == "asr_text"),
            ),
        ];
        fragments.extend(
            fields
                .into_iter()
                .filter_map(|(field, text)| media_fragment(work.public_ref, item, field, text?)),
        );
    }
    let metadata = json!({"platform":work.platform,"externalId":work.content_external_id,
        "authorExternalId":work.author_external_id,"authorDisplayName":work.creator_display_name,
        "publishedAt":work.published_at,"publishedPrecision":work.published_at_precision,"observedAt":work.observed_at,"likes":{"value":work.like_count,"state":work.like_count_state,"observedAt":work.like_source.observed_at},
        "comments":{"value":work.comment_count,"state":work.comment_count_state,"observedAt":work.comment_source.observed_at},
        "collects":{"value":work.collect_count,"state":work.collect_count_state,"observedAt":work.collect_source.observed_at},"shares":{"value":work.share_count,"state":work.share_count_state,"observedAt":work.share_source.observed_at}});
    let coverage = json!({"title":work.title_state,"titleQualification":work.title_qualification,"body":work.body_state,"mediaCount":media.len(),
        "mediaAvailable":media.iter().filter(|m| m["state"]=="ACQUIRED").count(),
        "uncertainImages":media.iter().filter(|m| matches!(m["ocrLayering"]["state"].as_str(),Some("PARTIAL"|"NEEDS_REVIEW"))).count(),
        "comments":"separate_authorized_research_channel","sampling":"collected_material_only"});
    let revision = hash(&json!([VERSION, &fragments, &media, &metadata, &coverage]).to_string());
    let sections = sections(&fragments, &coverage);
    let image_text_projection = crate::ocr_adoption_persistence::project_dossier_images(&media);
    ResearchDossier {
        image_text_projection,
        contract: VERSION,
        work_ref: work.public_ref,
        revision,
        material_revision: hash(&json!([VERSION, &fragments]).to_string()),
        as_of: as_of.into(),
        fragments,
        media,
        coverage,
        metadata,
        sections,
    }
}

/// The caller supplies its transaction/cutoff: a Run cannot accidentally receive a newer body.
pub async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, ResearchDossier>, sqlx::Error> {
    let mut output = BTreeMap::new();
    for batch in works.chunks(100) {
        let currents =
            crate::work_resource_current::read_work_resource_currents(tx, batch, as_of).await?;
        let mut media =
            crate::material_media_read::read_topic_derivatives_batch(tx, batch, as_of).await?;
        let inventory = read_inventory(tx, batch, as_of).await?;
        for work in currents {
            output.insert(
                work.public_ref,
                assemble(
                    &work,
                    media.remove(&work.public_ref).unwrap_or_default(),
                    as_of,
                ),
            );
            if let Some(dossier) = output.get_mut(&work.public_ref) {
                dossier.coverage["mediaSlots"] = inventory
                    .get(&work.public_ref)
                    .cloned()
                    .unwrap_or(json!([]));
                dossier.revision = hash(
                    &json!([
                        VERSION,
                        &dossier.fragments,
                        &dossier.media,
                        &dossier.metadata,
                        &dossier.coverage
                    ])
                    .to_string(),
                );
            }
        }
    }
    Ok(output)
}

/// Work identities within a creation reading scope; this is collected inventory, not coverage
/// of the external platform. Callers paginate the selected fragment set, not these identities.
pub async fn creation_work_directory_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    query: Option<&str>,
    offset: i64,
) -> Result<(Vec<Uuid>, i64), sqlx::Error> {
    let filter = "FROM linggan_material_content material WHERE EXISTS(SELECT 1 FROM linggan_material_domain_usage usage WHERE usage.content_public_ref=material.public_ref AND usage.domain_ref=$1) AND ($2::text IS NULL OR EXISTS(SELECT 1 FROM linggan_material_content_detail detail JOIN linggan_runtime_capture_package package USING(package_ref) WHERE detail.content_public_ref=material.public_ref AND package.accepted_at<=scope_001_now() AND (lower(COALESCE(detail.title,'')) LIKE '%'||lower($2)||'%' OR lower(COALESCE(detail.body_text,'')) LIKE '%'||lower($2)||'%')))";
    let total = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) {filter}")))
        .bind(domain)
        .bind(query)
        .fetch_one(&mut **tx)
        .await?;
    let refs = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT material.public_ref {filter} ORDER BY material.public_ref LIMIT 31 OFFSET $3"
    )))
    .bind(domain)
    .bind(query)
    .bind(offset)
    .fetch_all(&mut **tx)
    .await?;
    Ok((refs, total))
}

pub async fn creation_work_in_domain(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    domain: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE content_public_ref=$1 AND domain_ref=$2)").bind(work).bind(domain).fetch_one(&mut **tx).await
}

pub async fn read(
    database: &linggan_storage_postgres::Database,
    work: Uuid,
) -> Result<Option<ResearchDossier>, sqlx::Error> {
    read_base(database, work, true).await
}

pub async fn plan(
    database: &linggan_storage_postgres::Database,
    work: Uuid,
) -> Result<Option<Value>, sqlx::Error> {
    Ok(read_base(database, work, false)
        .await?
        .map(|d| relayer_plan(&d)))
}

async fn read_base(
    database: &linggan_storage_postgres::Database,
    work: Uuid,
    include_lines: bool,
) -> Result<Option<ResearchDossier>, sqlx::Error> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let mut dossier = read_in(&mut tx, &[work], &as_of).await?.remove(&work);
    if include_lines && let Some(dossier) = dossier.as_mut() {
        for media in &mut dossier.media {
            enrich_layout(&mut tx, media).await?;
        }
    }
    tx.commit().await?;
    Ok(dossier)
}

pub async fn enrich_layout(
    tx: &mut Transaction<'_, Postgres>,
    media: &mut Value,
) -> Result<(), sqlx::Error> {
    let Some(layout) = media["ocrLayering"]["layoutRef"]
        .as_str()
        .and_then(|s| s.parse::<Uuid>().ok())
    else {
        return Ok(());
    };
    if media["state"] != "ACQUIRED" {
        return Ok(());
    }
    if media["processorKind"] == "image_ocr" {
        // Bind the picture to the selected OCR job's bytes, never the slot's newest replica.
        let job: Uuid = serde_json::from_value(media["jobRef"].clone())
            .map_err(|_| sqlx::Error::Protocol("invalid dossier job".into()))?;
        let asset=sqlx::query("SELECT replica.materialization_ref,job.blob_sha256 FROM linggan_media_processing_job job \
            JOIN linggan_media_materialization replica ON replica.blob_sha256=job.blob_sha256 \
            JOIN linggan_media_download_attempt attempt USING(download_attempt_ref) \
            JOIN linggan_media_observation observation ON observation.observation_ref=attempt.media_observation_ref AND observation.slot_key=job.slot_key \
            WHERE job.job_ref=$1 AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition disposition \
                WHERE disposition.blob_sha256=job.blob_sha256 OR disposition.materialization_ref=replica.materialization_ref OR disposition.slot_key=job.slot_key) \
            ORDER BY replica.materialization_ref LIMIT 1").bind(job).fetch_optional(&mut **tx).await?;
        if let Some(asset) = asset {
            let replica: Uuid = asset.get("materialization_ref");
            let sha: String = asset.get("blob_sha256");
            media["originalAssetUrl"] = json!(format!("/api/local/media/{replica}/{sha}"));
            media["originalMaterializationRef"] = json!(replica);
        }
    }
    let rows=sqlx::query("SELECT line_ref,ordinal,text_content,confidence::float8 AS confidence,left_norm::float8 AS l,top_norm::float8 AS t,right_norm::float8 AS r,bottom_norm::float8 AS b FROM linggan_media_ocr_line WHERE layout_ref=$1 ORDER BY ordinal").bind(layout).fetch_all(&mut **tx).await?;
    media["ocrLayering"]["lines"]=json!(rows.iter().map(|r|json!({"lineRef":r.get::<Uuid,_>("line_ref"),"ordinal":r.get::<i32,_>("ordinal"),"text":r.get::<String,_>("text_content"),"confidence":r.get::<f64,_>("confidence"),"bbox":[r.get::<f64,_>("l"),r.get::<f64,_>("t"),r.get::<f64,_>("r"),r.get::<f64,_>("b")]})).collect::<Vec<_>>());
    Ok(())
}

/// Shared creator-material projection. The full dossier is never truncated in storage.
pub fn bounded_fragments(fragments: &[Fragment], budget: usize) -> Vec<Fragment> {
    let mut remaining = budget;
    fragments
        .iter()
        .filter_map(|f| {
            let mut f = f.clone();
            f.text = f.text.chars().take(remaining).collect();
            f.end = f.text.chars().count();
            // Creator manual support stores IDs, so a changed excerpt must not
            // inherit an earlier positive override. Full dossier IDs remain stable.
            f.fragment_id = format!("{}.{}", f.fragment_id, &hash(&f.text)[..12]);
            remaining -= f.end;
            (!f.text.is_empty()).then_some(f)
        })
        .collect()
}

pub fn context_sources(dossier: &ResearchDossier) -> Vec<Value> {
    dossier.fragments.iter().map(|f|{
        let item=dossier.media.iter().find(|m| m["derivativeRef"].as_str()==Some(&f.source_ref.to_string()));
        json!({"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"sourceVersion":f.source_version,
            "kind":match f.field.as_str(){"title"=>"native_title","ocr"=>"image_substantive_text","transcript"=>"asr_text",other=>other},
            "text":f.text,"characterCount":f.end,"slotOrdinal":item.map(|m|&m["slotOrdinal"]),
            "sourceLocation":item.map(|m|&m["semanticMediaSource"]),"contentHash":hash(&f.text)})
    }).collect()
}

/// Fixed headings and stable fragment IDs; source text is data, never instructions.
pub fn render_materials(fragments: &[Fragment]) -> String {
    let mut out = String::new();
    for (field, label) in [
        ("title", "平台标题"),
        ("cover_headline", "封面与图片大字"),
        ("body", "正文"),
        ("tags", "话题标签"),
        ("ocr", "图片正文"),
        ("transcript", "视频转写"),
    ] {
        out.push_str(&format!("## {label}\n\n"));
        let selected: Vec<_> = fragments
            .iter()
            .enumerate()
            .filter(|(_, f)| f.field == field)
            .collect();
        if selected.is_empty() {
            out.push_str("未选入，来源是否存在未知。\n\n");
        }
        for (index, f) in selected {
            out.push_str(&format!("[S{}]\n{}\n\n", index + 1, f.text));
        }
    }
    out
}

pub fn markdown(dossier: &ResearchDossier) -> String {
    let mut out = format!(
        "# 研究底稿\n\n作品：{}\n\n版本：{} / {}\n\n",
        dossier.work_ref, VERSION, dossier.revision
    );
    out.push_str(&render_materials(&dossier.fragments));
    out.push_str("## 其他图片文字\n\n待确认与排除行在原图对照中查看，不作为正文发送。\n\n## 评论\n\n已采评论由同一作品的授权评论通道分页读取，不受作品正文长度限制。\n\n");
    out.push_str(&format!(
        "## 互动与观察信息\n\n```json\n{}\n```\n\n## 完整度\n\n```json\n{}\n```\n\n",
        dossier.metadata, dossier.coverage
    ));
    out.push_str("## 来源与版本\n\n");
    for (index, f) in dossier.fragments.iter().enumerate() {
        out.push_str(&format!(
            "- [S{}] {}，来源 {}，版本 {}，字符 {}–{}\n",
            index + 1,
            f.fragment_id,
            f.source_ref,
            f.source_version,
            f.start,
            f.end
        ));
    }
    out
}

fn sections(fragments: &[Fragment], coverage: &Value) -> Vec<Value> {
    [("title","平台标题"),("cover_headline","封面与图片大字"),("body","正文"),("tags","话题标签"),("ocr","图片正文"),("background","其他图片文字"),("transcript","视频转写"),("comments","评论")]
        .into_iter().map(|(kind,label)|{
            let selected:Vec<_>=fragments.iter().filter(|f| f.field==kind).map(|f|f.fragment_id.clone()).collect();
            let state=if !selected.is_empty(){"available"}else if kind=="comments"{"separate_channel"}else{"unknown"};
            json!({"kind":kind,"label":label,"state":state,"sourceState":coverage[kind],"fragmentIds":selected})
        }).collect()
}

pub fn relayer_plan(dossier: &ResearchDossier) -> Value {
    let images: Vec<_> = dossier
        .media
        .iter()
        .filter(|m| {
            matches!(
                m["processorKind"].as_str(),
                Some("image_ocr" | "video_frame_ocr")
            ) && m["contextOnly"] != true
        })
        .collect();
    let layouts: Vec<_> = images
        .iter()
        .filter(|m| m["state"] == "ACQUIRED" && m["ocrLayering"]["layoutRef"].is_string())
        .collect();
    let current = layouts
        .iter()
        .filter(|m| m["ocrLayering"]["layerVersion"] == crate::ocr_layering::VERSION)
        .count();
    let slots: Vec<_> = dossier.coverage["mediaSlots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| matches!(s["purpose"].as_str(), Some("cover" | "body_image")))
        .collect();
    let missing = slots
        .iter()
        .filter(|s| {
            !images.iter().any(|m| {
                m["slotKey"] == s["slotKey"]
                    && (m["ocrLayering"]["layoutRef"].is_string()
                        || m["state"] == "KNOWN_EMPTY"
                        || m["state"] == "WITHDRAWN_OR_RESTRICTED")
            })
        })
        .count();
    json!({"workCount":1,"imageSlots":slots.len(),"imageJobs":images.len(),"layoutCount":layouts.len(),"pendingLayouts":layouts.len()-current,"alreadyCurrent":current,
        "needsOcr":missing,"maximumLayoutsPerWork":100,"expectedRevision":dossier.revision})
}

async fn read_inventory(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Value>, sqlx::Error> {
    let rows=sqlx::query("SELECT content_public_ref,jsonb_agg(jsonb_build_object('slotKey',slot_key,'purpose',purpose) ORDER BY slot_key,purpose) AS slots FROM (SELECT DISTINCT origin.content_public_ref,origin.slot_key,origin.purpose FROM linggan_material_media_origin origin JOIN linggan_runtime_capture_package package USING(package_ref) WHERE origin.content_public_ref=ANY($1) AND origin.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz AND origin.purpose NOT IN ('comment_image','author_avatar')) slots GROUP BY content_public_ref").bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    Ok(rows
        .iter()
        .map(|r| (r.get("content_public_ref"), r.get("slots")))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readable_layout_survives_pending_revision_and_is_never_spliced() {
        let old = json!({"slotKey":"a","processorKind":"image_ocr","sourceScope":"full","jobCreatedAt":"1","jobRef":"old","derivativeRef":"d1","state":"ACQUIRED","researchText":"完整原文","ocrLayering":{"layoutRef":"l1","imageWidth":100,"imageHeight":100}});
        let queued = json!({"slotKey":"a","processorKind":"image_ocr","sourceScope":"full","jobCreatedAt":"2","jobRef":"new","state":"QUEUED"});
        let selected = select_media(vec![queued, old.clone()]);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0]["jobRef"], "old");
        assert_eq!(selected[0]["otherProcessing"]["state"], "QUEUED");
        let mut restricted = old.clone();
        restricted["jobCreatedAt"] = json!("3");
        restricted["state"] = json!("WITHDRAWN_OR_RESTRICTED");
        assert_eq!(
            select_media(vec![old, restricted])[0]["state"],
            "WITHDRAWN_OR_RESTRICTED"
        );
    }
    #[test]
    fn bounded_input_preserves_unicode_source_range_and_full_source() {
        let full = Fragment {
            fragment_id: "w.body".into(),
            source_ref: Uuid::nil(),
            field: "body".into(),
            source_version: "v".into(),
            start: 0,
            end: 5,
            text: "中🙂文正文".into(),
        };
        let short = bounded_fragments(std::slice::from_ref(&full), 2);
        assert_eq!(short[0].text, "中🙂");
        assert_eq!(short[0].end, 2);
        assert_eq!(full.text, "中🙂文正文");
        assert!(render_materials(&short).contains("[S1]\n中🙂"));
    }
    #[test]
    fn duplicate_blobs_select_one_complete_layout_without_text_deduping() {
        let base = json!({"state":"ACQUIRED","processorKind":"image_ocr","sourceScope":"full_blob",
            "semanticMediaSource":{"blobSha256":"same","inputScope":"full_blob","position":{}},
            "ocrLayering":{"layerVersion":crate::ocr_layering::VERSION,"coverHeadline":"相同标题"}});
        let mut richer = base.clone();
        richer["ocrLayering"]["imageSubstantiveText"] = json!("有完整正文");
        let mut different = base.clone();
        different["semanticMediaSource"]["blobSha256"] = json!("different");
        let mut crop = base.clone();
        crop["semanticMediaSource"]["position"] = json!({"frame":1});
        let items = vec![base, richer, different, crop];
        let selected = research_media(&items);
        assert_eq!(selected.len(), 3);
        assert_eq!(
            selected[0]["ocrLayering"]["imageSubstantiveText"],
            "有完整正文"
        );
    }
}
