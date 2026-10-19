use super::{Column, ColumnState};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(super) struct Status {
    pub kind: String,
    pub slot_key: String,
    pub job_ref: Option<Uuid>,
    pub position: String,
    pub state: String,
    pub reason: String,
}

pub(super) async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Vec<Status>>, sqlx::Error> {
    let rows=sqlx::query(r#"
WITH slots AS (
 SELECT DISTINCT o.content_public_ref,o.slot_key,o.purpose FROM linggan_material_media_origin o
 JOIN linggan_runtime_capture_package p USING(package_ref)
 WHERE o.content_public_ref=ANY($1) AND o.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
 AND o.purpose NOT IN ('comment_image','author_avatar')
), latest AS (
 SELECT DISTINCT ON(s.content_public_ref,s.slot_key,j.processor_kind,j.input_scope)
 s.content_public_ref,s.slot_key,s.purpose,j.job_ref,j.processor_kind,j.input_scope,
 w.state AS work_state,(w.updated_at<=$2::timestamptz) AS work_state_at_cutoff,
 EXISTS(SELECT 1 FROM linggan_current_material_media_disposition e WHERE e.slot_key=s.slot_key AND e.state='WITHDRAWN_OR_RESTRICTED') AS slot_restricted,
 e.state AS event_state,e.reason AS event_reason
 FROM slots s LEFT JOIN linggan_media_processing_job j ON j.slot_key=s.slot_key
 AND j.created_at<=$2::timestamptz AND j.processor_kind IN ('image_ocr','asr')
 LEFT JOIN linggan_media_processing_work w USING(job_ref)
 LEFT JOIN LATERAL(SELECT state,reason FROM linggan_media_processing_job_event
 WHERE job_ref=j.job_ref AND occurred_at<=$2::timestamptz ORDER BY occurred_at DESC,event_ref DESC LIMIT 1) e ON true
 ORDER BY s.content_public_ref,s.slot_key,j.processor_kind,j.input_scope,j.created_at DESC,j.job_ref DESC
)
SELECT latest.*,slot.ordinal AS slot_ordinal FROM latest JOIN linggan_media_slot slot USING(slot_key)
ORDER BY content_public_ref,slot.ordinal,purpose,processor_kind,input_scope
"#).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    let mut out = BTreeMap::<Uuid, Vec<Status>>::new();
    for row in rows {
        let kind = row
            .get::<Option<String>, _>("processor_kind")
            .unwrap_or_else(|| {
                if row.get::<String, _>("purpose").contains("video") {
                    "asr".into()
                } else {
                    "image_ocr".into()
                }
            });
        let work = if row.get::<Option<bool>, _>("work_state_at_cutoff") == Some(true) {
            row.get::<Option<String>, _>("work_state")
        } else {
            None
        };
        let (state, reason) = if row.get::<bool, _>("slot_restricted") {
            ("restricted", "source_restricted")
        } else {
            classify(
                work.as_deref(),
                row.get::<Option<String>, _>("event_state").as_deref(),
                row.get::<Option<String>, _>("event_reason").as_deref(),
            )
        };
        out.entry(row.get("content_public_ref"))
            .or_default()
            .push(Status {
                kind,
                slot_key: row.get("slot_key"),
                job_ref: row.get("job_ref"),
                position: format!(
                    "{}:{}",
                    row.get::<i32, _>("slot_ordinal"),
                    row.get::<Option<String>, _>("input_scope")
                        .unwrap_or_default()
                ),
                state: state.into(),
                reason: reason.into(),
            });
    }
    Ok(out)
}

fn classify(
    work: Option<&str>,
    event: Option<&str>,
    reason: Option<&str>,
) -> (&'static str, &'static str) {
    match work {
        Some("pending" | "leased" | "retry_wait") => ("waiting", "media_processing_pending"),
        Some("terminal") => ("terminal_failure", "media_processing_terminal"),
        Some("not_applicable") => ("not_adopted", "media_processing_not_applicable"),
        Some("completed") if event == Some("succeeded") => {
            if matches!(reason, Some("no_text_observed" | "no_speech_observed")) {
                ("known_empty", "processor_reported_empty")
            } else {
                ("available", "processed")
            }
        }
        _ if event == Some("succeeded") => {
            if matches!(reason, Some("no_text_observed" | "no_speech_observed")) {
                ("known_empty", "processor_reported_empty")
            } else {
                ("available", "processed")
            }
        }
        _ => ("waiting", "media_work_state_unavailable"),
    }
}

pub(super) fn columns(
    statuses: &[Status],
    media: &[Value],
    has_ocr: bool,
    has_asr: bool,
) -> Vec<Column> {
    [
        ("ocr", "image_ocr", has_ocr),
        ("transcript", "asr", has_asr),
    ]
    .into_iter()
    .map(|(field, kind, available)| {
        let items: Vec<_> = statuses.iter().filter(|s| s.kind == kind).collect();
        let restricted = items.iter().any(|s| s.state == "restricted")
            || media
                .iter()
                .any(|m| m["processorKind"] == kind && m["state"] == "WITHDRAWN_OR_RESTRICTED");
        let relayer = kind == "image_ocr"
            && media.iter().any(|m| {
                m["processorKind"] == kind
                    && m["contextOnly"] != true
                    && m["state"] == "ACQUIRED"
                    && m["ocrLayering"]["layerVersion"] != crate::ocr_layering::VERSION
            });
        let waiting = items.iter().any(|s| {
            s.state == "waiting"
                && !media.iter().any(|m| {
                    m["slotKey"] == s.slot_key
                        && m["jobRef"].as_str().and_then(|v| v.parse::<Uuid>().ok()) == s.job_ref
                        && m["processorKind"] == kind
                        && m["state"] == "WITHDRAWN_OR_RESTRICTED"
                })
        });
        let state = if waiting {
            ColumnState::Waiting
        } else if relayer {
            ColumnState::NeedsRelayer
        } else if restricted {
            ColumnState::Restricted
        } else if available {
            ColumnState::Available
        } else if items.iter().any(|s| s.state == "terminal_failure") {
            ColumnState::TerminalFailure
        } else if !items.is_empty() && items.iter().all(|s| s.state == "known_empty") {
            ColumnState::KnownEmpty
        } else if !items.is_empty() && items.iter().all(|s| s.state == "not_adopted") {
            ColumnState::NotAdopted
        } else if items.is_empty() {
            ColumnState::NotProvided
        } else {
            ColumnState::NotAcquired
        };
        let mut reasons: Vec<String> = items
            .iter()
            .filter(|s| s.state != "available")
            .map(|s| format!("{}:{}", s.position, s.reason))
            .collect();
        if relayer {
            reasons.push("current_layer_required".into());
        }
        if restricted {
            reasons.push("source_restricted".into());
        }
        Column {
            field: field.into(),
            state,
            reasons,
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_event_does_not_prove_terminal_and_expired_leases_still_wait() {
        for work in [Some("retry_wait"), Some("leased"), Some("pending"), None] {
            assert_eq!(classify(work, Some("failed"), None).0, "waiting");
        }
        assert_eq!(
            classify(Some("terminal"), Some("failed"), None).0,
            "terminal_failure"
        );
        assert_eq!(
            classify(
                Some("completed"),
                Some("succeeded"),
                Some("no_text_observed")
            )
            .0,
            "known_empty"
        );
    }
    #[test]
    fn one_restricted_slot_cannot_hide_another_pending_slot() {
        let pending = Status {
            kind: "image_ocr".into(),
            slot_key: "pending-slot".into(),
            job_ref: None,
            position: "2".into(),
            state: "waiting".into(),
            reason: "media_processing_pending".into(),
        };
        let media = vec![
            serde_json::json!({"slotKey":"restricted-slot","processorKind":"image_ocr","state":"WITHDRAWN_OR_RESTRICTED"}),
        ];
        assert_eq!(
            columns(&[pending], &media, false, false)[0].state,
            ColumnState::Waiting
        );
    }
}
