//! Bounded status/ledger projections. HTTP reachability never becomes an executor health fact.
use super::super::store::FoundationError;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};

pub(crate) async fn snapshot_in(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Value, FoundationError> {
    let service:Value=sqlx::query_scalar("SELECT COALESCE((SELECT jsonb_build_object(
        'instanceRef',i.instance_ref,'generation',i.generation::text,'processId',i.process_id,'sourceRevision',i.source_revision,
        'state',i.state,'observedState',CASE WHEN i.state IN ('running','draining') AND NOT linggan_ai_executor_is_owner(i.instance_ref,i.generation,false) THEN 'stale' ELSE i.state END,
        'concurrency',i.concurrency,'startedAt',i.started_at,'heartbeatAt',i.heartbeat_at,
        'drainRequestedAt',i.drain_requested_at,'drainedAt',i.drained_at,
        'inflight',(SELECT count(*) FROM linggan_ai_batch_item_control WHERE lease_instance_ref=i.instance_ref AND state='running'))
        FROM linggan_ai_executor_current c JOIN linggan_ai_executor_instance i USING(instance_ref,generation)),jsonb_build_object('state','not_started','observedState','not_started'))")
        .fetch_one(&mut **tx).await?;
    let usage:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(row ORDER BY day DESC,task),'[]') FROM (
        SELECT budget_day AS day,task_key AS task,jsonb_build_object('taskKey',task_key,'budgetDay',budget_day,
        'calls',count(*)::text,'runningCalls',count(*) FILTER(WHERE state='running')::text,
        'failedCalls',count(*) FILTER(WHERE state='failed')::text,
        'measuredCalls',count(*) FILTER(WHERE usage_state='measured')::text,
        'unknownCalls',count(*) FILTER(WHERE usage_state='unknown_reserved')::text,
        'notDispatchedCalls',count(*) FILTER(WHERE usage_state='not_dispatched')::text,
        'measuredTokens',sum(measured_tokens) FILTER(WHERE usage_state='measured')::text,
        'quotaHeldTokens',sum(charged_tokens)::text,
        'unknownHeldTokens',sum(charged_tokens) FILTER(WHERE usage_state='unknown_reserved')::text,
        'knownQuotePicoUsd',sum(cost_pico_usd)::text,
        'unpricedMeasuredCalls',count(*) FILTER(WHERE usage_state='measured' AND cost_pico_usd IS NULL)::text,
        'cacheReadUnknownCalls',count(*) FILTER(WHERE cached_input_tokens IS NULL)::text,
        'reasoningUnknownCalls',count(*) FILTER(WHERE reasoning_tokens IS NULL)::text) AS row
        FROM linggan_model_invocation WHERE budget_day>=linggan_ai_budget_day()-6 GROUP BY budget_day,task_key
        ORDER BY budget_day DESC,task_key LIMIT 100)rows")
        .fetch_one(&mut **tx).await?;
    let failures:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(row ORDER BY created_at DESC),'[]') FROM (
        SELECT i.created_at,jsonb_build_object('invocationRef',i.invocation_ref,'taskKey',i.task_key,
        'batchRef',i.batch_ref,'taskConfigRef',i.task_config_ref,'connectionVersionRef',i.connection_version_ref,
        'state',i.state,'failureCode',i.failure_code,'description',f.description_zh,
        'retryable',f.retryable,'countsAsFailure',f.counts_as_failure,'usageState',i.usage_state,
        'dispatchState',i.dispatch_state,'createdAt',i.created_at,'finishedAt',i.finished_at) AS row
        FROM linggan_model_invocation i LEFT JOIN linggan_ai_failure_code f ON f.code=i.failure_code
        WHERE i.state='failed' ORDER BY i.created_at DESC,i.invocation_ref DESC LIMIT 100)rows")
        .fetch_one(&mut **tx).await?;
    let late: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('receiptCount',count(*)::text,
        'knownInputTokens',sum(input_tokens)::text,'knownOutputTokens',sum(output_tokens)::text,
        'quotaReconciled',false) FROM linggan_ai_late_usage_receipt",
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(json!({"service":service,"dailyUsage":usage,"failures":failures,"lateUsage":late}))
}
