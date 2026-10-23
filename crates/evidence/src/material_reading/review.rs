use super::*;
use std::collections::BTreeSet;

pub async fn review_reading(
    db: &Database,
    unit_ref: Uuid,
    request: &ReviewReadingRequest,
) -> Result<Value, ReadingError> {
    validate(request)?;
    let request_hash =
        crate::ocr_adoption::content_hash(json!([unit_ref, request]).to_string().as_bytes());
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(421,hashtext($1::text))")
        .bind(request.command_ref.to_string())
        .execute(&mut *tx)
        .await?;
    let replay = sqlx::query(
        "SELECT request_hash,response FROM linggan_material_reading_command WHERE command_ref=$1",
    )
    .bind(request.command_ref)
    .fetch_optional(&mut *tx)
    .await?;
    if replay
        .as_ref()
        .is_some_and(|r| r.get::<String, _>("request_hash") != request_hash)
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
    lock_work(&mut tx, work).await?;
    let row = sqlx::query("SELECT * FROM linggan_material_unit WHERE unit_ref=$1 FOR UPDATE")
        .bind(unit_ref)
        .fetch_one(&mut *tx)
        .await?;
    let mut unit = Unit::from_row(&row);
    let candidate =
        sqlx::query("SELECT * FROM linggan_material_reading WHERE reading_ref=$1 AND unit_ref=$2")
            .bind(request.candidate_revision_ref)
            .bind(unit.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ReadingError::NotFound)?;
    // Current source eligibility also gates retries; a receipt cannot bypass withdrawal.
    require_current_source(&mut tx, &unit, &candidate).await?;
    if let Some(replay) = replay {
        return Ok(replay.get("response"));
    }
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let effective_current = read::current_reference_in(&mut tx, work, unit.id, &now).await?;
    if effective_current != request.expected_current_revision_ref {
        return Err(ReadingError::Conflict);
    }
    if !source::bytes_available(&mut tx, &unit).await? {
        return Err(ReadingError::OriginalUnavailable);
    }
    if request.action == "keep" {
        require_current_reading(&mut tx, &unit).await?;
    }
    apply(&mut tx, &mut unit, request, &candidate).await?;
    // Opinions preserve the stored selection, including an ineligible historical pointer.
    let resulting = if matches!(request.action.as_str(), "pending" | "reject") {
        effective_current
    } else {
        unit.current
    };
    let receipt = json!({"commandRef":request.command_ref,"unitRef":unit.id,
        "candidateRevisionRef":request.candidate_revision_ref,"action":request.action,
        "currentRevisionRef":resulting,
        "adoption":resulting.map(|_| &unit.adoption),
        "verification":resulting.map(|_| &unit.verification)});
    sqlx::query("INSERT INTO linggan_material_reading_command(command_ref,request_hash,unit_ref,candidate_reading_ref,expected_reading_ref,resulting_reading_ref,action,actor,reason,verified,response) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(request.command_ref).bind(request_hash).bind(unit.id).bind(request.candidate_revision_ref)
        .bind(request.expected_current_revision_ref).bind(resulting).bind(&request.action)
        .bind(&request.actor).bind(&request.reason).bind(request.verified).bind(&receipt)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(receipt)
}

async fn require_current_source(
    conn: &mut PgConnection,
    unit: &Unit,
    reading: &sqlx::postgres::PgRow,
) -> Result<(), ReadingError> {
    let blocks: Vec<Value> =
        serde_json::from_value(reading.get("blocks")).map_err(|_| ReadingError::Invalid)?;
    let issues: Vec<Value> =
        serde_json::from_value(reading.get("issues")).map_err(|_| ReadingError::Invalid)?;
    validate_content(&blocks, &issues).map_err(|_| ReadingError::Invalid)?;
    let version: String = reading.get("source_version");
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *conn)
        .await?;
    if version != unit.source
        || !source::current(
            conn,
            unit.work,
            &unit.kind,
            unit.slot.as_deref(),
            &version,
            reading.get("derivative_ref"),
            &now,
        )
        .await?
    {
        return Err(ReadingError::SourceChanged);
    }
    Ok(())
}

async fn require_current_reading(conn: &mut PgConnection, unit: &Unit) -> Result<(), ReadingError> {
    let current = unit.current.ok_or(ReadingError::Invalid)?;
    let reading =
        sqlx::query("SELECT * FROM linggan_material_reading WHERE reading_ref=$1 AND unit_ref=$2")
            .bind(current)
            .bind(unit.id)
            .fetch_one(&mut *conn)
            .await?;
    require_current_source(conn, unit, &reading).await
}

fn validate(r: &ReviewReadingRequest) -> Result<(), ReadingError> {
    if !["adopt", "edit", "keep", "pending", "reject"].contains(&r.action.as_str())
        || r.actor.trim().is_empty()
        || r.actor.chars().count() > 120
        || r.reason.trim().is_empty()
        || r.reason.chars().count() > 300
        || (["pending", "reject"].contains(&r.action.as_str()) && r.verified)
        || (r.action == "edit") != r.edited_blocks.is_some()
        || (r.action != "edit" && !r.resolved_issue_keys.is_empty())
        || r.resolved_issue_keys.iter().any(|k| k.is_empty())
        || r.resolved_issue_keys.iter().collect::<BTreeSet<_>>().len()
            != r.resolved_issue_keys.len()
    {
        return Err(ReadingError::Invalid);
    }
    if let Some(blocks) = &r.edited_blocks {
        crate::research_dossier::content::validate_blocks(blocks)
            .map_err(|_| ReadingError::Invalid)?;
    }
    Ok(())
}

async fn apply(
    conn: &mut PgConnection,
    unit: &mut Unit,
    r: &ReviewReadingRequest,
    candidate: &sqlx::postgres::PgRow,
) -> Result<(), ReadingError> {
    match r.action.as_str() {
        "adopt" => {
            unit.current = Some(r.candidate_revision_ref);
            unit.state = candidate.get("source_state");
            unit.page = candidate.get("page_ordinal");
            unit.adoption = "human_adopted".into();
        }
        "edit" => {
            unit.current = Some(insert_edit(conn, unit, r, candidate).await?);
            unit.state = candidate.get("source_state");
            unit.page = candidate.get("page_ordinal");
            unit.adoption = "human_edited".into();
        }
        "keep" => {
            if unit.current.is_none() && r.verified {
                return Err(ReadingError::Invalid);
            }
            if unit.current.is_some() && unit.adoption == "automatic" {
                unit.adoption = "human_adopted".into();
            }
        }
        "pending" | "reject" => return Ok(()),
        _ => return Err(ReadingError::Invalid),
    }
    unit.verification = if r.verified { "verified" } else { "unchecked" }.into();
    update_unit(conn, unit).await?;
    record_selection(conn, unit, &r.action, Some(r.command_ref)).await?;
    Ok(())
}

async fn insert_edit(
    conn: &mut PgConnection,
    unit: &Unit,
    r: &ReviewReadingRequest,
    candidate: &sqlx::postgres::PgRow,
) -> Result<Uuid, ReadingError> {
    let id = Uuid::new_v4();
    let mut blocks = r.edited_blocks.clone().ok_or(ReadingError::Invalid)?;
    let before: Vec<Value> =
        serde_json::from_value(candidate.get("blocks")).map_err(|_| ReadingError::Invalid)?;
    let mut issues: Vec<Value> =
        serde_json::from_value(candidate.get("issues")).map_err(|_| ReadingError::Invalid)?;
    retain_unresolved(&before, &mut blocks, &mut issues, &r.resolved_issue_keys)?;
    normalize_refs(unit.work, id, &mut blocks, &mut issues);
    validate_content(&blocks, &issues).map_err(|_| ReadingError::Invalid)?;
    sqlx::query("INSERT INTO linggan_material_reading(reading_ref,unit_ref,content_public_ref,source_version,derivative_ref,producer_ref,producer_kind,rule_version,blocks,issues,source_location,based_on_reading_ref,publication,source_state,page_ordinal) VALUES($1,$2,$3,$4,$5,$6,'human_edit',$7,$8,$9,$10,$11,'human_edit',$12,$13)")
        .bind(id).bind(unit.id).bind(unit.work).bind(&unit.source).bind(candidate.get::<Option<Uuid>,_>("derivative_ref"))
        .bind(r.command_ref).bind(crate::research_dossier::content::RULE_VERSION).bind(json!(blocks)).bind(json!(issues))
        .bind(candidate.get::<Value,_>("source_location")).bind(r.candidate_revision_ref)
        .bind(candidate.get::<String,_>("source_state")).bind(candidate.get::<Option<i32>,_>("page_ordinal")).execute(conn).await?;
    Ok(id)
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
