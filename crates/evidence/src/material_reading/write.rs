use super::*;

pub async fn publish_in(
    conn: &mut PgConnection,
    mut input: NewReading,
) -> Result<PublishedReading, sqlx::Error> {
    validate_input(&input)?;
    lock_work(conn, input.content_public_ref).await?;
    let mut unit = ensure_unit(conn, &input).await?;
    let reading = existing_or_new(conn, &mut input, &unit).await?;
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *conn)
        .await?;
    let current_source = source::current(
        conn,
        unit.work,
        &unit.kind,
        unit.slot.as_deref(),
        &input.source_version,
        input.derivative_ref,
        &now,
    )
    .await?;
    let preserve_human = if unit.current.is_some() && unit.adoption != "automatic" {
        let derivative: Option<Uuid> = sqlx::query_scalar(
            "SELECT derivative_ref FROM linggan_material_reading WHERE reading_ref=$1",
        )
        .bind(unit.current)
        .fetch_one(&mut *conn)
        .await?;
        source::unrestricted(conn, &unit, derivative, &unit.source).await?
            && source::current(
                conn,
                unit.work,
                &unit.kind,
                unit.slot.as_deref(),
                &unit.source,
                derivative,
                &now,
            )
            .await?
    } else {
        false
    };
    let original_current = unit.current;
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
    let expected_matches = match input.publication {
        Publication::Automatic { expected_current } => expected_current == original_current,
    };
    if current_source
        && expected_matches
        && unit.adoption == "automatic"
        && unit.current != Some(reading)
    {
        unit.current = Some(reading);
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
        ["image", "audio", "video_frame"].contains(&input.kind.as_str())
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
            "frame_ocr",
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
        Publication::Automatic { .. } => "automatic",
    };
    if let Some(r) = previous {
        if r.get::<Value, _>("blocks") != json!(input.blocks)
            || r.get::<Value, _>("issues") != json!(input.issues)
            || r.get::<Value, _>("source_location") != input.source_location
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
