//! Exact, opt-in detail-only supplement. A target's missing-details read and
//! request are two views of the same accepted directory and execution facts.
//! It never schedules merely because the inspector was opened.
use super::{Error,Scope,scope};
use crate::{acquisition_chain,MaterialDeepeningTarget,DETAIL_WINDOW_COMMENT_LIMIT,DETAIL_WINDOW_REPLY_EXPAND_LIMIT};
use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::{Postgres,Transaction,AssertSqlSafe};
use uuid::Uuid;
use std::collections::BTreeSet;

#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct MissingDetail {
    pub work_ref:Uuid,
    pub status:&'static str,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct DetailGapPreview {
    pub target_ref:Uuid,
    pub domain_ref:Uuid,
    pub works:Vec<MissingDetail>,
    pub eligible_refs:Vec<Uuid>,
    pub truncated:bool,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct DetailGapReceipt {
    pub request_ref:Uuid,
    pub work_order_ref:Option<Uuid>,
    pub admission:String,
    pub requested_count:usize,
}
const LIMIT:usize=200;

async fn domain_in(tx:&mut Transaction<'_,Postgres>,request:&Scope)->Result<Uuid,Error>{
    let rows:Vec<Uuid>=sqlx::query_scalar(
        "SELECT relation.domain_ref FROM observation_domain_target relation \
         JOIN observation_domain domain USING(domain_ref) \
         WHERE relation.target_ref=$1 AND domain.status='active' \
         AND (($2::uuid IS NOT NULL AND relation.domain_ref=$2) \
           OR ($2::uuid IS NULL AND relation.role='primary')) \
         ORDER BY domain.created_at,relation.domain_ref LIMIT 2",
    ).bind(request.target_ref).bind(request.domain_ref).fetch_all(&mut **tx).await?;
    if rows.len()!=1{return Err(Error::Scope);}
    Ok(rows[0])
}
async fn missing_in(tx:&mut Transaction<'_,Postgres>,target:Uuid,domain:Uuid)
    ->Result<Vec<(Uuid,bool,bool)>,Error>
{
    let statement=concat!(
        "WITH ",crate::archive_ledger::directory_works_sql!("target.target_ref=$1","true"),
        r#" SELECT directory_work.content_public_ref,
             EXISTS(
                SELECT 1 FROM collection_work_order scoped_order
                JOIN collection_work_order_material_target scope USING(work_order_ref)
                JOIN collection_work_order_domain_usage scoped_usage USING(work_order_ref)
                LEFT JOIN collection_work_order_lease lease USING(work_order_ref)
                WHERE scoped_order.target_ref=$1 AND scoped_usage.domain_ref=$2
                  AND scope.content_public_ref=directory_work.content_public_ref
                  AND (scoped_order.queue_state IN ('queued','leased')
                    OR (lease.released_at IS NULL AND lease.expires_at>scope_001_now()))
             ) AS in_flight,
             /*executable*/ AS executable
           FROM directory_work
           JOIN linggan_material_content content ON content.public_ref=directory_work.content_public_ref
           WHERE directory_work.target_ref=$1
             AND NOT directory_work.has_detail AND NOT directory_work.is_retired
             AND EXISTS(
                SELECT 1 FROM linggan_material_domain_usage usage
                WHERE usage.content_public_ref=directory_work.content_public_ref AND usage.domain_ref=$2
             )
           ORDER BY directory_work.content_public_ref LIMIT 201"#,
    ).replace("/*executable*/",&crate::execution_input_eligibility::has_executable_locator_predicate("content.content_external_id"));
    Ok(sqlx::query_as::<_,(Uuid,bool,bool)>(AssertSqlSafe(statement))
        .bind(target).bind(domain).fetch_all(&mut **tx).await?)
}
fn make_preview(target:Uuid,domain:Uuid,rows:Vec<(Uuid,bool,bool)>)->DetailGapPreview{
    let truncated=rows.len()>LIMIT;
    let works=rows.into_iter().take(LIMIT).map(|(work_ref,in_flight,executable)|MissingDetail{
        work_ref,status:if in_flight{"in_flight"}else if !executable{"locator_unavailable"}else{"requestable"},
    }).collect::<Vec<_>>();
    let eligible_refs=works.iter().filter(|w|w.status=="requestable").map(|w|w.work_ref).collect();
    DetailGapPreview{target_ref:target,domain_ref:domain,works,eligible_refs,truncated}
}
pub async fn preview(database:&Database,request:&Scope)->Result<DetailGapPreview,Error>{
    let (mut tx,_)=scope::begin(database,request).await?;
    let target_kind:String=sqlx::query_scalar("SELECT target_kind FROM collection_observation_target WHERE target_ref=$1")
        .bind(request.target_ref).fetch_one(&mut *tx).await?;
    if target_kind!="creator"{return Err(Error::Invalid("detail_gaps_require_creator"));}
    let domain=domain_in(&mut tx,request).await?;
    let rows=missing_in(&mut tx,request.target_ref,domain).await?;
    tx.commit().await?;
    Ok(make_preview(request.target_ref,domain,rows))
}
pub async fn request(
    database:&Database,request:&Scope,requested:&[Uuid],
)->Result<DetailGapReceipt,Error>{
    if requested.is_empty()||requested.len()>LIMIT||requested.iter().collect::<BTreeSet<_>>().len()!=requested.len(){
        return Err(Error::Invalid("invalid_exact_detail_scope"));
    }
    let mut tx=database.pool().begin().await?;
    let target_kind:Option<String>=sqlx::query_scalar(
        "SELECT target_kind FROM collection_observation_target WHERE target_ref=$1 FOR UPDATE",
    ).bind(request.target_ref).fetch_optional(&mut *tx).await?;
    if target_kind.as_deref()!=Some("creator"){return Err(Error::NotFound);}
    let domain=domain_in(&mut tx,request).await?;
    let current=make_preview(request.target_ref,domain,missing_in(&mut tx,request.target_ref,domain).await?);
    let allowed=current.eligible_refs.into_iter().collect::<BTreeSet<_>>();
    if !requested.iter().all(|ref_id|allowed.contains(ref_id)){
        return Err(Error::Invalid("detail_gap_selection_stale"));
    }
    let materials=requested.iter().copied().map(|work|MaterialDeepeningTarget{
        content_public_ref:work,
        comment_limit:DETAIL_WINDOW_COMMENT_LIMIT,
        reply_expand_limit:DETAIL_WINDOW_REPLY_EXPAND_LIMIT,
        acquire_media:true,
        allow_ocr:true,
        allow_asr:true,
    }).collect::<Vec<_>>();
    let outcome=acquisition_chain::request_and_admit_in_transaction_scoped(
        &mut tx,request.target_ref,"deep_archive",
        "从观察目标页发起深度建档","person",Some(domain),&materials,
        None,None,false,
    ).await?;
    let receipt=DetailGapReceipt{
        request_ref:outcome.request_ref,work_order_ref:outcome.work_order_ref,
        admission:outcome.outcome.code().to_owned(),requested_count:requested.len(),
    };
    tx.commit().await?;
    Ok(receipt)
}
