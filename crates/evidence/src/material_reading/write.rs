use super::*;

const DEFERRED_IMAGE: &str = "deferredImageSelection";

struct ImageContext<'a> {
    as_of: &'a str,
    sources: &'a crate::research_dossier::DisplayedImageSources,
}

async fn source_current(
    conn: &mut PgConnection,
    unit: &Unit,
    version: &str,
    derivative: Option<Uuid>,
    as_of: &str,
    images: Option<&ImageContext<'_>>,
) -> Result<bool, sqlx::Error> {
    if unit.kind == "image" {
        if let (Some(images), Some(slot)) = (images, unit.slot.as_deref()) {
            return source::image_current(
                conn,
                slot,
                version,
                derivative,
                as_of,
                images.sources.get(&(unit.work, slot.to_owned())),
            )
            .await;
        }
    }
    source::current(
        conn,
        unit.work,
        &unit.kind,
        unit.slot.as_deref(),
        version,
        derivative,
        as_of,
    )
    .await
}

/// Complete previously recorded selections when their image set becomes displayable.
pub(crate) async fn activate_deferred_images_in(
    conn: &mut PgConnection,
    work: Uuid,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query("SELECT u.slot_key,u.source_version AS unit_source,u.current_reading_ref AS unit_current,r.reading_ref,r.source_version,r.source_state,r.page_ordinal,r.derivative_ref,r.producer_ref,r.producer_kind,r.rule_version,r.blocks,r.issues,r.source_location FROM linggan_material_unit u JOIN linggan_material_reading r USING(unit_ref) WHERE u.content_public_ref=$1 AND u.kind='image' AND r.created_at<=$2::text::timestamptz AND r.source_location ? 'deferredImageSelection' AND NOT EXISTS(SELECT 1 FROM linggan_material_reading_selection s WHERE s.reading_ref=r.reading_ref) ORDER BY r.created_at DESC,r.reading_ref DESC")
        .bind(work).bind(as_of).fetch_all(&mut *conn).await?;
    if rows.is_empty() {
        return Ok(());
    }
    let currents =
        crate::work_resource_current::read_work_resource_currents_in(conn, &[work], as_of).await?;
    let sources = crate::research_dossier::displayed_image_sources_in(
        conn,
        &currents.iter().collect::<Vec<_>>(),
        as_of,
    )
    .await?;
    let image_context = ImageContext {
        as_of,
        sources: &sources,
    };
    let mut attempted = std::collections::BTreeSet::new();
    for row in rows {
        let Some(slot) = row.get::<Option<String>, _>("slot_key") else {
            continue;
        };
        if attempted.contains(&slot) {
            continue;
        }
        let version: String = row.get("source_version");
        let selected = sources.get(&(work, slot.clone()));
        if !source::image_current(
            conn,
            &slot,
            &version,
            row.get("derivative_ref"),
            as_of,
            selected,
        )
        .await?
        {
            continue;
        }
        let location: Value = row.get("source_location");
        let marker = &location[DEFERRED_IMAGE];
        let Ok(mut expected) =
            serde_json::from_value::<Option<Uuid>>(marker["expectedCurrent"].clone())
        else {
            continue;
        };
        let Some(requested) = marker["requested"].as_bool() else {
            continue;
        };
        let current_source: String = row.get("unit_source");
        if marker["baseSource"] == current_source && current_source != version {
            if let Some((_, sha)) = selected {
                if !source::has_image_blob(conn, &current_source, sha, as_of).await? {
                    // An edit to different old bytes is preserved in history and does not invalidate the new image.
                    expected = row.get("unit_current");
                }
            }
        }
        attempted.insert(slot.clone());
        let blocks = serde_json::from_value(row.get("blocks"))
            .map_err(|_| invalid("deferred_image_blocks_invalid"))?;
        let issues = serde_json::from_value(row.get("issues"))
            .map_err(|_| invalid("deferred_image_issues_invalid"))?;
        publish(
            conn,
            NewReading {
                content_public_ref: work,
                unit_key: format!("image:{slot}"),
                kind: "image".into(),
                slot_key: Some(slot),
                page: row.get("page_ordinal"),
                source_version: version,
                source_state: row.get("source_state"),
                derivative_ref: row.get("derivative_ref"),
                producer_ref: row.get("producer_ref"),
                generated_by: row.get("producer_kind"),
                rule_version: row.get("rule_version"),
                blocks,
                issues,
                source_location: location,
                publication: if requested {
                    Publication::Requested {
                        expected_current: expected,
                    }
                } else {
                    Publication::Automatic {
                        expected_current: expected,
                    }
                },
            },
            Some(row.get("reading_ref")),
            Some(&image_context),
        )
        .await?;
    }
    Ok(())
}

pub async fn publish_in(
    conn: &mut PgConnection,
    input: NewReading,
) -> Result<PublishedReading, sqlx::Error> {
    publish(conn, input, None, None).await
}

async fn publish(
    conn: &mut PgConnection,
    mut input: NewReading,
    saved_reading: Option<Uuid>,
    images: Option<&ImageContext<'_>>,
) -> Result<PublishedReading, sqlx::Error> {
    validate_input(&input)?;
    lock_work(conn, input.content_public_ref).await?;
    let mut unit = ensure_unit(conn, &input).await?;
    let now: String = if let Some(images) = images {
        images.as_of.to_owned()
    } else {
        sqlx::query_scalar("SELECT scope_001_now()::text")
            .fetch_one(&mut *conn)
            .await?
    };
    let current_source = source_current(
        conn,
        &unit,
        &input.source_version,
        input.derivative_ref,
        &now,
        images,
    )
    .await?;
    let original_current = unit.current;
    let expected_current = match input.publication {
        Publication::Automatic { expected_current }
        | Publication::Requested { expected_current } => expected_current,
    };
    let mut expected_matches = expected_current == original_current;
    if !expected_matches
        && current_source
        && unit.kind == "image"
        && unit.source != input.source_version
    {
        if let Some(slot) = unit.slot.as_deref() {
            let selected = if let Some(images) = images {
                images.sources.get(&(unit.work, slot.to_owned())).cloned()
            } else {
                crate::research_dossier::displayed_image_source_in(conn, unit.work, slot, &now)
                    .await?
            };
            if let Some((_, sha)) = selected {
                // A change to old, different bytes must not suppress the new source.
                expected_matches = !source::has_image_blob(conn, &unit.source, &sha, &now).await?;
            }
        }
    }
    let requested = matches!(input.publication, Publication::Requested { .. }) && expected_matches;
    if !current_source
        && unit.kind == "image"
        && input.source_location.get(DEFERRED_IMAGE).is_none()
    {
        if let Some(slot) = unit.slot.as_deref() {
            if crate::research_dossier::pending_image_source_in(
                conn,
                unit.work,
                slot,
                &input.source_version,
                &now,
            )
            .await?
            {
                input.source_location[DEFERRED_IMAGE] = json!({"expectedCurrent":expected_current,"baseSource":unit.source,"requested":matches!(input.publication, Publication::Requested { .. })});
            }
        }
    }
    let reading = if let Some(reading) = saved_reading {
        reading
    } else {
        existing_or_new(conn, &mut input, &unit).await?
    };
    let preserve_human = if !requested && unit.current.is_some() && unit.adoption != "automatic" {
        let derivative: Option<Uuid> = sqlx::query_scalar(
            "SELECT derivative_ref FROM linggan_material_reading WHERE reading_ref=$1",
        )
        .bind(unit.current)
        .fetch_one(&mut *conn)
        .await?;
        source::unrestricted(conn, &unit, derivative, &unit.source).await?
            && source_current(conn, &unit, &unit.source, derivative, &now, images).await?
    } else {
        false
    };
    if current_source && unit.source != input.source_version && !preserve_human {
        unit.source.clone_from(&input.source_version);
        unit.state.clone_from(&input.source_state);
        unit.page = input.page;
        unit.current = None;
        unit.adoption = "automatic".into();
        unit.verification = "unchecked".into();
        update_unit(conn, &mut unit).await?;
        record_selection(conn, &unit, "source_changed", None).await?;
    }
    if current_source
        && expected_matches
        && (unit.adoption == "automatic" || requested)
        && unit.current != Some(reading)
    {
        unit.current = Some(reading);
        unit.adoption = "automatic".into();
        unit.state = input.source_state;
        unit.page = input.page;
        unit.verification = "unchecked".into();
        update_unit(conn, &mut unit).await?;
        record_selection(conn, &unit, "automatic", None).await?;
    }
    Ok(PublishedReading {
        unit_ref: unit.id,
        reading_ref: reading,
        current: unit.current == Some(reading),
    })
}

fn validate_input(input: &NewReading) -> Result<(), sqlx::Error> {
    let identity_valid = if input.kind == "body" {
        input.slot_key.is_none() && input.unit_key == "body"
    } else {
        ["image", "audio"].contains(&input.kind.as_str())
            && input
                .slot_key
                .as_ref()
                .is_some_and(|s| !s.is_empty() && input.unit_key == format!("{}:{s}", input.kind))
    };
    if !identity_valid
        || Uuid::parse_str(&input.source_version).is_err()
        || input.rule_version.is_empty()
        || input.page.is_some_and(|p| p <= 0)
        || !input.source_location.is_object()
        || ![
            "present",
            "known_empty",
            "not_acquired",
            "not_applicable",
            "restricted",
        ]
        .contains(&input.source_state.as_str())
        || ![
            "platform",
            "ordinary_ocr",
            "local_vision",
            "cloud_vision",
            "asr",
        ]
        .contains(&input.generated_by.as_str())
        || input.issues.iter().any(|i| !i.is_object())
    {
        return Err(invalid("material_reading_input_invalid"));
    }
    validate_content(&input.blocks, &input.issues)
}

pub(super) async fn ensure_unit(
    conn: &mut PgConnection,
    input: &NewReading,
) -> Result<Unit, sqlx::Error> {
    let id = Uuid::new_v4();
    let inserted = sqlx::query("INSERT INTO linggan_material_unit(unit_ref,content_public_ref,unit_key,kind,slot_key,page_ordinal,source_version,source_state) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(content_public_ref,unit_key) DO NOTHING")
        .bind(id).bind(input.content_public_ref).bind(&input.unit_key).bind(&input.kind).bind(&input.slot_key)
        .bind(input.page).bind(&input.source_version).bind(&input.source_state).execute(&mut *conn).await?.rows_affected()==1;
    let row = sqlx::query("SELECT * FROM linggan_material_unit WHERE content_public_ref=$1 AND unit_key=$2 FOR UPDATE")
        .bind(input.content_public_ref).bind(&input.unit_key).fetch_one(&mut *conn).await?;
    let unit = Unit::from_row(&row);
    if inserted {
        record_selection(conn, &unit, "initialization", None).await?
    }
    Ok(unit)
}

async fn existing_or_new(
    conn: &mut PgConnection,
    input: &mut NewReading,
    unit: &Unit,
) -> Result<Uuid, sqlx::Error> {
    let previous = sqlx::query("SELECT reading_ref,blocks,issues,source_location,producer_kind,publication,source_state,page_ordinal,derivative_ref FROM linggan_material_reading WHERE unit_ref=$1 AND source_version=$2 AND producer_ref=$3 AND rule_version=$4 AND publication<>'human_edit'")
        .bind(unit.id).bind(&input.source_version).bind(input.producer_ref).bind(&input.rule_version)
        .fetch_optional(&mut *conn).await?;
    let id = previous
        .as_ref()
        .map_or_else(Uuid::new_v4, |r| r.get("reading_ref"));
    normalize_refs(unit.work, id, &mut input.blocks, &mut input.issues);
    validate_content(&input.blocks, &input.issues)?;
    let publication = match input.publication {
        Publication::Automatic { .. } | Publication::Requested { .. } => "automatic",
    };
    if let Some(r) = previous {
        let previous_location: Value = r.get("source_location");
        if input.source_location.get(DEFERRED_IMAGE).is_none() {
            if let Some(deferred) = previous_location.get(DEFERRED_IMAGE) {
                input.source_location[DEFERRED_IMAGE] = deferred.clone();
            }
        }
        if r.get::<Value, _>("blocks") != json!(input.blocks)
            || r.get::<Value, _>("issues") != json!(input.issues)
            || previous_location != input.source_location
            || r.get::<String, _>("producer_kind") != input.generated_by
            || r.get::<String, _>("publication") != publication
            || r.get::<String, _>("source_state") != input.source_state
            || r.get::<Option<i32>, _>("page_ordinal") != input.page
            || r.get::<Option<Uuid>, _>("derivative_ref") != input.derivative_ref
        {
            return Err(invalid("material_reading_producer_replay_conflict"));
        }
        return Ok(id);
    }
    sqlx::query("INSERT INTO linggan_material_reading(reading_ref,unit_ref,content_public_ref,source_version,derivative_ref,producer_ref,producer_kind,rule_version,blocks,issues,source_location,publication,source_state,page_ordinal) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
        .bind(id).bind(unit.id).bind(unit.work).bind(&input.source_version).bind(input.derivative_ref).bind(input.producer_ref)
        .bind(&input.generated_by).bind(&input.rule_version).bind(json!(input.blocks)).bind(json!(input.issues))
        .bind(&input.source_location).bind(publication).bind(&input.source_state).bind(input.page).execute(conn).await?;
    Ok(id)
}
