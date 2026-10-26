use super::*;
use std::collections::BTreeSet;

/// Resolve an acquired source with no reading yet, without manufacturing machine content.
pub async fn review_work_reading(
    db: &Database,
    work: Uuid,
    locator: &str,
    r: &ReviewReadingRequest,
) -> Result<Value, ReadingError> {
    validate(r)?;
    if locator != r.unit_ref {
        return Err(ReadingError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(421,hashtext($1::text))")
        .bind(r.command_ref.to_string())
        .execute(&mut *tx)
        .await?;
    lock_work(&mut tx, work).await?;
    let unit = if let Ok(id) = Uuid::parse_str(locator) {
        sqlx::query_scalar("SELECT unit_ref FROM linggan_material_unit WHERE unit_ref=$1 AND content_public_ref=$2").bind(id).bind(work).fetch_optional(&mut *tx).await?.ok_or(ReadingError::NotFound)?
    } else {
        let rest = locator
            .strip_prefix(&format!("{work}."))
            .ok_or(ReadingError::NotFound)?;
        let (kind, slot) = rest.split_once('.').ok_or(ReadingError::NotFound)?;
        if !["body", "image", "audio", "video_frame"].contains(&kind) {
            return Err(ReadingError::NotFound);
        }
        let key = if kind == "body" && slot == "native" {
            "body".to_owned()
        } else {
            format!("{kind}:{slot}")
        };
        let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
            .fetch_one(&mut *tx)
            .await?;
        let existing:Option<Uuid>=sqlx::query_scalar("SELECT unit_ref FROM linggan_material_unit WHERE content_public_ref=$1 AND unit_key=$2").bind(work).bind(&key).fetch_optional(&mut *tx).await?;
        if let Some(id) = existing {
            let row =
                sqlx::query("SELECT * FROM linggan_material_unit WHERE unit_ref=$1 FOR UPDATE")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            let mut stored = Unit::from_row(&row);
            if stored.source != r.source_version {
                if r.expected_current_revision_ref.is_some()
                    || read::current_reference_in(&mut tx, work, id, &now)
                        .await?
                        .is_some()
                {
                    return Err(ReadingError::Conflict);
                }
                let dossier = crate::research_dossier::read_in(&mut tx, &[work], &now)
                    .await?
                    .remove(&work)
                    .ok_or(ReadingError::NotFound)?;
                let material = dossier
                    .materials
                    .iter()
                    .find(|m| {
                        m["ref"] == locator
                            && m["sourceVersion"] == r.source_version
                            && m["sourceState"] == "present"
                    })
                    .ok_or(ReadingError::SourceChanged)?;
                stored.source = r.source_version.clone();
                stored.state = "present".into();
                stored.current = None;
                stored.adoption = "automatic".into();
                stored.verification = "unchecked".into();
                stored.page = material["page"]
                    .as_i64()
                    .and_then(|v| i32::try_from(v).ok());
                update_unit(&mut tx, &mut stored).await?;
                record_selection(&mut tx, &stored, "source_changed", None).await?;
            }
            id
        } else {
            let dossier = crate::research_dossier::read_in(&mut tx, &[work], &now)
                .await?
                .remove(&work)
                .ok_or(ReadingError::NotFound)?;
            let material = dossier
                .materials
                .iter()
                .find(|m| {
                    m["ref"] == locator
                        && m["sourceVersion"] == r.source_version
                        && m["sourceState"] == "present"
                })
                .ok_or(ReadingError::SourceChanged)?;
            let input = NewReading {
                content_public_ref: work,
                unit_key: key,
                kind: kind.into(),
                slot_key: material["slotKey"].as_str().map(str::to_owned),
                page: material["page"]
                    .as_i64()
                    .and_then(|v| i32::try_from(v).ok()),
                source_version: r.source_version.clone(),
                source_state: "present".into(),
                derivative_ref: None,
                producer_ref: None,
                generated_by: "human_edit".into(),
                rule_version: crate::research_dossier::content::RULE_VERSION.into(),
                blocks: vec![],
                issues: vec![],
                source_location: json!({}),
                publication: Publication::Automatic {
                    expected_current: None,
                },
            };
            write::ensure_unit(&mut tx, &input).await?.id
        }
    };
    let receipt = review_reading_in(&mut tx, unit, r).await?;
    tx.commit().await?;
    Ok(receipt)
}

/// Human corrections apply to the displayed current reading, never a separate candidate.
pub async fn review_reading(
    db: &Database,
    unit_ref: Uuid,
    r: &ReviewReadingRequest,
) -> Result<Value, ReadingError> {
    let mut tx = db.pool().begin().await?;
    let receipt = review_reading_in(&mut tx, unit_ref, r).await?;
    tx.commit().await?;
    Ok(receipt)
}
async fn review_reading_in(
    tx: &mut PgConnection,
    unit_ref: Uuid,
    r: &ReviewReadingRequest,
) -> Result<Value, ReadingError> {
    validate(r)?;
    let request_hash = crate::ocr_source::content_hash(json!([unit_ref, r]).to_string().as_bytes());
    sqlx::query("SELECT pg_advisory_xact_lock(421,hashtext($1::text))")
        .bind(r.command_ref.to_string())
        .execute(&mut *tx)
        .await?;
    let replay = sqlx::query(
        "SELECT request_hash,response FROM linggan_material_reading_command WHERE command_ref=$1",
    )
    .bind(r.command_ref)
    .fetch_optional(&mut *tx)
    .await?;
    if replay
        .as_ref()
        .is_some_and(|row| row.get::<String, _>("request_hash") != request_hash)
    {
        return Err(ReadingError::Conflict);
    }
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_unit WHERE unit_ref=$1",
    )
    .bind(unit_ref)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ReadingError::NotFound)?;
    lock_work(tx, work).await?;
    let row = sqlx::query("SELECT * FROM linggan_material_unit WHERE unit_ref=$1 FOR UPDATE")
        .bind(unit_ref)
        .fetch_one(&mut *tx)
        .await?;
    let mut unit = Unit::from_row(&row);
    let locator = format!(
        "{}.{}.{}",
        unit.work,
        unit.kind,
        unit.slot.as_deref().unwrap_or("native")
    );
    if r.unit_ref != unit.id.to_string() && r.unit_ref != locator {
        return Err(ReadingError::Invalid);
    }

    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    if unit.source != r.source_version
        || !source::current(
            tx,
            work,
            &unit.kind,
            unit.slot.as_deref(),
            &r.source_version,
            None,
            &now,
        )
        .await?
    {
        return Err(ReadingError::SourceChanged);
    }
    // Replay remains source gated, but is checked before CAS so a lost receipt is safe to retry.
    if let Some(replay) = replay {
        return Ok(replay.get("response"));
    }
    let current = read::current_reference_in(tx, work, unit.id, &now).await?;
    if current != r.expected_current_revision_ref {
        return Err(ReadingError::Conflict);
    }
    if !source::bytes_available(tx, &unit).await? {
        return Err(ReadingError::OriginalUnavailable);
    }
    let before = if let Some(id) = current {
        Some(
            sqlx::query(
                "SELECT * FROM linggan_material_reading WHERE reading_ref=$1 AND unit_ref=$2",
            )
            .bind(id)
            .bind(unit.id)
            .fetch_one(&mut *tx)
            .await?,
        )
    } else {
        None
    };
    let original_blocks: Vec<Value> = before
        .as_ref()
        .map(|row| serde_json::from_value(row.get("blocks")))
        .transpose()
        .map_err(|_| ReadingError::Invalid)?
        .unwrap_or_default();
    let mut issues: Vec<Value> = before
        .as_ref()
        .map(|row| serde_json::from_value(row.get("issues")))
        .transpose()
        .map_err(|_| ReadingError::Invalid)?
        .unwrap_or_default();
    let mut blocks = r.edited_blocks.clone().ok_or(ReadingError::Invalid)?;
    retain_unresolved(
        &original_blocks,
        &mut blocks,
        &mut issues,
        &r.resolved_issue_keys,
    )?;
    let id = Uuid::new_v4();
    normalize_refs(work, id, &mut blocks, &mut issues);
    validate_content(&blocks, &issues).map_err(|_| ReadingError::Invalid)?;
    let derivative: Option<Uuid> = before.as_ref().and_then(|row| row.get("derivative_ref"));
    let location: Value = if let Some(row) = &before {
        row.get("source_location")
    } else {
        let blob:Option<String>=sqlx::query_scalar("SELECT blob_sha256 FROM linggan_media_materialization WHERE materialization_ref::text=$1").bind(&unit.source).fetch_optional(&mut *tx).await?;
        json!({"slotKey":unit.slot,"sourceVersion":unit.source,"materializationRef":unit.source,"blobSha256":blob})
    };
    sqlx::query("INSERT INTO linggan_material_reading(reading_ref,unit_ref,content_public_ref,source_version,derivative_ref,producer_ref,producer_kind,rule_version,blocks,issues,source_location,based_on_reading_ref,publication,source_state,page_ordinal) VALUES($1,$2,$3,$4,$5,$6,'human_edit',$7,$8,$9,$10,$11,'human_edit',$12,$13)")
        .bind(id).bind(unit.id).bind(work).bind(&unit.source).bind(derivative).bind(r.command_ref).bind(crate::research_dossier::content::RULE_VERSION).bind(json!(blocks)).bind(json!(issues)).bind(location).bind(current).bind(&unit.state).bind(unit.page).execute(&mut *tx).await?;
    unit.current = Some(id);
    unit.adoption = "human_edited".into();
    unit.verification = if r.verified { "verified" } else { "unchecked" }.into();
    update_unit(tx, &mut unit).await?;
    record_selection(tx, &unit, "edit", Some(r.command_ref)).await?;
    let receipt = json!({"commandRef":r.command_ref,"unitRef":r.unit_ref,"action":"edit","currentRevisionRef":id,"adoption":unit.adoption,"verification":unit.verification});
    sqlx::query("INSERT INTO linggan_material_reading_command(command_ref,request_hash,unit_ref,base_reading_ref,expected_reading_ref,resulting_reading_ref,action,actor,reason,verified,response) VALUES($1,$2,$3,$4,$4,$5,'edit',$6,$7,$8,$9)")
        .bind(r.command_ref).bind(request_hash).bind(unit.id).bind(current).bind(id).bind(&r.actor).bind(&r.reason).bind(r.verified).bind(&receipt).execute(&mut *tx).await?;
    Ok(receipt)
}
fn validate(r: &ReviewReadingRequest) -> Result<(), ReadingError> {
    if r.action != "edit"
        || Uuid::parse_str(&r.source_version).is_err()
        || r.actor.trim().is_empty()
        || r.actor.chars().count() > 120
        || r.reason.trim().is_empty()
        || r.reason.chars().count() > 300
        || r.edited_blocks.is_none()
        || r.resolved_issue_keys.iter().any(|k| k.is_empty())
        || r.resolved_issue_keys.iter().collect::<BTreeSet<_>>().len()
            != r.resolved_issue_keys.len()
    {
        return Err(ReadingError::Invalid);
    }
    crate::research_dossier::content::validate_blocks(r.edited_blocks.as_ref().unwrap())
        .map_err(|_| ReadingError::Invalid)
}

fn retain_unresolved(
    before: &[Value],
    after: &mut [Value],
    issues: &mut Vec<Value>,
    resolved: &[String],
) -> Result<(), ReadingError> {
    let originals: Vec<Value> = issues
        .iter()
        .cloned()
        .chain(before.iter().flat_map(|b| {
            b["issues"]
                .as_array()
                .into_iter()
                .flatten()
                .cloned()
                .map(|mut issue| {
                    if issue.get("blockRef").is_none() {
                        issue["blockRef"] = b["ref"].clone();
                    }
                    issue
                })
        }))
        .collect();
    for key in resolved {
        let issue = originals
            .iter()
            .find(|i| issue_key(i) == Some(key.as_str()))
            .ok_or(ReadingError::Invalid)?;
        if issue["resolution"] != "reading_edit" {
            return Err(ReadingError::Invalid);
        }
    }
    protect_observed_metadata(before, after, &originals)?;
    let is_resolved = |i: &Value| issue_key(i).is_some_and(|key| resolved.iter().any(|r| r == key));
    issues.clear();
    // A client cannot clear source or unresolved reading issues by omitting their blocks.
    // Keep each original issue once; moved/deleted-block issues become material-level issues.
    for issue in originals.into_iter().filter(|i| !is_resolved(i)) {
        let block_ref = issue["blockRef"].as_str();
        let block = after
            .iter_mut()
            .find(|b| block_ref.is_some() && b["ref"].as_str() == block_ref);
        if let Some(block) = block {
            if block.get("issues").is_none() {
                block["issues"] = json!([]);
            }
            if let Some(values) = block.get_mut("issues").and_then(Value::as_array_mut) {
                if !values.contains(&issue) {
                    values.push(issue);
                }
            }
        } else {
            let mut issue = issue;
            if let Some(object) = issue.as_object_mut() {
                object.remove("blockRef");
            }
            issues.push(issue);
        }
    }
    Ok(())
}

fn protect_observed_metadata(
    before: &[Value],
    after: &mut [Value],
    issues: &[Value],
) -> Result<(), ReadingError> {
    for block in after {
        let original = before
            .iter()
            .find(|b| block.get("ref").is_some() && b["ref"] == block["ref"]);
        for field in ["bbox", "time"] {
            if block
                .get(field)
                .is_some_and(|v| original.and_then(|b| b.get(field)) != Some(v))
            {
                return Err(ReadingError::Invalid);
            }
            if let Some(source) = original.filter(|source| same_content(source, block)) {
                if let Some(value) = source.get(field) {
                    block[field] = value.clone();
                }
            } else if let Some(object) = block.as_object_mut() {
                // A reused block ID alone does not establish a new transcription's location.
                object.remove(field);
            }
        }
        if let Some(provided) = block.get("issues").and_then(Value::as_array) {
            for value in provided {
                let mut value = value.clone();
                if value.get("blockRef").is_none() {
                    value["blockRef"] = block["ref"].clone();
                }
                if !issues.contains(&value) {
                    return Err(ReadingError::Invalid);
                }
            }
        }
        if let Some(object) = block.as_object_mut() {
            object.remove("issues");
        }
    }
    Ok(())
}

fn same_content(before: &Value, after: &Value) -> bool {
    [
        "kind", "text", "items", "ordered", "start", "columns", "rows", "turns", "level",
    ]
    .iter()
    .all(|field| before.get(*field) == after.get(*field))
}

fn issue_key(issue: &Value) -> Option<&str> {
    issue["key"].as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_keeps_observed_anchors_only_when_source_content_still_matches() {
        let before = vec![
            json!({"ref":"b","kind":"paragraph","text":"不是 30 mg🙂","speaker":"unconfirmed",
            "bbox":[0.1,0.2,0.3,0.2],"time":{"startMs":42,"endMs":42}}),
        ];
        let mut unchanged = before.clone();
        unchanged[0]["speaker"] = json!("other");
        unchanged[0].as_object_mut().unwrap().remove("bbox");
        protect_observed_metadata(&before, &mut unchanged, &[]).unwrap();
        assert_eq!(unchanged[0]["bbox"], before[0]["bbox"]);
        assert_eq!(unchanged[0]["time"], before[0]["time"]);
        let mut changed = before.clone();
        changed[0]["text"] = json!("一段不同的人工转写");
        protect_observed_metadata(&before, &mut changed, &[]).unwrap();
        assert!(changed[0].get("bbox").is_none() && changed[0].get("time").is_none());
    }
    #[test]
    fn editing_cannot_clear_source_gaps_or_silently_remove_issues() {
        let before = vec![
            json!({"ref":"b","kind":"paragraph","text":"原文","issues":[{"key":"low","resolution":"reading_edit","blockRef":"b"}]}),
        ];
        let source = json!({"key":"missing_page","resolution":"source_refresh"});
        assert!(
            retain_unresolved(
                &before,
                &mut [],
                &mut vec![source.clone()],
                &["missing_page".into()]
            )
            .is_err()
        );
        let mut issues = vec![source.clone()];
        retain_unresolved(&before, &mut [], &mut issues, &[]).unwrap();
        assert_eq!(issues.len(), 2);
        retain_unresolved(&before, &mut [], &mut issues, &["low".into()]).unwrap();
        assert_eq!(issues, vec![source]);
    }
}
