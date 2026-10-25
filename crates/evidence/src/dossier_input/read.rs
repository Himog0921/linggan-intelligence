//! Internal planning and snapshot adapter over the sole public dossier contract.
use super::{
    Column, ColumnState, Error, FINGERPRINT_VERSION, Material, VERSION, cutoff, identity,
    validate_works,
};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub async fn read(database: &Database, domain: Uuid, work: Uuid) -> Result<Material, Error> {
    Ok(read_at(database, domain, &[work], None).await?.remove(0))
}
pub async fn read_at(
    database: &Database,
    domain: Uuid,
    works: &[Uuid],
    as_of: Option<&str>,
) -> Result<Vec<Material>, Error> {
    validate_works(works)?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let cutoff = cutoff(&mut tx, as_of).await?;
    let result = read_in(&mut tx, domain, works, &cutoff).await?;
    tx.commit().await?;
    Ok(result)
}
pub(super) async fn scope(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    works: &[Uuid],
    as_of: &str,
) -> Result<(), Error> {
    validate_works(works)?;
    let rows=sqlx::query("SELECT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=ANY($2) AND created_at<=$3::timestamptz GROUP BY content_public_ref")
        .bind(domain).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    if rows.len() != works.len() {
        return Err(Error::NotFound);
    }
    Ok(())
}
pub(super) async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    works: &[Uuid],
    as_of: &str,
) -> Result<Vec<Material>, Error> {
    scope(tx, domain, works, as_of).await?;
    let mut dossiers = crate::research_dossier::read_in(tx, works, as_of).await?;
    let mut output = Vec::new();
    for work in works {
        let dossier = dossiers.remove(work).ok_or(Error::NotFound)?;
        let fragments = dossier
            .fragments
            .iter()
            .map(|f| {
                let mut value = identity::fragment(
                    &f.field,
                    f.fragment_id.clone(),
                    f.source_ref,
                    f.source_version.clone(),
                    f.start,
                    f.text.clone(),
                );
                value.speaker = f.speaker.clone();
                value.structure = f.structure.clone();
                value.issues = f.issues.clone();
                value.reading_ref = f.reading_ref;
                value.unit_ref = f.unit_ref;
                value
            })
            .collect();
        let columns = canonical_columns(&dossier.note, &dossier.materials);
        let not_ready_reasons = columns
            .iter()
            .filter(|c| {
                matches!(
                    c.state,
                    ColumnState::Waiting | ColumnState::Restricted | ColumnState::NotAcquired
                )
            })
            .map(|c| format!("{}_not_ready", c.field))
            .collect();
        let mut input = Material {
            contract: VERSION.into(),
            fingerprint_contract: FINGERPRINT_VERSION.into(),
            domain_ref: domain,
            work_ref: *work,
            as_of: as_of.into(),
            reading_title: crate::research_dossier::reading_title(&dossier),
            dossier_revision: dossier.revision,
            material_fingerprint: String::new(),
            provenance_hash: String::new(),
            snapshot_identity: String::new(),
            ready: false,
            not_ready_reasons,
            columns,
            fragments,
            layer_version: crate::research_dossier::content::RULE_VERSION.into(),
            processing_scope: super::ProcessingScope {
                included: vec![
                    "platform".into(),
                    "image_ocr".into(),
                    "asr".into(),
                    "video_frame_ocr".into(),
                ],
                context_only: vec!["comment_image".into(), "author_avatar".into()],
                not_connected: vec![],
            },
        };
        identity::finish(&mut input);
        output.push(input);
    }
    Ok(output)
}
pub(super) async fn lock_work(tx: &mut Transaction<'_, Postgres>, work: Uuid) -> Result<(), Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(420,hashtext($1::uuid::text))")
        .bind(work)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub(super) async fn check_revision(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    expected: &str,
) -> Result<(), Error> {
    let now = cutoff(tx, None).await?;
    let value = crate::research_dossier::read_in(tx, &[work], &now)
        .await?
        .remove(&work)
        .ok_or(Error::NotFound)?;
    if value.revision != expected {
        return Err(Error::Conflict);
    }
    Ok(())
}

fn canonical_columns(note: &serde_json::Value, materials: &[serde_json::Value]) -> Vec<Column> {
    let mut columns = vec![Column {
        field: "title".into(),
        state: if note["displayTitle"].is_string() {
            ColumnState::Available
        } else {
            ColumnState::NotProvided
        },
        reasons: vec![],
    }];
    for (field, kind) in [
        ("body", "body"),
        ("tags", "tags"),
        ("ocr", "image"),
        ("transcript", "audio"),
        ("frame_text", "video_frame"),
    ] {
        let units: Vec<_> = materials
            .iter()
            .filter(|unit| unit["kind"] == kind)
            .collect();
        let state = if units.iter().any(|unit| unit["sourceState"] == "restricted") {
            ColumnState::Restricted
        } else if units
            .iter()
            .any(|unit| unit["sourceState"] == "present" && !unit["reading"].is_object())
        {
            ColumnState::Waiting
        } else if units.iter().any(|unit| {
            !matches!(
                unit["sourceState"].as_str(),
                Some("present" | "known_empty" | "not_applicable")
            )
        }) {
            ColumnState::NotAcquired
        } else if units.iter().any(|unit| unit["sourceState"] == "present") {
            ColumnState::Available
        } else if units
            .iter()
            .any(|unit| unit["sourceState"] == "known_empty")
        {
            ColumnState::KnownEmpty
        } else {
            ColumnState::NotProvided
        };
        let reasons = units
            .iter()
            .flat_map(|unit| unit["issues"].as_array().into_iter().flatten())
            .filter_map(|issue| issue["code"].as_str().map(str::to_owned))
            .collect();
        columns.push(Column {
            field: field.into(),
            state,
            reasons,
        });
    }
    columns
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn multiple_image_units_have_one_column_and_never_hide_an_unread_or_missing_page() {
        let note = json!({"platformTitle":"标题"});
        let mut materials = vec![
            json!({"kind":"image","sourceState":"present","reading":{}}),
            json!({"kind":"image","sourceState":"known_empty"}),
        ];
        let fields =
            |columns: Vec<Column>| columns.into_iter().map(|c| c.field).collect::<Vec<_>>();
        assert_eq!(
            fields(canonical_columns(&note, &materials)),
            ["title", "body", "tags", "ocr", "transcript", "frame_text"]
        );
        assert_eq!(
            canonical_columns(&note, &materials)[3].state,
            ColumnState::Available
        );
        materials.push(json!({"kind":"image","sourceState":"present","reading":null}));
        assert_eq!(
            canonical_columns(&note, &materials)[3].state,
            ColumnState::Waiting
        );
        materials[2] =
            json!({"kind":"image","sourceState":"not_acquired","issues":[{"code":"missing_page"}]});
        assert_eq!(
            canonical_columns(&note, &materials)[3].state,
            ColumnState::NotAcquired
        );
        assert_eq!(
            canonical_columns(&note, &materials)[3].reasons,
            ["missing_page"]
        );
        materials[2] = json!({"kind":"image","sourceState":"restricted"});
        assert_eq!(
            canonical_columns(&note, &materials)[3].state,
            ColumnState::Restricted
        );
    }
}
