//! Optional human adoption overlay. Raw observations and local machine decisions remain intact.
use super::*;
pub(super) async fn attach_in(
    tx: &mut Transaction<'_, Postgres>,
    item: &mut Value,
    id: Uuid,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let present: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_media_ocr_review') IS NOT NULL")
            .fetch_one(&mut **tx)
            .await?;
    if !present {
        return Ok(());
    }
    let rows=sqlx::query("SELECT DISTINCT ON(field) review_ref,revision,field,after_text,source_kind,block_ref,source_start,source_end,created_at::text AS reviewed_at FROM linggan_media_ocr_review WHERE adoption_ref=$1 AND decision='adopt' AND created_at<=$2::text::timestamptz ORDER BY field,revision DESC").bind(id).bind(as_of).fetch_all(&mut **tx).await?;
    for row in rows {
        let field: String = row.get("field");
        let text: String = row.get("after_text");
        let record = json!({"reviewRef":row.get::<Uuid,_>("review_ref"),"revision":row.get::<i64,_>("revision"),"field":field,"text":text,
    "sourceKind":row.get::<String,_>("source_kind"),"blockRef":row.get::<Option<String>,_>("block_ref"),"start":row.get::<Option<i32>,_>("source_start"),"end":row.get::<Option<i32>,_>("source_end"),"reviewedAt":row.get::<String,_>("reviewed_at")});
        if !item["ocrHumanAdoption"].is_object() {
            item["ocrHumanAdoption"] = json!({})
        }
        item["ocrHumanAdoption"][&field] = record;
        if field == "adopted_body" {
            item["machineResearchText"] = item["researchText"].clone();
            let source = if row.get::<String, _>("source_kind") == "ocr_block" {
                let block = row.get::<Option<String>, _>("block_ref");
                let bbox = item["ocrAdoption"]["rawBlocks"]
                    .as_array()
                    .and_then(|rows| {
                        rows.iter()
                            .find(|v| v["blockRef"].as_str() == block.as_deref())
                    })
                    .map(|v| v["bbox"].clone());
                json!({"sourceKind":"block","sourceRef":block,"start":row.get::<Option<i32>,_>("source_start"),"end":row.get::<Option<i32>,_>("source_end"),"text":text,"bbox":bbox,"reviewRef":row.get::<Uuid,_>("review_ref")})
            } else {
                json!({"sourceKind":"manual_transcription","sourceRef":row.get::<Uuid,_>("review_ref"),"start":0,"end":text.chars().count(),"text":text,"bbox":null})
            };
            item["ocrLayering"]["machineSourceFragments"] =
                item["ocrLayering"]["sourceFragments"].clone();
            item["ocrLayering"]["sourceFragments"] = json!([source]);
            item["ocrLayering"]["decisionSource"] = json!("human_ocr_review");
            item["researchText"] = json!(text);
            item["displayText"] = json!(text.chars().take(600).collect::<String>());
            item["ocrLayering"]["imageSubstantiveText"] = json!(text);
            item["ocrLayering"]["cleanCorpusEligible"] = json!(!text.is_empty());
        } else {
            item["ocrLayering"]["coverHeadline"] = json!(text);
        }
    }
    Ok(())
}
pub(super) fn project(media: &[Value], mut p: Value) -> Value {
    let Some(images) = p["images"].as_array_mut() else {
        return p;
    };
    for image in images.iter_mut() {
        let Some(m) = media
            .iter()
            .find(|m| m["ocrAdoption"]["assessment"]["image"]["imageRef"] == image["imageRef"])
        else {
            continue;
        };
        let review = &m["ocrHumanAdoption"]["adopted_body"];
        if let Some(text) = review["text"].as_str() {
            image["text"] = json!(text);
            image["pendingBlocks"] = json!([]);
            let source = m["ocrLayering"]["sourceFragments"][0].clone();
            image["segments"] = json!([{"kind":"source","start":0,"end":text.chars().count(),"source":source,"humanReview":review}]);
            image["humanReview"] = review.clone();
        }
    }
    let images = images.clone();
    let mut body = String::new();
    let mut fragments = vec![];
    for image in &images {
        if !image["ordinal"].is_number() {
            continue;
        }
        let text = image["text"].as_str().unwrap_or("");
        if text.is_empty() {
            continue;
        }
        if !body.is_empty() {
            body.push_str("\n\n")
        }
        let offset = body.chars().count();
        body.push_str(text);
        let Some(m) = media
            .iter()
            .find(|m| m["ocrAdoption"]["assessment"]["image"]["imageRef"] == image["imageRef"])
        else {
            continue;
        };
        for segment in image["segments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| s["kind"] == "source")
        {
            fragments.push(json!({"imageRef":image["imageRef"],"blobSha256":m["ocrAdoption"]["assessment"]["image"]["blobSha256"],"imageOrdinal":image["ordinal"],"start":offset+segment["start"].as_u64().unwrap_or(0) as usize,"end":offset+segment["end"].as_u64().unwrap_or(0) as usize,"source":segment["source"],"humanReview":segment["humanReview"]}));
        }
    }
    p["imageText"] = json!(body);
    p["fragments"] = json!(fragments);
    for m in media {
        if let Some(title) = m["ocrHumanAdoption"]["cover_headline"]["text"].as_str() {
            p["coverTitle"] = json!(title);
            p["coverTitleHumanReview"] = m["ocrHumanAdoption"]["cover_headline"].clone();
            break;
        }
    }
    p
}
