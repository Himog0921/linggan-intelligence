use super::*;

/// Enrich only already admitted, unrestricted results. Legacy reads do not require the new table.
pub(crate) async fn attach_media(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [Value],
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let jobs: Vec<Uuid> = items
        .iter()
        .filter(|v| v["state"] == "ACQUIRED" && v["processorVersion"] == PROCESSOR_VERSION)
        .filter_map(|v| v["jobRef"].as_str()?.parse().ok())
        .collect();
    if jobs.is_empty() {
        return Ok(());
    }
    let rows=sqlx::query("SELECT job.job_ref,a.adoption_ref,a.derivative_ref,a.layout_ref,a.rule_version,a.input_fingerprint,a.observation,a.assessment,a.line_ref_map,a.image_text,a.cover_title,a.state \
        FROM linggan_media_ocr_adoption a JOIN linggan_media_derivative d USING(derivative_ref) JOIN linggan_media_processing_job job USING(job_ref) \
        WHERE job.job_ref=ANY($1) AND a.created_at<=$2::text::timestamptz")
        .bind(&jobs).bind(as_of).fetch_all(&mut **tx).await?;
    for item in items
        .iter_mut()
        .filter(|v| v["state"] == "ACQUIRED" && v["processorVersion"] == PROCESSOR_VERSION)
    {
        // A raw derivative alone never qualifies text. A missing projection fails closed.
        item["displayText"] = Value::Null;
        item["researchText"] = Value::Null;
        item["ocrLayering"] = Value::Null;
        let Some(row) = rows
            .iter()
            .find(|r| json!(r.get::<Uuid, _>("job_ref")) == item["jobRef"])
        else {
            continue;
        };
        let input: ImageInput = serde_json::from_value(row.get("observation")).map_err(decode)?;
        let a: Assessment = serde_json::from_value(row.get("assessment")).map_err(decode)?;
        // Validate stored provenance rather than recomputing old results under future rules.
        if a.image != input.image
            || a.input_fingerprint != row.get::<String, _>("input_fingerprint")
            || a.rule_version != row.get::<String, _>("rule_version")
        {
            return Err(sqlx::Error::Protocol(
                "ocr_adoption_identity_mismatch".into(),
            ));
        }
        // Re-project saved decisions with gap annotations; do not re-assess old OCR.
        let projection = rule::project_work(std::slice::from_ref(&a)).map_err(|_| {
            sqlx::Error::Protocol("ocr_adoption_projection_version_unsupported".into())
        })?;
        let image = &projection.images[0];
        let text = &image.text;
        let title: Option<String> = row.get("cover_title");
        item["derivativeRef"] = json!(row.get::<Uuid, _>("derivative_ref"));
        item["displayText"] = json!(text.chars().take(600).collect::<String>());
        item["researchText"] = json!(text);
        item["ocrAdoption"] = json!({"assessment":a,"lineRefMap":row.get::<Value,_>("line_ref_map"),"sourceRelation":"image_transcription_not_author_claim","rawBlocks":input.layout.blocks,"segments":image.segments,"pendingBlocks":image.pending_blocks,"projectionVersion":"ocr-source-gaps-v1"});
        item["ocrLayering"] = json!({"layoutRef":row.get::<Uuid,_>("layout_ref"),"layeringRef":row.get::<Uuid,_>("adoption_ref"),
            "layerVersion":a.rule_version,"state":row.get::<String,_>("state"),"decisionSource":"local_ocr_adoption",
            "imageWidth":a.image.width,"imageHeight":a.image.height,"engineVersion":input.layout.pipeline.version,
            "coverHeadline":title,"imageSubstantiveText":text,"cleanCorpusEligible":!projection.fragments.is_empty(),
            "retainedLines":[],"excludedLines":[],"sourceFragments":a.regions.iter().flat_map(|r|&r.usable_fragments).collect::<Vec<_>>()});
    }
    Ok(())
}
fn decode(error: serde_json::Error) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(error))
}

pub(crate) fn project_dossier_images(media: &[Value]) -> Value {
    let images: Vec<Assessment> = media
        .iter()
        .filter(|m| m["state"] == "ACQUIRED" && m["contextOnly"] != true)
        .filter_map(|m| serde_json::from_value(m["ocrAdoption"]["assessment"].clone()).ok())
        .collect();
    if images.is_empty() {
        return Value::Null;
    }
    match rule::project_work(&images) {
        Ok(p) => json!({"state":"OBSERVED_IMAGES_ONLY","projection":p}),
        Err(_) => json!({"state":"ORDER_OR_IDENTITY_UNRESOLVED","projection":null}),
    }
}
