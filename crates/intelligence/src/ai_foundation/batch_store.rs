//! Queue frozen references only. Providers and material bodies are resolved by execution handlers.
use super::{TaskKey,store::{Command,FoundationError},task_config::lock_setting};
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_storage_postgres::Database;
use serde::{Deserialize,Serialize};
use serde_json::json;
use sqlx::{Postgres,Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ProbeOperation {Connect,Discover,Probe}
impl ProbeOperation {pub fn as_str(self)->&'static str{match self{Self::Connect=>"connect",Self::Discover=>"discover",Self::Probe=>"probe"}}}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ProbeProfile{pub capability_ref:Uuid,pub limits:super::capability::ModelLimits}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum InputRef {
    Probe{sample_ref:Uuid,connection_version_ref:Uuid,model_ref:Option<Uuid>,operation:ProbeOperation,
        #[serde(default,skip_serializing_if="Option::is_none")] profile:Option<ProbeProfile>},
    OcrAuditSample{sample_ref:Uuid},
    OcrRecognitionItem{item_ref:Uuid},
    WorkComparison{request_ref:Uuid},
    WritingRequest{request_ref:Uuid},
    WritingPreview{preview_ref:Uuid},
    Note{snapshot_ref:Uuid},
    DirectionSample{sample_ref:Uuid},
    PublicAuthor{summary_ref:Uuid},
    WritingSession{session_ref:Uuid},
}
impl InputRef {
    fn matches(&self,task:TaskKey,trial:bool)->bool {
        match self {
            Self::Probe{model_ref,operation,profile,..}=>task==TaskKey::ModelProbe
                &&((*operation==ProbeOperation::Probe)==model_ref.is_some())
                &&(profile.is_none()||*operation==ProbeOperation::Probe),
            Self::OcrAuditSample{..}=>task==TaskKey::OcrQualityAudit,
            Self::OcrRecognitionItem{..}=>task==TaskKey::OcrRecognition,
            Self::WorkComparison{..}=>task==TaskKey::WorkComparison&&!trial,
            Self::WritingRequest{..}=>task==TaskKey::WritingLegacy&&!trial,
            Self::WritingPreview{..}=>task==TaskKey::WritingLegacy&&trial,
            Self::Note{..}=>task==TaskKey::NoteInterpretation,
            Self::DirectionSample{..}=>matches!(task,TaskKey::DirectionListDraft|TaskKey::DirectionProposal),
            Self::PublicAuthor{..}=>task==TaskKey::AuthorOrgIdentity,
            Self::WritingSession{..}=>task==TaskKey::WritingAssistant,
        }
    }
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct BatchInput{pub item_key:String,pub input:InputRef}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum BatchKind{Trial,Automatic,Backfill,Interactive,Probe}
impl BatchKind {
    fn as_str(self)->&'static str{match self{Self::Trial=>"trial",Self::Automatic=>"automatic",Self::Backfill=>"backfill",Self::Interactive=>"interactive",Self::Probe=>"probe"}}
    fn permits(self,task:TaskKey)->bool{match task{
        TaskKey::ModelProbe=>self==Self::Probe,
        TaskKey::WorkComparison=>self==Self::Interactive,
        TaskKey::WritingLegacy|TaskKey::WritingAssistant|TaskKey::DirectionListDraft|TaskKey::DirectionProposal=>matches!(self,Self::Interactive|Self::Trial),
        TaskKey::NoteInterpretation|TaskKey::AuthorOrgIdentity|TaskKey::OcrQualityAudit|TaskKey::OcrRecognition=>self!=Self::Probe,
        _=>false,
    }}
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CreateBatch{pub command_ref:Uuid,pub task_key:TaskKey,pub expected_task_revision:i64,pub task_config_ref:Uuid,pub kind:BatchKind,pub items:Vec<BatchInput>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct BatchReceipt{pub batch_ref:Uuid,pub inputs_sha256:String,pub item_count:usize,pub accepted:bool}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CreateTrial{pub command_ref:Uuid,pub task_key:TaskKey,pub expected_task_revision:i64,pub baseline_config_ref:Option<Uuid>,pub candidate_config_ref:Uuid,pub items:Vec<BatchInput>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TrialReceipt{pub comparison_ref:Uuid,pub baseline_batch_ref:Option<Uuid>,pub candidate_batch_ref:Uuid,pub inputs_sha256:String,pub accepted:bool}

pub(crate) fn input_hash(task:TaskKey,items:&[BatchInput],trial:bool)->Result<String,FoundationError>{
    if items.is_empty()||items.len()>if trial{AI_LIMITS.round_size as usize}else{AI_LIMITS.max_batch_items as usize}{return Err(FoundationError::Invalid);}
    let mut keys=BTreeSet::new();
    for item in items{
        if item.item_key.is_empty()||item.item_key.len()>200
            ||!item.item_key.bytes().all(|b|b.is_ascii_alphanumeric()||b"._:-".contains(&b))
            ||!keys.insert(&item.item_key)||!item.input.matches(task,trial){return Err(FoundationError::Invalid);}
    }
    let text=serde_json::to_string(items).map_err(|_|FoundationError::Invalid)?;
    Ok(crate::research_text::content_hash(&text))
}
pub(crate) async fn insert_batch(tx:&mut Transaction<'_,Postgres>,command:Uuid,task:TaskKey,config:Uuid,kind:BatchKind,items:&[BatchInput],hash:&str)->Result<Uuid,FoundationError>{
    super::admission::check_queue_configuration(tx,task,config,kind==BatchKind::Trial).await?;
    let mut scopes=BTreeSet::new();
    for item in items{
        let scope=match &item.input{
            InputRef::Probe{connection_version_ref,model_ref,operation,profile,..}=>format!("{connection_version_ref}:{model_ref:?}:{}:{}",*operation==ProbeOperation::Probe,serde_json::to_string(profile).map_err(|_|FoundationError::Invalid)?),
            _=>"material".into(),
        };
        if scopes.insert(scope){super::admission::check_queue_input_scope(tx,task,config,&item.input).await?;}
    }
    let batch=Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(batch).bind(task.as_str()).bind(config).bind(kind.as_str()).bind(hash).bind(items.len() as i32).bind(command).execute(&mut **tx).await?;
    let records=items.iter().enumerate().map(|(position,item)|json!({"item_ref":Uuid::new_v4(),"position":position,"item_key":item.item_key,"input_ref":item.input})).collect::<Vec<_>>();
    sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) SELECT item_ref,$1,position,item_key,input_ref FROM jsonb_to_recordset($2::jsonb) AS items(item_ref uuid,position integer,item_key text,input_ref jsonb)")
        .bind(batch).bind(json!(records)).execute(&mut **tx).await?;
    Ok(batch)
}
pub async fn create_batch(db:&Database,request:&CreateBatch)->Result<BatchReceipt,FoundationError>{
    if request.kind==BatchKind::Trial||!request.kind.permits(request.task_key)||request.expected_task_revision<0{return Err(FoundationError::Invalid);}
    // Work comparison must freeze its exact material scope through its owning command.
    if request.task_key==TaskKey::WorkComparison{return Err(FoundationError::Invalid);}
    let hash=input_hash(request.task_key,&request.items,false)?;
    let payload=json!({"task":request.task_key,"expectedTaskRevision":request.expected_task_revision,"config":request.task_config_ref,"kind":request.kind,"inputsHash":hash});
    let mut command=Command::begin(db,request.command_ref,"batch",&payload).await?;
    if let Some(receipt)=command.replay()?{return Ok(receipt);}
    let current=lock_setting(&mut command.tx,request.task_key,request.expected_task_revision).await?;
    if current!=request.task_config_ref{return Err(FoundationError::Conflict);}
    let batch=insert_batch(&mut command.tx,request.command_ref,request.task_key,current,request.kind,&request.items,&hash).await?;
    command.finish(BatchReceipt{batch_ref:batch,inputs_sha256:hash,item_count:request.items.len(),accepted:true}).await
}
/// Caller locks this batch before any request row, matching execution's publication boundary.
pub(crate) async fn cancel_in(tx:&mut Transaction<'_,Postgres>,batch:Uuid)->Result<(),FoundationError>{
    use sqlx::Row;
    let row=sqlx::query("SELECT state,cancel_requested FROM linggan_ai_batch_control WHERE batch_ref=$1 FOR UPDATE")
        .bind(batch).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    if row.get::<String,_>("state")!="open"{return Ok(());}
    if !row.get::<bool,_>("cancel_requested"){
        sqlx::query("UPDATE linggan_ai_batch_control SET cancel_requested=true,revision=revision+1,stop_reason='human_cancelled' WHERE batch_ref=$1").bind(batch).execute(&mut **tx).await?;
    }
    sqlx::query("UPDATE linggan_ai_batch_item_control c SET state='cancelled',usable=false,finished_at=scope_001_now() FROM linggan_ai_batch_item item WHERE item.item_ref=c.item_ref AND item.batch_ref=$1 AND c.state='pending'").bind(batch).execute(&mut **tx).await?;
    super::rounds::evaluate_in(tx,batch).await?;Ok(())
}
pub async fn create_trial(db:&Database,request:&CreateTrial)->Result<TrialReceipt,FoundationError>{
    if !BatchKind::Trial.permits(request.task_key)||request.expected_task_revision<0||request.baseline_config_ref==Some(request.candidate_config_ref){return Err(FoundationError::Invalid);}
    let hash=input_hash(request.task_key,&request.items,true)?;
    let payload=json!({"task":request.task_key,"expectedTaskRevision":request.expected_task_revision,"baseline":request.baseline_config_ref,"candidate":request.candidate_config_ref,"inputsHash":hash});
    let mut command=Command::begin(db,request.command_ref,"trial_comparison",&payload).await?;
    if let Some(receipt)=command.replay()?{return Ok(receipt);}
    lock_setting(&mut command.tx,request.task_key,request.expected_task_revision).await?;
    let baseline=if let Some(config)=request.baseline_config_ref{
        let approved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_task_config c JOIN linggan_skill_binding b USING(task_key) WHERE c.task_config_ref=$1 AND c.task_key=$2 AND c.skill_version_ref IS NOT DISTINCT FROM b.current_version_ref)").bind(config).bind(request.task_key.as_str()).fetch_one(&mut *command.tx).await?;
        if !approved{return Err(FoundationError::SkillTrialRequired);}
        Some(insert_batch(&mut command.tx,request.command_ref,request.task_key,config,BatchKind::Trial,&request.items,&hash).await?)
    }else{None};
    let candidate=insert_batch(&mut command.tx,request.command_ref,request.task_key,request.candidate_config_ref,BatchKind::Trial,&request.items,&hash).await?;
    sqlx::query("INSERT INTO linggan_ai_trial_comparison(comparison_ref,task_key,baseline_batch_ref,candidate_batch_ref) VALUES($1,$2,$3,$4)").bind(request.command_ref).bind(request.task_key.as_str()).bind(baseline).bind(candidate).execute(&mut *command.tx).await?;
    command.finish(TrialReceipt{comparison_ref:request.command_ref,baseline_batch_ref:baseline,candidate_batch_ref:candidate,inputs_sha256:hash,accepted:true}).await
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn maximum_typed_batch_fits_the_http_boundary_and_probe_has_no_skill_trial_bypass(){
        let items=(0..AI_LIMITS.max_batch_items).map(|index|BatchInput{
            item_key:format!("{}{:010}","x".repeat(190),index),
            input:InputRef::Probe{sample_ref:Uuid::new_v4(),connection_version_ref:Uuid::new_v4(),model_ref:Some(Uuid::new_v4()),operation:ProbeOperation::Probe,profile:None},
        }).collect::<Vec<_>>();
        assert_eq!(input_hash(TaskKey::ModelProbe,&items,false).unwrap().len(),64);
        let request=CreateBatch{command_ref:Uuid::new_v4(),task_key:TaskKey::ModelProbe,expected_task_revision:0,task_config_ref:Uuid::new_v4(),kind:BatchKind::Probe,items};
        assert!(serde_json::to_vec(&request).unwrap().len()<8*1048576);
        assert!(!BatchKind::Trial.permits(TaskKey::ModelProbe));
    }
}
