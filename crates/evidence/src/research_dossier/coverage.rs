use serde_json::{Value, json};

pub(super) fn project(materials: &[Value], expected: Option<u64>) -> Value {
    let acquired_images = materials.iter().filter(|material| {
        material["kind"] == "image"
            && matches!(
                material["sourceState"].as_str(),
                Some("present" | "known_empty")
            )
    });
    let acquired = page_count(acquired_images.clone(), expected);
    let processed = page_count(
        acquired_images.filter(|material| material["currentRevisionRef"].is_string()),
        expected,
    );
    json!({"imageInventory":{"basis":if expected.is_some(){"platform_image_list"}else{"observed_set"},"expectedPages":expected,
        "acquiredPages":acquired,"processedPages":processed,"missingPages":[],
        "unknownOrder":false},
        "sources":{"body":source(materials,"body"),"images":source(materials,"image"),"audio":source(materials,"audio"),"videoFrames":source(materials,"video_frame")},
        "issues":[]})
}

// Supplemental images stay readable without counting as confirmed platform pages.
fn page_count<'a>(images: impl Iterator<Item = &'a Value>, expected: Option<u64>) -> usize {
    match expected {
        Some(total) => images
            .filter(|image| {
                matches!(image["purpose"].as_str(), Some("body_image" | "live_photo"))
                    || image["pageSource"]["basis"] == "same_verified_bytes"
            })
            .filter_map(|image| image["page"].as_u64())
            .filter(|page| (1..=total).contains(page))
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        None => images.count(),
    }
}

fn source(materials: &[Value], kind: &str) -> Value {
    let units: Vec<_> = materials.iter().filter(|m| m["kind"] == kind).collect();
    let readings = units
        .iter()
        .filter(|m| {
            m["sourceState"] != "restricted"
                && (m["currentRevisionRef"].is_string()
                    || (kind == "body"
                        && m["blocks"].as_array().is_some_and(|blocks| {
                            blocks.iter().any(super::content::has_source_text)
                        })))
        })
        .count();
    let all = |state: &str| !units.is_empty() && units.iter().all(|m| m["sourceState"] == state);
    let state = if all("not_applicable") {
        "not_applicable"
    } else if all("restricted") {
        "restricted"
    } else if all("known_empty") {
        "known_empty"
    } else if units.is_empty() || all("not_acquired") {
        "not_acquired"
    } else if readings == units.len() {
        let missing_information = units.iter().any(|unit| {
            unit["blocks"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|block| {
                    block["issues"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|issue| issue["code"] == "visual_information_missing")
                })
        });
        if missing_information {
            "partial"
        } else {
            "available"
        }
    } else if readings > 0 {
        "partial"
    } else {
        "not_processed"
    };
    json!({"state":state,"units":units.len(),"readings":readings})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_standalone_cover_cannot_fill_a_missing_platform_page() {
        let cover = json!({
            "kind":"image","purpose":"cover","page":1,"sourceState":"present",
            "currentRevisionRef":"cover-reading"
        });
        let page = json!({
            "kind":"image","purpose":"body_image","page":2,"sourceState":"present",
            "currentRevisionRef":null
        });
        let inventory =
            |materials: &[Value], expected| project(materials, expected)["imageInventory"].clone();
        let standalone = inventory(&[cover.clone(), page.clone()], Some(2));
        assert_eq!(standalone["acquiredPages"], 1);
        assert_eq!(standalone["processedPages"], 0);
        let observed = inventory(&[cover.clone(), page.clone()], None);
        assert_eq!(observed["acquiredPages"], 2);
        assert_eq!(observed["processedPages"], 1);

        let mut verified = cover;
        verified["pageSource"] = json!({"basis":"same_verified_bytes"});
        let matched = inventory(&[verified, page.clone(), page], Some(2));
        assert_eq!(matched["acquiredPages"], 2);
        assert_eq!(matched["processedPages"], 1);
    }

    #[test]
    fn missing_visual_information_is_partial_without_discarding_the_reading() {
        let mut image = json!({"kind":"image","sourceState":"present","currentRevisionRef":"reading",
            "blocks":[{"kind":"paragraph","text":"已有文字","issues":[{"code":"visual_information_missing"}]}]});
        assert_eq!(
            source(&[image.clone()], "image"),
            json!({"state":"partial","units":1,"readings":1})
        );
        image["blocks"][0]["issues"] = json!([]);
        assert_eq!(source(&[image], "image")["state"], "available");
    }
    #[test]
    fn native_body_is_available_without_a_persisted_reading() {
        let body = json!({"kind":"body","sourceState":"present","currentRevisionRef":null,
            "blocks":[{"kind":"paragraph","text":"原生正文"}]});
        assert_eq!(source(&[body.clone()], "body")["state"], "available");
        let mut gap = body.clone();
        gap["blocks"][0]["kind"] = json!("gap");
        assert_eq!(source(&[gap], "body")["state"], "not_processed");
        let mut restricted = body;
        restricted["sourceState"] = json!("restricted");
        assert_eq!(source(&[restricted], "body")["state"], "restricted");
        // No observed motion is not evidence that the original had no audio.
        assert_eq!(source(&[], "audio")["state"], "not_acquired");
    }
}
