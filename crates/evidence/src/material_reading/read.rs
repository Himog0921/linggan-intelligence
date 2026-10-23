use super::*;

pub async fn read_current_in(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    read_rows(conn, works, as_of, None).await
}

/// Exact immutable versions are resolved without substituting a newer current reading.
pub async fn read_versions_in(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
    readings: &[Uuid],
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    read_rows(conn, works, as_of, Some(readings)).await
}

/// A current pointer requires the same source eligibility as the canonical current reader.
pub(crate) async fn current_reference_in(
    conn: &mut PgConnection,
    work: Uuid,
    unit_ref: Uuid,
    as_of: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query("SELECT u.*,r.derivative_ref FROM linggan_material_unit u LEFT JOIN linggan_material_reading r ON r.reading_ref=u.current_reading_ref AND r.unit_ref=u.unit_ref WHERE u.content_public_ref=$1 AND u.unit_ref=$2")
        .bind(work).bind(unit_ref).fetch_one(&mut *conn).await?;
    let unit = Unit::from_row(&row);
    let derivative = row.get("derivative_ref");
    if unit.current.is_some()
        && source::unrestricted(conn, &unit, derivative, &unit.source).await?
        && source::current(
            conn,
            work,
            &unit.kind,
            unit.slot.as_deref(),
            &unit.source,
            derivative,
            as_of,
        )
        .await?
    {
        Ok(unit.current)
    } else {
        Ok(None)
    }
}

async fn read_rows(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
    readings: Option<&[Uuid]>,
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    if works.is_empty() {
        return Ok(BTreeMap::new());
    }
    let sql = if readings.is_some() {
        "SELECT u.unit_ref,u.content_public_ref,u.kind,u.slot_key,r.page_ordinal,r.source_version,r.source_state,r.reading_ref AS current_reading_ref,COALESCE(s.revision,0) AS revision,COALESCE(s.adoption,'automatic') AS adoption,COALESCE(s.verification,'unchecked') AS verification, \
        r.reading_ref,r.source_version AS reading_source_version,r.derivative_ref,r.producer_ref,r.producer_kind,r.rule_version,r.blocks,r.issues,r.source_location,r.created_at::text AS reading_created_at,CASE WHEN rc.command_ref IS NOT NULL THEN jsonb_build_object('commandRef',rc.command_ref,'actor',rc.actor,'reason',rc.reason,'action',rc.action,'reviewedAt',rc.created_at::text) END AS review \
        FROM linggan_material_unit u JOIN linggan_material_reading r ON r.unit_ref=u.unit_ref AND r.created_at<=$2::text::timestamptz AND r.reading_ref=ANY($3::uuid[]) \
        LEFT JOIN LATERAL(SELECT * FROM linggan_material_reading_selection s WHERE s.unit_ref=u.unit_ref AND s.reading_ref=r.reading_ref AND s.created_at<=$2::text::timestamptz ORDER BY s.revision DESC LIMIT 1) s ON true \
        LEFT JOIN linggan_material_reading_command rc ON rc.command_ref=s.command_ref \
        WHERE u.content_public_ref=ANY($1) ORDER BY u.content_public_ref,r.page_ordinal NULLS LAST,u.unit_key,r.created_at"
    } else {
        "SELECT u.unit_ref,u.content_public_ref,u.kind,u.slot_key,s.page_ordinal,s.source_version,s.source_state,s.reading_ref AS current_reading_ref,s.revision,s.adoption,s.verification, \
        r.reading_ref,r.source_version AS reading_source_version,r.derivative_ref,r.producer_ref,r.producer_kind,r.rule_version,r.blocks,r.issues,r.source_location,r.created_at::text AS reading_created_at,CASE WHEN rc.command_ref IS NOT NULL THEN jsonb_build_object('commandRef',rc.command_ref,'actor',rc.actor,'reason',rc.reason,'action',rc.action,'reviewedAt',rc.created_at::text) END AS review \
        FROM linggan_material_unit u JOIN LATERAL(SELECT * FROM linggan_material_reading_selection s WHERE s.unit_ref=u.unit_ref AND s.created_at<=$2::text::timestamptz ORDER BY s.revision DESC LIMIT 1) s ON true \
        LEFT JOIN linggan_material_reading r ON r.reading_ref=s.reading_ref AND r.unit_ref=u.unit_ref AND r.created_at<=$2::text::timestamptz \
        LEFT JOIN linggan_material_reading_command rc ON rc.command_ref=s.command_ref \
        WHERE u.content_public_ref=ANY($1) AND $3::uuid[] IS NULL ORDER BY u.content_public_ref,s.page_ordinal NULLS LAST,u.unit_key"
    };
    let rows = sqlx::query(sql)
        .bind(works)
        .bind(as_of)
        .bind(readings)
        .fetch_all(&mut *conn)
        .await?;
    let mut result: BTreeMap<Uuid, Vec<Value>> = works.iter().map(|w| (*w, vec![])).collect();
    for row in rows {
        let mut unit = Unit::from_row(&row);
        let reading: Option<Uuid> = row.get("reading_ref");
        let version: Option<String> = row.get("reading_source_version");
        let derivative: Option<Uuid> = row.get("derivative_ref");
        let unrestricted = source::unrestricted(
            conn,
            &unit,
            derivative,
            version.as_deref().unwrap_or(&unit.source),
        )
        .await?;
        if readings.is_some() && !unrestricted {
            continue;
        }
        if readings.is_none()
            && unrestricted
            && !source::current(
                conn,
                unit.work,
                &unit.kind,
                unit.slot.as_deref(),
                &unit.source,
                derivative,
                as_of,
            )
            .await?
        {
            continue;
        }
        if readings.is_some() {
            unit.current = reading;
            unit.source = version.unwrap_or(unit.source);
        }
        if !unrestricted {
            unit.current = None;
            unit.state = "restricted".into();
        }
        let mut material = project(&unit, &row, unrestricted && reading.is_some())?;
        if !source::bytes_available(conn, &unit).await? {
            if let Some(object) = material.as_object_mut() {
                object.remove("originalUrl");
            }
            if let Some(object) = material
                .get_mut("sourceLocation")
                .and_then(Value::as_object_mut)
            {
                object.remove("originalUrl");
            }
        }
        result.entry(unit.work).or_default().push(material);
    }
    Ok(result)
}

fn project(unit: &Unit, row: &sqlx::postgres::PgRow, readable: bool) -> Result<Value, sqlx::Error> {
    let (blocks, issues) = if readable {
        stored_content(row)?
    } else {
        (vec![], vec![])
    };
    let location: Option<Value> = if readable {
        row.get("source_location")
    } else {
        None
    };
    if location.as_ref().is_some_and(|v| !v.is_object()) {
        return Err(invalid("reading_source_location_invalid"));
    }
    let source_ref = Uuid::parse_str(&unit.source).ok();
    let mut value = json!({
        "ref":unit.id,"kind":unit.kind,"slotKey":unit.slot,"page":unit.page,
        "sourceState":unit.state,"sourceVersion":unit.source,"sourceRef":source_ref,
        "currentRevisionRef":unit.current,"revision":unit.revision,
        "derivativeRef":if readable {row.get::<Option<Uuid>,_>("derivative_ref")}else{None},
        "jobRef":if readable {row.get::<Option<Uuid>,_>("producer_ref")}else{None},"sourceLocation":location,
        "createdAt":row.get::<Option<String>,_>("reading_created_at"),
        "reading":if readable {json!({"generatedBy":row.get::<Option<String>,_>("producer_kind"),"adoption":unit.adoption,"verification":unit.verification,"ruleVersion":row.get::<Option<String>,_>("rule_version"),"review":row.get::<Option<Value>,_>("review")})}else{Value::Null},
        "blocks":blocks,"issues":issues
    });
    if let Some(url) = location.as_ref().and_then(|v| v["originalUrl"].as_str()) {
        value["originalUrl"] = json!(url);
    }
    Ok(value)
}

fn stored_content(row: &sqlx::postgres::PgRow) -> Result<(Vec<Value>, Vec<Value>), sqlx::Error> {
    let blocks: Vec<Value> = serde_json::from_value(row.try_get("blocks")?)
        .map_err(|_| invalid("reading_blocks_invalid"))?;
    let issues: Vec<Value> = serde_json::from_value(row.try_get("issues")?)
        .map_err(|_| invalid("reading_issues_invalid"))?;
    validate_content(&blocks, &issues)?;
    Ok((blocks, issues))
}

pub async fn read_candidates(db: &Database, work: Uuid) -> Result<Vec<Value>, ReadingError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let rows = candidate_rows_in(&mut tx, &[work], &now, true).await?;
    let mut candidates = vec![];
    let mut current_refs = BTreeMap::new();
    for row in rows {
        let unit = Unit::from_row(&row);
        if !source::current(
            &mut tx,
            work,
            &unit.kind,
            unit.slot.as_deref(),
            &row.get::<String, _>("candidate_source"),
            row.get("derivative_ref"),
            &now,
        )
        .await?
        {
            continue;
        }
        let (blocks, issues) = stored_content(&row)?;
        let current = if let Some(reference) = current_refs.get(&unit.id) {
            *reference
        } else {
            let reference = current_reference_in(&mut tx, work, unit.id, &now).await?;
            current_refs.insert(unit.id, reference);
            reference
        };
        let value = json!({"unitRef":unit.id,"slotKey":unit.slot,"candidateRevisionRef":row.get::<Uuid,_>("reading_ref"),"currentRevisionRef":current,"blocks":blocks,"issues":issues,"generatedBy":row.get::<String,_>("producer_kind"),"verification":"unchecked","createdAt":row.get::<String,_>("reading_created_at"),"review":row.get::<Option<Value>,_>("review")});
        candidates.push(value);
    }
    tx.commit().await?;
    Ok(candidates)
}

/// Candidate existence is metadata; default dossier reads never load the candidate text again.
pub(crate) async fn candidate_statuses_in(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    let rows = candidate_rows_in(conn, works, as_of, false).await?;
    let mut result = BTreeMap::<Uuid, Vec<Value>>::new();
    for row in rows {
        let unit = Unit::from_row(&row);
        if source::current(
            conn,
            unit.work,
            &unit.kind,
            unit.slot.as_deref(),
            &row.get::<String, _>("candidate_source"),
            row.get("derivative_ref"),
            as_of,
        )
        .await?
        {
            result.entry(unit.work).or_default().push(json!({
                "unitRef":unit.id,"kind":unit.kind,"slotKey":unit.slot,
                "sourceVersion":row.get::<String,_>("candidate_source"),
                "candidateRevisionRef":row.get::<Uuid,_>("reading_ref")}));
        }
    }
    Ok(result)
}

async fn candidate_rows_in(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
    content: bool,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        r#"
SELECT u.unit_ref,u.content_public_ref,u.kind,u.slot_key,s.page_ordinal,s.source_version,
 s.source_state,s.reading_ref AS current_reading_ref,s.revision,s.adoption,s.verification,
 r.reading_ref,r.source_version AS candidate_source,r.derivative_ref,
 CASE WHEN $3 THEN r.blocks END AS blocks,CASE WHEN $3 THEN r.issues END AS issues,
 r.producer_kind,r.created_at::text AS reading_created_at,
 CASE WHEN c.command_ref IS NOT NULL THEN jsonb_build_object('commandRef',c.command_ref,
 'actor',c.actor,'reason',c.reason,'action',c.action,'reviewedAt',c.created_at::text) END AS review
FROM linggan_material_unit u JOIN linggan_material_reading r USING(unit_ref)
JOIN LATERAL(SELECT * FROM linggan_material_reading_selection s
 WHERE s.unit_ref=u.unit_ref AND s.created_at<=$2::text::timestamptz
 ORDER BY s.revision DESC LIMIT 1) s ON true
LEFT JOIN LATERAL(SELECT * FROM linggan_material_reading_command c
 WHERE c.candidate_reading_ref=r.reading_ref AND c.created_at<=$2::text::timestamptz
 ORDER BY c.command_sequence DESC LIMIT 1) c ON true
WHERE u.content_public_ref=ANY($1) AND r.created_at<=$2::text::timestamptz
 AND r.reading_ref IS DISTINCT FROM s.reading_ref
 AND (c.action IS NULL OR c.action='pending')
 AND NOT EXISTS(SELECT 1 FROM linggan_material_reading_selection prior
 WHERE prior.reading_ref=r.reading_ref AND prior.created_at<=$2::text::timestamptz)
ORDER BY r.created_at DESC,r.reading_ref
"#,
    )
    .bind(works)
    .bind(as_of)
    .bind(content)
    .fetch_all(conn)
    .await
}
