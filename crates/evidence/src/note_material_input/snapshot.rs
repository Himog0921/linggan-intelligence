use super::{Error, Material, cutoff, read};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReceipt {
    pub snapshot_ref: Uuid,
    pub inserted: bool,
    pub snapshot_identity: String,
    pub material_fingerprint: String,
    pub provenance_hash: String,
    pub ready: bool,
    pub fragment_count: usize,
}

/// Explicit append only. GET never invokes this function or persists source text.
pub async fn save_snapshot(
    database: &Database,
    domain: Uuid,
    work: Uuid,
    as_of: &str,
    expected_revision: &str,
) -> Result<SnapshotReceipt, Error> {
    save_planned_snapshot(database, domain, work, as_of, expected_revision, None).await
}

pub(super) async fn save_planned_snapshot(
    database: &Database,
    domain: Uuid,
    work: Uuid,
    as_of: &str,
    expected_revision: &str,
    expected_fingerprint: Option<&str>,
) -> Result<SnapshotReceipt, Error> {
    let mut tx = database.pool().begin().await?;
    read::lock_work(&mut tx, work).await?;
    let as_of = cutoff(&mut tx, Some(as_of)).await?;
    read::scope(&mut tx, domain, &[work], &as_of).await?;
    read::check_revision(&mut tx, work, expected_revision).await?;
    let input = read::read_in(&mut tx, domain, &[work], &as_of)
        .await?
        .remove(0);
    if expected_fingerprint.is_some_and(|expected| expected != input.material_fingerprint) {
        return Err(Error::Conflict);
    }
    let receipt = insert(&mut tx, &input).await?;
    tx.commit().await?;
    Ok(receipt)
}

async fn insert(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &Material,
) -> Result<SnapshotReceipt, Error> {
    let candidate = Uuid::new_v4();
    let inserted:Option<Uuid>=sqlx::query_scalar(r#"
INSERT INTO linggan_note_material_snapshot(snapshot_ref,domain_ref,content_public_ref,snapshot_identity,
material_fingerprint,provenance_hash,input_contract,fingerprint_contract,dossier_contract,layer_version,as_of,ready,not_ready_reasons,columns,fragment_count)
VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::timestamptz,$12,$13,$14,$15)
ON CONFLICT(domain_ref,content_public_ref,snapshot_identity) DO NOTHING RETURNING snapshot_ref
"#).bind(candidate).bind(input.domain_ref).bind(input.work_ref).bind(&input.snapshot_identity)
.bind(&input.material_fingerprint).bind(&input.provenance_hash).bind(&input.contract).bind(&input.fingerprint_contract)
.bind(crate::research_dossier::VERSION).bind(&input.layer_version).bind(&input.as_of).bind(input.ready)
.bind(json!(input.not_ready_reasons)).bind(json!(input.columns)).bind(i32::try_from(input.fragments.len()).map_err(|_|Error::Invalid("too_many_fragments"))?)
.fetch_optional(&mut **tx).await?;
    let reference=match inserted {
        Some(reference)=>{
            for (ordinal,f) in input.fragments.iter().enumerate(){
                sqlx::query("INSERT INTO linggan_note_material_fragment(snapshot_ref,ordinal,citation_id,field,source_ref,source_version,logical_position,text_hash,char_start,char_end) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                .bind(reference).bind(ordinal as i32).bind(&f.citation_id).bind(&f.field).bind(f.source_ref)
                .bind(&f.source_version).bind(&f.logical_position).bind(&f.text_hash)
                .bind(i32::try_from(f.start).map_err(|_|Error::Invalid("invalid_character_range"))?)
                .bind(i32::try_from(f.end).map_err(|_|Error::Invalid("invalid_character_range"))?).execute(&mut **tx).await?;
            }
            reference
        },
        None=>sqlx::query_scalar("SELECT snapshot_ref FROM linggan_note_material_snapshot WHERE domain_ref=$1 AND content_public_ref=$2 AND snapshot_identity=$3")
            .bind(input.domain_ref).bind(input.work_ref).bind(&input.snapshot_identity).fetch_one(&mut **tx).await?,
    };
    Ok(SnapshotReceipt {
        snapshot_ref: reference,
        inserted: inserted.is_some(),
        snapshot_identity: input.snapshot_identity.clone(),
        material_fingerprint: input.material_fingerprint.clone(),
        provenance_hash: input.provenance_hash.clone(),
        ready: input.ready,
        fragment_count: input.fragments.len(),
    })
}
