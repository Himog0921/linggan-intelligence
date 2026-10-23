//! Internal planning and snapshot adapter over the sole public dossier contract.
use super::{Column,ColumnState,Error,FINGERPRINT_VERSION,Material,VERSION,cutoff,identity,validate_works};
use linggan_storage_postgres::Database;
use sqlx::{Postgres,Transaction};
use uuid::Uuid;

pub async fn read(database:&Database,domain:Uuid,work:Uuid)->Result<Material,Error>{Ok(read_at(database,domain,&[work],None).await?.remove(0))}
pub async fn read_at(database:&Database,domain:Uuid,works:&[Uuid],as_of:Option<&str>)->Result<Vec<Material>,Error>{
    validate_works(works)?;let mut tx=database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(&mut *tx).await?;
    let cutoff=cutoff(&mut tx,as_of).await?;let result=read_in(&mut tx,domain,works,&cutoff).await?;
    tx.commit().await?;Ok(result)
}
pub(super) async fn scope(tx:&mut Transaction<'_,Postgres>,domain:Uuid,works:&[Uuid],as_of:&str)->Result<(),Error>{
    validate_works(works)?;
    let rows=sqlx::query("SELECT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=ANY($2) AND created_at<=$3::timestamptz GROUP BY content_public_ref")
        .bind(domain).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    if rows.len()!=works.len(){return Err(Error::NotFound);}
    Ok(())
}
pub(super) async fn read_in(tx:&mut Transaction<'_,Postgres>,domain:Uuid,works:&[Uuid],as_of:&str)->Result<Vec<Material>,Error>{
    scope(tx,domain,works,as_of).await?;
    let mut dossiers=crate::research_dossier::read_in(tx,works,as_of).await?;
    let mut output=Vec::new();
    for work in works{
        let dossier=dossiers.remove(work).ok_or(Error::NotFound)?;
        let fragments=dossier.fragments.iter().map(|f| {
            let mut value = identity::fragment(&f.field,f.fragment_id.clone(),f.source_ref,f.source_version.clone(),f.start,f.text.clone());
            value.speaker=f.speaker.clone(); value.structure=f.structure.clone(); value.issues=f.issues.clone();
            value.reading_ref=f.reading_ref; value.unit_ref=f.unit_ref; value
        }).collect();
        let mut columns:Vec<_>=dossier.materials.iter().map(|m|{
            let state=match m["sourceState"].as_str(){Some("known_empty")=>ColumnState::KnownEmpty,Some("restricted")=>ColumnState::Restricted,Some("not_applicable")=>ColumnState::NotProvided,
                Some("present") if m["reading"].is_object()=>ColumnState::Available,Some("present")=>ColumnState::Waiting,_=>ColumnState::NotAcquired};
            let reasons=m["issues"].as_array().into_iter().flatten().filter_map(|i|i["code"].as_str().map(str::to_owned)).collect();
            Column{field:match m["kind"].as_str(){Some("image")=>"ocr",Some("audio")=>"transcript",Some("video_frame")=>"frame_text",Some(v)=>v,None=>"unknown"}.into(),state,reasons}
        }).collect();
        columns.push(Column{field:"title".into(),state:if dossier.note["platformTitle"].is_string(){ColumnState::Available}else{ColumnState::NotAcquired},reasons:vec![]});
        let not_ready_reasons=columns.iter().filter(|c|matches!(c.state,ColumnState::Waiting|ColumnState::Restricted)).map(|c|format!("{}_not_ready",c.field)).collect();
        let mut input=Material{contract:VERSION.into(),fingerprint_contract:FINGERPRINT_VERSION.into(),domain_ref:domain,work_ref:*work,as_of:as_of.into(),dossier_revision:dossier.revision,
            material_fingerprint:String::new(),provenance_hash:String::new(),snapshot_identity:String::new(),ready:false,not_ready_reasons,columns,fragments,layer_version:crate::ocr_layering::VERSION.into(),
            processing_scope:super::ProcessingScope{included:vec!["platform".into(),"image_ocr".into(),"asr".into(),"video_frame_ocr".into()],context_only:vec!["comment_image".into(),"author_avatar".into()],not_connected:vec![]}};
        identity::finish(&mut input);output.push(input);
    }
    Ok(output)
}
pub(super) async fn lock_work(tx:&mut Transaction<'_,Postgres>,work:Uuid)->Result<(),Error>{
    sqlx::query("SELECT pg_advisory_xact_lock(420,hashtext($1::uuid::text))").bind(work).execute(&mut **tx).await?;Ok(())
}
pub(super) async fn check_revision(tx:&mut Transaction<'_,Postgres>,work:Uuid,expected:&str)->Result<(),Error>{
    let now=cutoff(tx,None).await?;let value=crate::research_dossier::read_in(tx,&[work],&now).await?.remove(&work).ok_or(Error::NotFound)?;
    if value.revision!=expected{return Err(Error::Conflict);}Ok(())
}
