use super::{ComparedRow,Error,METHOD_VERSION,Metric,MetricDifference,MetricDifferences,Metrics,
    ROUND_WORK_LIMIT,Round,RoundComparison,RoundResult,RoundWork,Scope,rounds,scope,trace};
use linggan_storage_postgres::Database;
use sqlx::{Postgres,Row,Transaction};
use std::collections::{BTreeMap,BTreeSet};
use uuid::Uuid;

pub async fn read_round_result(database:&Database,request:&Scope,round_ref:Uuid) -> Result<RoundResult,Error> {
    let (mut tx,cutoff)=scope::begin(database,request).await?;
    let round=rounds::read_in(&mut tx,request,&cutoff,None,Some(&[round_ref]),1,0).await?
        .pop().ok_or(Error::NotFound)?;
    let mut works=read_works(&mut tx,request,&cutoff,&round,&round).await?;
    let truncated=works.len()>ROUND_WORK_LIMIT;
    works.truncate(ROUND_WORK_LIMIT);
    tx.commit().await?;
    Ok(RoundResult {target_ref:request.target_ref,knowledge_cutoff:cutoff,method_version:METHOD_VERSION,round,works,truncated})
}

pub async fn compare_rounds(database:&Database,request:&Scope,left_ref:Uuid,right_ref:Uuid) -> Result<RoundComparison,Error> {
    if left_ref==right_ref{return Err(Error::Invalid("choose_two_different_rounds"));}
    let (mut tx,cutoff)=scope::begin(database,request).await?;
    let selected=rounds::read_in(&mut tx,request,&cutoff,None,Some(&[left_ref,right_ref]),2,0).await?;
    let left=selected.iter().find(|r|r.round_ref==left_ref).cloned().ok_or(Error::NotFound)?;
    let right=selected.iter().find(|r|r.round_ref==right_ref).cloned().ok_or(Error::NotFound)?;
    if left.observed_epoch_ms>right.observed_epoch_ms{return Err(Error::Invalid("left_round_must_not_be_newer"));}
    let a=read_works(&mut tx,request,&cutoff,&left,&left).await?;
    let b=read_works(&mut tx,request,&cutoff,&right,&left).await?;
    let truncated=a.len()>ROUND_WORK_LIMIT||b.len()>ROUND_WORK_LIMIT;
    let a:BTreeMap<_,_>=a.into_iter().take(ROUND_WORK_LIMIT).map(|w|(w.work_ref,w)).collect();
    let b:BTreeMap<_,_>=b.into_iter().take(ROUND_WORK_LIMIT).map(|w|(w.work_ref,w)).collect();
    let keys:BTreeSet<_>=a.keys().chain(b.keys()).copied().collect();
    let rows=keys.into_iter().map(|key|compared_row(key,a.get(&key).cloned(),b.get(&key).cloned(),truncated)).collect::<Vec<_>>();
    let reasons=comparability_reasons(&left,&right,truncated);
    let comparison_state=if left.rule_ref!=right.rule_ref{"side_by_side"}else if reasons.is_empty(){"comparable"}else{"limited"};
    let count=|kind|(!truncated).then(||rows.iter().filter(|r|r.membership==kind).count());
    let common_count=count("both");let entered_count=count("entered");let absent_count=count("absent");
    tx.commit().await?;
    Ok(RoundComparison {target_ref:request.target_ref,knowledge_cutoff:cutoff,method_version:METHOD_VERSION,
        left,right,comparison_state,reasons,rows,common_count,entered_count,absent_count,truncated})
}

fn compared_row(key:Uuid,left:Option<RoundWork>,right:Option<RoundWork>,truncated:bool) -> ComparedRow {
    let membership=match (left.is_some(),right.is_some(),truncated){
        (true,true,_)=>"both",(_,_,true)=>"unconfirmed",(false,true,false)=>"entered",_=>"absent"};
    let discovery_label=match right.as_ref(){
        None if truncated=>"membership_unconfirmed",None=>"not_observed_this_round",
        Some(_) if membership=="both"=>"repeat",
        Some(v) if v.library_presence=="new"=>"new_in_library",
        Some(v) if v.first_seen_in_scope==Some(true)&&v.library_presence=="existing"=>"first_in_scope",
        Some(v) if v.seen_before_left=>"returned",_=>"first_scope_unconfirmed",
    }.to_owned();
    let metrics=differences(left.as_ref().map(|w|&w.metrics),right.as_ref().map(|w|&w.metrics));
    ComparedRow {work_ref:key,membership,discovery_label,left,right,metrics}
}

fn comparability_reasons(left:&Round,right:&Round,truncated:bool) -> Vec<&'static str> {
    let mut reasons=Vec::new();
    if left.coverage!="complete"{reasons.push("left_round_incomplete");}
    if right.coverage!="complete"{reasons.push("right_round_incomplete");}
    if left.rule_ref.is_none()||right.rule_ref.is_none(){reasons.push("historical_rule_unknown");}
    if left.rule_ref!=right.rule_ref{reasons.push("different_rules");}
    if left.conditions!=right.conditions{reasons.push("execution_conditions_changed");}
    match (left.account_ref,right.account_ref){
        (Some(a),Some(b)) if a!=b=>reasons.push("observation_account_changed"),
        (None,_)|(_,None)=>reasons.push("observation_account_unknown"),_=>(),
    }
    if left.observed_epoch_ms==right.observed_epoch_ms{reasons.push("observation_order_unknown");}
    if truncated{reasons.push("comparison_scope_truncated");}
    reasons
}

async fn read_works(tx:&mut Transaction<'_,Postgres>,request:&Scope,cutoff:&str,round:&Round,left:&Round)
    -> Result<Vec<RoundWork>,Error>
{
    let rows=sqlx::query(include_str!("round_works.sql")).bind(request.target_ref).bind(request.domain_ref)
        .bind(cutoff).bind(round.package_ref).bind(round.rule_ref).bind(&left.observed_at)
        .bind(i64::try_from(ROUND_WORK_LIMIT+1).unwrap_or(201)).fetch_all(&mut **tx).await?;
    rows.into_iter().map(|row|{
        let source=round.conditions["surface"].as_str().unwrap_or("unknown");
        let material:Uuid=row.try_get("material_ref")?;
        let mut metrics=trace::metrics_from_row(&row,&material.to_string(),&round.observed_at,round.observed_epoch_ms,source)?;
        for (name,metric) in [("conflicting_likes",&mut metrics.likes),("conflicting_comments",&mut metrics.comments),
            ("conflicting_collects",&mut metrics.collects),("conflicting_shares",&mut metrics.shares)] {
            if row.try_get::<bool,_>(name)?{*metric=Metric::unknown();metric.precision="conflicting_records".into();}
        }
        Ok(RoundWork {work_ref:row.try_get("content_public_ref")?,title:row.try_get("title")?,
            creator:row.try_get("creator_display_name")?,position:row.try_get("position")?,metrics,
            first_seen_in_scope:row.try_get("first_seen_in_scope")?,library_presence:row.try_get("library_presence")?,
            seen_before_left:row.try_get("seen_before_left")?})
    }).collect()
}

pub(crate) fn differences(a:Option<&Metrics>,b:Option<&Metrics>) -> MetricDifferences {
    let unknown=Metrics::unknown();let a=a.unwrap_or(&unknown);let b=b.unwrap_or(&unknown);
    MetricDifferences {likes:difference(&a.likes,&b.likes),comments:difference(&a.comments,&b.comments),
        collects:difference(&a.collects,&b.collects),shares:difference(&a.shares,&b.shares)}
}

fn difference(left:&Metric,right:&Metric) -> MetricDifference {
    let elapsed=left.observed_epoch_ms.zip(right.observed_epoch_ms).and_then(|(a,b)|b.checked_sub(a))
        .filter(|d|*d>0).map(|v|v as f64/1000.0);
    let reason=if left.state!="known"||right.state!="known"{Some("metric_not_observed")}
        else if left.precision!="exact"||right.precision!="exact"{Some("metric_precision_insufficient")}
        else if elapsed.is_none(){Some("observation_order_unknown")}else{None};
    let delta=if reason.is_none(){left.value.zip(right.value).and_then(|(a,b)|b.checked_sub(a))}else{None};
    let percent=delta.zip(left.value).filter(|(_,a)|*a>0).map(|(d,a)|d as f64/a as f64*100.0);
    MetricDifference {left:left.clone(),right:right.clone(),delta,elapsed_seconds:elapsed,percent,comparable:delta.is_some(),reason}
}
