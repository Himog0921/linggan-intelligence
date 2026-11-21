//! Each accepted shard is committed in the same transaction as its foundation receipt.
use super::{super::{ExecutorError,queue::ClaimedItem,receipt::{self,ItemResult}},research_call};
use crate::{
    ai_foundation::{admission,batch_store::InputRef,store::FoundationError,transport::ResponseSchema},
    model_invocation::finish_invocation_in,model_secrets::ModelSecretStore,
    ops_content_lens::label::{self,Title},pi_adapter::{PiAdapter,PiResponse,safe_result},
    research_text::content_hash
};
use linggan_storage_postgres::Database;
use serde_json::{json,Value};
use sqlx::{Postgres,Transaction};
use uuid::Uuid;

struct Prepared {call:research_call::Call,run:Uuid,titles:Vec<Title>,taxonomy:Value}

async fn permitted(tx:&mut Transaction<'_,Postgres>,item:&ClaimedItem)->Result<(),FoundationError>{
    admission::check_queue_configuration(tx,item.task_key,item.task_config_ref,true).await?;
    admission::check_queue_input_scope(tx,item.task_key,item.task_config_ref,&item.input).await
}

async fn prepare(db:&Database,item:&ClaimedItem,secrets:&dyn ModelSecretStore)->Result<Option<Prepared>,ExecutorError>{
    let InputRef::OpsTitleLabel{run_ref,shard_index}=item.input else{return Err(ExecutorError::Invalid);};
    if shard_index!=item.position{return Err(ExecutorError::Invalid);}
    let mut tx=item.owner.begin(db,true).await?;
    if item.lock_in(&mut tx).await?{return Ok(None);}
    permitted(&mut tx,item).await?;
    let (taxonomy,sqlx::types::Json(titles))=sqlx::query_as::<_,(Value,sqlx::types::Json<Vec<Title>>)>(
        "SELECT t.body,(r.frozen_input->'shards')->$4::int FROM linggan_ops_lens_label_run r
         JOIN linggan_ops_lens_definition t ON t.definition_ref=r.taxonomy_ref
         WHERE r.run_ref=$1 AND r.foundation_batch_ref=$2 AND r.task_config_ref=$3")
         .bind(run_ref).bind(item.batch_ref).bind(item.task_config_ref).bind(shard_index as i32)
         .fetch_one(&mut *tx).await?;
    let payload=label::payload(&taxonomy,&titles);
    let fingerprint=content_hash(&payload.to_string());
    let call=research_call::prepare_in(&mut tx,item,secrets,research_call::Input{
        contract:label::CONTRACT,reference:run_ref,fingerprint:&fingerprint,payload,
        response_schema:ResponseSchema{name:"ops_title_label".into(),schema:label::response_schema(&taxonomy)},
        material_class:"own_published_title",
    }).await?;
    tx.commit().await?;
    Ok(Some(Prepared{call,run:run_ref,titles,taxonomy}))
}

pub(super) async fn execute(db:&Database,item:&ClaimedItem,secrets:&dyn ModelSecretStore,adapter:&PiAdapter)->Result<(),ExecutorError>{
    let prepared=match prepare(db,item,secrets).await {
        Ok(Some(p))=>p,
        Ok(None)=>return item.skip_unattempted(db,"batch_cancelled").await,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons)))=>return item.defer(db,reasons.first().map_or("admission_denied",String::as_str)).await,
        Err(e)=>return super::super::preparation::handle(db,item,e).await,
    };
    let response=super::super::dispatch::call(db,item,adapter,&prepared.call.request).await;
    receipt::finalize(db,item,prepared.call.invocation,response.as_ref(),||settle(db,item,&prepared,response.as_ref())).await
}

async fn settle(db:&Database,item:&ClaimedItem,p:&Prepared,response:Option<&PiResponse>)->Result<(),ExecutorError>{
    let invocation=p.call.invocation;
    let (mut tx,cancelled)=receipt::lock_in(db,item,invocation,response).await?;
    let allowed=if cancelled{false}else{match permitted(&mut tx,item).await {
        Ok(())=>true,Err(FoundationError::AdmissionDenied(_))=>false,Err(e)=>return Err(e.into())
    }};
    let accepted=response.filter(|r|r.ok && allowed).and_then(|r|r.text.as_deref())
        .and_then(|text|label::accepted_labels(text,&p.taxonomy,&p.titles));
    if let Some(rows)=&accepted {
        sqlx::query(r#"INSERT INTO linggan_ops_lens_label(run_ref,title_key,situation,moment,offer,audience,confidence,invocation_ref)
            SELECT $1,"titleKey",situation,moment,offer,audience,confidence,$2 FROM jsonb_to_recordset($3)
            AS x("titleKey" text,situation text,moment text,offer text,audience text,confidence text)"#)
            .bind(p.run).bind(invocation).bind(sqlx::types::Json(rows)).execute(&mut *tx).await?;
    }
    let success=accepted.is_some();
    let failure=research_call::failure(response,allowed,success,"ai_source_unavailable");
    let mut result=response.map(safe_result).unwrap_or_else(||json!({"ok":false}));
    result["executorReceipt"]=json!(item.lease_token);
    receipt::observe_in(&mut tx,invocation,response).await?;
    finish_invocation_in(&mut tx,invocation,response,success,failure,&result).await?;
    let output=json!({"runRef":p.run,"labeledTitles":p.titles.len()});
    receipt::write_item_in(&mut tx,item,ItemResult{success,cancelled,output:success.then_some(&output),failure,
        counts_as_failure:!success && allowed && !receipt::not_dispatched(response)}).await?;
    tx.commit().await?;
    Ok(())
}
