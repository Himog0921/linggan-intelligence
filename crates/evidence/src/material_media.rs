//! Media-slot qualification layered on the existing runtime media fact chain.

use crate::{material_admission, producer_runtime::ProducerRuntimeError};
use linggan_contracts::ProducerCapturePackage;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

pub(crate) fn record_disposition(
    package: &ProducerCapturePackage,
    record_ordinal: usize,
    record: &Value,
) -> Option<(&'static str, &'static str)> {
    if package.package_kind() != "media_slots" {
        return None;
    }
    let duplicated_observation = crate::material_contract_validation::duplicate_value(
        package,
        record_ordinal,
        "/observationRef",
        record,
    );
    let duplicated_slot = crate::material_contract_validation::duplicate_value(
        package,
        record_ordinal,
        "/slotKey",
        record,
    );
    Some(
        if !duplicated_observation && !duplicated_slot && media_record(package, record).is_some() {
            ("accepted_for_media_identity", "media_origin_contract_valid")
        } else if duplicated_observation {
            ("quarantined", "media_observation_identity_duplicate")
        } else if duplicated_slot {
            ("quarantined", "media_slot_identity_duplicate")
        } else {
            ("quarantined", "media_origin_contract_invalid")
        },
    )
}

pub(crate) async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    let Some(content_id) = target_content_id(package) else {
        return Ok(());
    };
    let content_public_ref = material_admission::ensure_content(tx, package, content_id).await?;
    if package.package_kind() == "media_bytes" {
        return material_admission::insert_lane_observation(
            tx,
            package,
            "media_bytes",
            Some(content_public_ref),
            None,
            0,
        )
        .await;
    }
    if package.package_kind() != "media_slots" {
        return Ok(());
    }
    let mut retained = 0;
    for (ordinal, value) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        let Some(media) = media_record(package, value) else {
            continue;
        };
        insert_slot_and_origin(tx, package, content_public_ref, ordinal, media).await?;
        retained += 1;
    }
    material_admission::insert_lane_observation(
        tx,
        package,
        "media_slots",
        Some(content_public_ref),
        None,
        retained,
    )
    .await
}

pub(crate) async fn identity_conflict_reason(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    record: &Value,
) -> Result<Option<&'static str>, ProducerRuntimeError> {
    let Some(media) = media_record(package, record) else {
        return Ok(None);
    };
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(media.slot_key)
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text,1))")
        .bind(media.observation_ref)
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let existing = sqlx::query(
        "SELECT platform,content_external_id,role,ordinal FROM linggan_media_slot WHERE slot_key=$1",
    )
    .bind(media.slot_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if existing.is_some_and(|row| {
        row.get::<String, _>("platform") != package.platform()
            || row.get::<String, _>("content_external_id") != media.content_id
            || row.get::<String, _>("role") != media.role
            || row.get::<i32, _>("ordinal") != media.producer_ordinal
    }) {
        return Ok(Some("media_slot_identity_conflict"));
    }
    let observation_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_media_observation WHERE observation_ref=$1)",
    )
    .bind(media.observation_ref)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    Ok(observation_exists.then_some("media_observation_identity_conflict"))
}

pub(crate) async fn insert_legacy_only(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
) -> Result<(), ProducerRuntimeError> {
    for record in package.records() {
        let Some(media) = legacy_media_record(record) else {
            continue;
        };
        sqlx::query("INSERT INTO linggan_media_slot (slot_key,platform,content_external_id,role,ordinal,first_package_ref) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (slot_key) DO NOTHING")
            .bind(media.slot_key).bind(package.platform()).bind(media.content_id).bind(media.role).bind(media.producer_ordinal).bind(package.package_ref())
            .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
        sqlx::query("INSERT INTO linggan_media_observation (observation_ref,slot_key,package_ref,observed_external_uri,observed_at) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (observation_ref) DO NOTHING")
            .bind(media.observation_ref).bind(media.slot_key).bind(package.package_ref()).bind(media.primary_uri).bind(package.observed_at())
            .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    }
    Ok(())
}

async fn insert_slot_and_origin(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    content_public_ref: Uuid,
    record_ordinal: usize,
    media: MediaRecord<'_>,
) -> Result<(), ProducerRuntimeError> {
    // `FOR UPDATE` cannot lock an absent row. Serialize the natural key before first insert;
    // generation is then calculated while the same transaction-level lock remains held.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(media.slot_key)
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let existing = sqlx::query("SELECT platform,content_external_id,role,ordinal FROM linggan_media_slot WHERE slot_key=$1 FOR UPDATE")
        .bind(media.slot_key).fetch_optional(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    if let Some(row) = existing {
        if row.get::<String, _>("platform") != package.platform()
            || row.get::<String, _>("content_external_id") != media.content_id
            || row.get::<String, _>("role") != media.role
            || row.get::<i32, _>("ordinal") != media.producer_ordinal
        {
            return Err(ProducerRuntimeError::MaterialIdentityConflict);
        }
    } else {
        sqlx::query("INSERT INTO linggan_media_slot (slot_key,platform,content_external_id,role,ordinal,first_package_ref) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(media.slot_key).bind(package.platform()).bind(media.content_id).bind(media.role).bind(media.producer_ordinal).bind(package.package_ref())
            .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
        sqlx::query("SELECT slot_key FROM linggan_media_slot WHERE slot_key=$1 FOR UPDATE")
            .bind(media.slot_key)
            .fetch_one(&mut **tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
    }
    let generation: i32 = sqlx::query_scalar("SELECT COALESCE(max(source_generation),0)+1 FROM linggan_material_media_origin WHERE slot_key=$1")
        .bind(media.slot_key).fetch_one(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    sqlx::query("INSERT INTO linggan_media_observation (observation_ref,slot_key,package_ref,observed_external_uri,observed_at) VALUES ($1,$2,$3,$4,$5)")
        .bind(media.observation_ref).bind(media.slot_key).bind(package.package_ref()).bind(media.primary_uri).bind(package.observed_at())
        .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    let live = media.purpose == "live_photo";
    let has_still = !media.still_candidates.is_empty();
    let has_motion = !media.motion_candidates.is_empty();
    sqlx::query("INSERT INTO linggan_material_media_origin (observation_ref,content_public_ref,slot_key,package_ref,record_ordinal,source_generation,purpose,producer_ordinal,display_ordinal,display_order_state,display_order_basis,candidate_set_state,composite_state,live_photo_still_state,live_photo_motion_state) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NULL,'UNKNOWN','producer_global_sequence_unverified','OBSERVED_SET',$9,$10,$11)")
        .bind(media.observation_ref).bind(content_public_ref).bind(media.slot_key).bind(package.package_ref())
        .bind(i32::try_from(record_ordinal).expect("package record count is bounded")).bind(generation).bind(media.purpose).bind(media.producer_ordinal)
        .bind(if live && has_still && has_motion { "COMPLETE" } else if live { "PARTIAL" } else { "NOT_APPLICABLE" })
        .bind(live.then_some(if has_still { "OBSERVED" } else { "UNKNOWN" }))
        .bind(live.then_some(if has_motion { "OBSERVED" } else { "UNKNOWN" }))
        .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    let component_candidates = if live && (has_still || has_motion) {
        media
            .still_candidates
            .iter()
            .map(|uri| ("still", *uri))
            .chain(media.motion_candidates.iter().map(|uri| ("motion", *uri)))
            .collect::<Vec<_>>()
    } else {
        media
            .candidates
            .iter()
            .map(|uri| ("single", *uri))
            .collect()
    };
    let component_column_ready: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_attribute \
         WHERE attrelid='linggan_material_media_candidate'::regclass \
           AND attname='component_kind' AND NOT attisdropped)",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if component_column_ready {
        let mut previous_component = "";
        for (index, (component_kind, uri)) in component_candidates.iter().enumerate() {
            let producer_primary = previous_component != *component_kind;
            previous_component = component_kind;
            sqlx::query("INSERT INTO linggan_material_media_candidate (candidate_ref,observation_ref,candidate_ordinal,external_uri,producer_primary,source_field,source_field_state,expires_at,expires_at_state,component_kind) VALUES ($1,$2,$3,$4,$5,NULL,'UNKNOWN',NULL,'UNKNOWN',$6)")
                .bind(Uuid::new_v4()).bind(media.observation_ref).bind(i32::try_from(index + 1).expect("candidate list is bounded"))
                .bind(uri).bind(producer_primary).bind(component_kind).execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
        }
    } else {
        for (index, uri) in media.candidates.iter().enumerate() {
            sqlx::query("INSERT INTO linggan_material_media_candidate (candidate_ref,observation_ref,candidate_ordinal,external_uri,producer_primary,source_field,source_field_state,expires_at,expires_at_state) VALUES ($1,$2,$3,$4,$5,NULL,'UNKNOWN',NULL,'UNKNOWN')")
                .bind(Uuid::new_v4()).bind(media.observation_ref).bind(i32::try_from(index + 1).expect("candidate list is bounded"))
                .bind(uri).bind(index == 0).execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
        }
    }
    let components = component_candidates
        .iter()
        .map(|(component, _)| *component)
        .collect::<HashSet<_>>();
    enqueue_media_acquisition(tx, media.observation_ref, &components).await?;
    Ok(())
}

/// Every accepted source-media observation gets bounded byte-acquisition work. Discovery covers
/// used to be the only producer; detail slots now use the same durable work table rather than a
/// second download queue.
async fn enqueue_media_acquisition(
    tx: &mut Transaction<'_, Postgres>,
    observation_ref: Uuid,
    components: &HashSet<&str>,
) -> Result<(), ProducerRuntimeError> {
    let schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_media_acquisition_work') IS NOT NULL")
            .fetch_one(&mut **tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
    if !schema_ready {
        return Ok(());
    }
    let component_column_ready: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_attribute \
         WHERE attrelid='linggan_media_acquisition_work'::regclass \
           AND attname='component_kind' AND NOT attisdropped)",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if component_column_ready {
        for component in components {
            sqlx::query(
                "INSERT INTO linggan_media_acquisition_work \
                 (work_ref,observation_ref,component_kind) VALUES ($1,$2,$3) \
                 ON CONFLICT (observation_ref,component_kind) DO NOTHING",
            )
            .bind(Uuid::new_v4())
            .bind(observation_ref)
            .bind(component)
            .execute(&mut **tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
        }
    } else {
        sqlx::query(
            "INSERT INTO linggan_media_acquisition_work (work_ref,observation_ref) \
             VALUES ($1,$2) ON CONFLICT (observation_ref) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(observation_ref)
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    }
    Ok(())
}

struct MediaRecord<'a> {
    slot_key: &'a str,
    observation_ref: Uuid,
    primary_uri: &'a str,
    candidates: Vec<&'a str>,
    still_candidates: Vec<&'a str>,
    motion_candidates: Vec<&'a str>,
    content_id: &'a str,
    role: &'a str,
    purpose: &'static str,
    producer_ordinal: i32,
}

fn legacy_media_record(record: &Value) -> Option<MediaRecord<'_>> {
    let slot_key = record.get("slotKey")?.as_str()?.trim();
    let observation_ref = Uuid::parse_str(record.get("observationRef")?.as_str()?).ok()?;
    let primary_uri = record.pointer("/observation/externalUri")?.as_str()?.trim();
    let content_id = record.pointer("/sourceObject/externalId")?.as_str()?.trim();
    let role = record.pointer("/slot/role")?.as_str()?.trim();
    let purpose = match role {
        "image" => "body_image",
        "cover" => "cover",
        "video" => "video",
        "live_photo" => "live_photo",
        _ => return None,
    };
    let producer_ordinal = record
        .pointer("/slot/ordinal")?
        .as_i64()
        .filter(|value| *value > 0)
        .and_then(|value| i32::try_from(value).ok())?;
    if slot_key.is_empty() || primary_uri.is_empty() || content_id.is_empty() {
        return None;
    }
    Some(MediaRecord {
        slot_key,
        observation_ref,
        primary_uri,
        candidates: vec![primary_uri],
        still_candidates: Vec::new(),
        motion_candidates: Vec::new(),
        content_id,
        role,
        purpose,
        producer_ordinal,
    })
}

fn media_record<'a>(
    package: &ProducerCapturePackage,
    record: &'a Value,
) -> Option<MediaRecord<'a>> {
    if record.get("kind")?.as_str()? != "media_slot" {
        return None;
    }
    let content_id = record.pointer("/sourceObject/externalId")?.as_str()?.trim();
    if Some(content_id) != target_content_id(package)
        || record.pointer("/sourceObject/platform")?.as_str()? != package.platform()
        || record.pointer("/sourceObject/type")?.as_str()? != "content"
    {
        return None;
    }
    let role = record.pointer("/slot/role")?.as_str()?.trim();
    let purpose = match role {
        "image" => "body_image",
        "cover" => "cover",
        "video" => "video",
        "live_photo" => "live_photo",
        _ => return None,
    };
    let producer_ordinal = record
        .pointer("/slot/ordinal")?
        .as_i64()
        .filter(|value| *value > 0)
        .and_then(|value| i32::try_from(value).ok())?;
    let slot_key = record.get("slotKey")?.as_str()?.trim();
    if slot_key != canonical_slot_key(package.platform(), content_id, role, producer_ordinal) {
        return None;
    }
    let observation_ref = Uuid::parse_str(record.get("observationRef")?.as_str()?).ok()?;
    let primary_uri = record.pointer("/observation/externalUri")?.as_str()?.trim();
    let candidates = record
        .pointer("/observation/candidateUris")?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .map(str::trim)
        .collect::<Vec<_>>();
    let component_candidates = |component: &str| {
        record
            .pointer(&format!(
                "/observation/components/{component}/candidateUris"
            ))
            .and_then(Value::as_array)
            .and_then(|values| {
                values
                    .iter()
                    .map(Value::as_str)
                    .collect::<Option<Vec<_>>>()
                    .map(|items| items.into_iter().map(str::trim).collect::<Vec<_>>())
            })
            .unwrap_or_default()
    };
    let still_candidates = component_candidates("still");
    let motion_candidates = component_candidates("motion");
    let component_set = still_candidates
        .iter()
        .chain(motion_candidates.iter())
        .copied()
        .collect::<HashSet<_>>();
    if primary_uri.is_empty()
        || candidates.is_empty()
        || candidates[0] != primary_uri
        || candidates.iter().any(|uri| uri.is_empty())
        || candidates.iter().copied().collect::<HashSet<_>>().len() != candidates.len()
        || (!component_set.is_empty()
            && (role != "live_photo"
                || component_set.len() != still_candidates.len() + motion_candidates.len()
                || component_set != candidates.iter().copied().collect::<HashSet<_>>()))
    {
        return None;
    }
    Some(MediaRecord {
        slot_key,
        observation_ref,
        primary_uri,
        candidates,
        still_candidates,
        motion_candidates,
        content_id,
        role,
        purpose,
        producer_ordinal,
    })
}

fn target_content_id(package: &ProducerCapturePackage) -> Option<&str> {
    package
        .coverage()
        .pointer("/target/contentExternalId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn canonical_slot_key(platform: &str, content_id: &str, role: &str, ordinal: i32) -> String {
    format!(
        "{platform}:{}:{role}:{ordinal}",
        encode_uri_component(content_id)
    )
}

fn encode_uri_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
