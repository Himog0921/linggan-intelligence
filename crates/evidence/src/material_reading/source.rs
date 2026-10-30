use super::*;

pub(super) fn body_current(material: Option<Uuid>, version: &str) -> bool {
    material.is_some_and(|reference| reference.to_string() == version)
}

pub(super) async fn current(
    conn: &mut PgConnection,
    work: Uuid,
    kind: &str,
    slot: Option<&str>,
    version: &str,
    derivative: Option<Uuid>,
    as_of: &str,
) -> Result<bool, sqlx::Error> {
    if kind == "body" {
        let owners =
            crate::work_resource_current::read_work_resource_currents_in(conn, &[work], as_of)
                .await?;
        return Ok(body_current(
            owners
                .iter()
                .find(|o| o.public_ref == work)
                .and_then(|o| o.body_source.material_ref),
            version,
        ));
    }
    let Some(slot) = slot else { return Ok(false) };
    let Ok(materialization) = Uuid::parse_str(version) else {
        return Ok(false);
    };
    sqlx::query_scalar(
        "WITH origin AS (SELECT o.* FROM linggan_material_media_origin o JOIN linggan_runtime_capture_package p USING(package_ref) JOIN linggan_runtime_record_disposition d ON d.package_ref=o.package_ref AND d.record_ordinal=o.record_ordinal WHERE o.content_public_ref=$1 AND o.slot_key=$2 AND o.created_at<=$5::text::timestamptz AND p.accepted_at<=$5::text::timestamptz AND d.disposition='accepted_for_media_identity' ORDER BY o.source_generation DESC LIMIT 1), \
         current_materialization AS (SELECT m2.blob_sha256 FROM origin o JOIN linggan_media_download_attempt a2 ON a2.media_observation_ref=o.observation_ref JOIN linggan_media_materialization m2 ON m2.download_attempt_ref=a2.download_attempt_ref JOIN linggan_media_blob b2 ON b2.sha256=m2.blob_sha256 WHERE (($6='image' AND b2.mime_type LIKE 'image/%') OR ($6='video_frame' AND b2.mime_type LIKE 'video/%') OR ($6='audio' AND (b2.mime_type LIKE 'video/%' OR (o.purpose<>'live_photo' AND b2.mime_type LIKE 'audio/%')))) AND m2.verified_at<=$5::text::timestamptz ORDER BY m2.verified_at DESC,m2.materialization_ref DESC LIMIT 1) \
         SELECT linggan_dossier_slot_active($1,$2,$5::text::timestamptz) AND EXISTS(SELECT 1 FROM current_materialization current JOIN linggan_media_materialization m ON m.materialization_ref=$3 AND m.blob_sha256=current.blob_sha256 WHERE m.verified_at<=$5::text::timestamptz \
         AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d WHERE d.state='WITHDRAWN_OR_RESTRICTED' AND (d.slot_key=$2 OR d.blob_sha256=m.blob_sha256 OR d.materialization_ref=m.materialization_ref OR d.derivative_ref=$4)) \
         AND NOT EXISTS(SELECT 1 FROM linggan_media_derivative d JOIN linggan_media_ocr_retirement r ON r.retired_job_ref=d.job_ref WHERE d.derivative_ref=$4))")
        .bind(work).bind(slot).bind(materialization).bind(derivative).bind(as_of).bind(kind)
        .fetch_one(conn).await
}

pub(super) async fn unrestricted(
    conn: &mut PgConnection,
    unit: &Unit,
    derivative: Option<Uuid>,
    version: &str,
) -> Result<bool, sqlx::Error> {
    if unit.kind == "body" {
        return sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_content_detail d JOIN linggan_runtime_record_disposition r ON r.package_ref=d.package_ref AND r.record_ordinal=d.record_ordinal WHERE d.content_public_ref=$1 AND d.material_ref::text=$2 AND r.disposition='accepted_for_library_content')")
            .bind(unit.work).bind(version).fetch_one(conn).await;
    }
    sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d \
        WHERE d.state='WITHDRAWN_OR_RESTRICTED' AND (d.slot_key=$1 OR d.derivative_ref=$2 \
        OR d.materialization_ref::text=$3 OR d.blob_sha256=(SELECT blob_sha256 FROM linggan_media_materialization WHERE materialization_ref::text=$3))) \
        AND NOT EXISTS(SELECT 1 FROM linggan_media_derivative d JOIN linggan_media_ocr_retirement r ON r.retired_job_ref=d.job_ref WHERE d.derivative_ref=$2)")
        .bind(&unit.slot).bind(derivative).bind(version).fetch_one(conn).await
}

pub(super) async fn bytes_available(
    conn: &mut PgConnection,
    unit: &Unit,
) -> Result<bool, sqlx::Error> {
    if unit.kind == "body" {
        return Ok(true);
    }
    sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d WHERE d.materialization_ref::text=$1 AND d.state='BYTES_CLEANED')")
        .bind(&unit.source).fetch_one(conn).await
}
