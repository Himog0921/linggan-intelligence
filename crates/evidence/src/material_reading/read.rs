use super::*;

type ImageSources = crate::research_dossier::DisplayedImageSources;

pub async fn read_current_in(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    if works.is_empty() {
        return Ok(BTreeMap::new());
    }
    let currents =
        crate::work_resource_current::read_work_resource_currents_in(conn, works, as_of).await?;
    let references: Vec<_> = currents.iter().collect();
    let sources =
        crate::research_dossier::displayed_image_sources_in(conn, &references, as_of).await?;
    let mut result = read_current_from_currents_in(conn, &references, as_of, &sources).await?;
    for work in works {
        result.entry(*work).or_default();
    }
    Ok(result)
}

/// Reuse the canonical body source from the caller's same-transaction Current.
/// Restriction checks and selected reading version checks are still applied below.
pub(crate) async fn read_current_from_currents_in(
    conn: &mut PgConnection,
    currents: &[&crate::work_resource_current::WorkResourceCurrent],
    as_of: &str,
    image_sources: &ImageSources,
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    let works: Vec<_> = currents.iter().map(|current| current.public_ref).collect();
    let bodies: BTreeMap<_, _> = currents
        .iter()
        .map(|current| (current.public_ref, current.body_source.material_ref))
        .collect();
    read_rows(
        conn,
        &works,
        as_of,
        None,
        Some(&bodies),
        Some(image_sources),
    )
    .await
}

/// Exact immutable versions are resolved without substituting a newer current reading.
pub async fn read_versions_in(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
    readings: &[Uuid],
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    read_rows(conn, works, as_of, Some(readings), None, None).await
}

/// A current pointer requires the same source eligibility as the canonical current reader.
pub(crate) async fn current_reference_in(
    conn: &mut PgConnection,
    work: Uuid,
    unit_ref: Uuid,
    as_of: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let rows = read_current_in(conn, &[work], as_of).await?;
    Ok(rows
        .get(&work)
        .into_iter()
        .flatten()
        .find(|row| row["ref"] == unit_ref.to_string())
        .and_then(|row| row["currentRevisionRef"].as_str())
        .and_then(|value| Uuid::parse_str(value).ok()))
}

async fn read_rows(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
    readings: Option<&[Uuid]>,
    bodies: Option<&BTreeMap<Uuid, Option<Uuid>>>,
    image_sources: Option<&ImageSources>,
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
        "WITH selected_images AS MATERIALIZED (SELECT * FROM jsonb_to_recordset($4::jsonb) AS image(work_ref uuid,slot_key text,source_version text,blob_sha256 text)) \
        SELECT u.unit_ref,u.content_public_ref,u.kind,u.slot_key,s.page_ordinal,s.source_version,s.source_state,s.reading_ref AS current_reading_ref,s.revision,s.adoption,s.verification, \
        r.reading_ref,r.source_version AS reading_source_version,r.derivative_ref,r.producer_ref,r.producer_kind,r.rule_version,r.blocks,r.issues,r.source_location,r.created_at::text AS reading_created_at,CASE WHEN rc.command_ref IS NOT NULL THEN jsonb_build_object('commandRef',rc.command_ref,'actor',rc.actor,'reason',rc.reason,'action',rc.action,'reviewedAt',rc.created_at::text) END AS review \
        FROM linggan_material_unit u JOIN LATERAL(SELECT * FROM linggan_material_reading_selection s WHERE s.unit_ref=u.unit_ref AND s.created_at<=$2::text::timestamptz \
        AND (u.kind<>'image' OR NOT EXISTS(SELECT 1 FROM selected_images i WHERE i.work_ref=u.content_public_ref AND i.slot_key=u.slot_key) \
        OR EXISTS(SELECT 1 FROM selected_images i WHERE i.work_ref=u.content_public_ref AND i.slot_key=u.slot_key AND (i.source_version=s.source_version OR EXISTS(SELECT 1 FROM linggan_media_materialization m WHERE m.materialization_ref::text=s.source_version AND m.blob_sha256=i.blob_sha256 AND m.verified_at<=$2::text::timestamptz)))) \
        ORDER BY s.revision DESC LIMIT 1) s ON true \
        LEFT JOIN linggan_material_reading r ON r.reading_ref=s.reading_ref AND r.unit_ref=u.unit_ref AND r.created_at<=$2::text::timestamptz \
        LEFT JOIN linggan_material_reading_command rc ON rc.command_ref=s.command_ref \
        WHERE u.content_public_ref=ANY($1) AND $3::uuid[] IS NULL ORDER BY u.content_public_ref,s.page_ordinal NULLS LAST,u.unit_key"
    };
    let query = sqlx::query(sql).bind(works).bind(as_of).bind(readings);
    let rows = if readings.is_some() {
        query.fetch_all(&mut *conn).await?
    } else {
        let sources: Vec<_> = image_sources.into_iter().flat_map(|images| images.iter()).map(|((work, slot), (version, sha))| {
            json!({"work_ref": work, "slot_key": slot, "source_version": version, "blob_sha256": sha})
        }).collect();
        query
            .bind(sqlx::types::Json(sources))
            .fetch_all(&mut *conn)
            .await?
    };
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
        let current = if readings.is_some() || !unrestricted {
            true
        } else if unit.kind == "body"
            && let Some(bodies) = bodies
        {
            source::body_current(bodies.get(&unit.work).copied().flatten(), &unit.source)
        } else if unit.kind == "image"
            && let Some(images) = image_sources
        {
            match unit
                .slot
                .as_deref()
                .and_then(|slot| images.get(&(unit.work, slot.to_owned())))
            {
                Some((version, _)) if version == &unit.source => true,
                Some((_, sha)) => source::has_image_blob(conn, &unit.source, sha, as_of).await?,
                None => false,
            }
        } else {
            source::current(
                conn,
                unit.work,
                &unit.kind,
                unit.slot.as_deref(),
                &unit.source,
                derivative,
                as_of,
            )
            .await?
        };
        if !current {
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
