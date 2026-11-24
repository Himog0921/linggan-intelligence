use super::{TASK, Title};
use crate::{
    ai_foundation::{
        batch_store::{self, BatchInput, BatchKind, InputRef},
        store::{Command, FoundationError},
    },
    ops_content_lens::{MetricDefinition, TITLED_PUBLICATIONS_CTE},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) struct Settings {
    pub taxonomy: Uuid,
    pub metric: Uuid,
    pub config: Uuid,
    pub prompt: Uuid,
    pub definition: MetricDefinition,
}

impl Settings {
    pub(super) async fn current(
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Self, FoundationError> {
        let (taxonomy,taxonomy_body,metric,config,prompt,sqlx::types::Json(definition))=sqlx::query_as(
            "SELECT t.definition_ref,td.body,m.definition_ref,c.task_config_ref,c.prompt_version_ref,d.body
             FROM linggan_ops_lens_definition_current t JOIN linggan_ops_lens_definition td ON td.definition_ref=t.definition_ref CROSS JOIN linggan_ops_lens_definition_current m
             JOIN linggan_ops_lens_definition d ON d.definition_ref=m.definition_ref
             CROSS JOIN linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref
             WHERE t.kind='taxonomy' AND m.kind='metric' AND s.task_key='ops_title_label'
             FOR SHARE OF t,m,s").fetch_optional(&mut **tx).await?.ok_or(FoundationError::ConfigurationIncomplete)?;
        super::advance::example_keys(&taxonomy_body)?;
        Ok(Self {
            taxonomy,
            metric,
            config,
            prompt,
            definition,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub total_titles: usize,
    pub reused_titles: usize,
    pub pending_titles: usize,
    pub shards: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReceipt {
    pub run_ref: Option<Uuid>,
    pub batch_ref: Option<Uuid>,
    pub plan: Plan,
}

async fn titles_in(
    tx: &mut Transaction<'_, Postgres>,
    s: &Settings,
) -> Result<(Vec<Title>, Plan), FoundationError> {
    let sql=format!("WITH {TITLED_PUBLICATIONS_CTE}, titles AS (
        SELECT DISTINCT ON (linggan_ops_lens_title_key(title)) linggan_ops_lens_title_key(title) AS key,title
        FROM titled_publications WHERE title ~ '[^[:space:]]'
        ORDER BY linggan_ops_lens_title_key(title),publication_ref)
        SELECT key,title,EXISTS(SELECT 1 FROM linggan_ops_lens_effective_label l
            WHERE l.taxonomy_ref=$1 AND l.task_config_ref=$2 AND l.prompt_version_ref=$3 AND l.title_key=t.key) AS reusable
        FROM titles t ORDER BY key");
    let all: Vec<(String, String, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(s.taxonomy)
        .bind(s.config)
        .bind(s.prompt)
        .fetch_all(&mut **tx)
        .await?;
    let total_titles = all.len();
    let titles: Vec<_> = all
        .into_iter()
        .filter(|r| !r.2)
        .map(|(title_key, title, _)| Title { title_key, title })
        .collect();
    let pending_titles = titles.len();
    let size = s.definition.label_batch_size as usize;
    if size == 0 {
        return Err(FoundationError::Invalid);
    }
    Ok((
        titles,
        Plan {
            total_titles,
            reused_titles: total_titles - pending_titles,
            pending_titles,
            shards: pending_titles.div_ceil(size),
        },
    ))
}

pub async fn plan(db: &Database) -> Result<Plan, FoundationError> {
    let mut tx = db.pool().begin().await?;
    let settings = Settings::current(&mut tx).await?;
    Ok(titles_in(&mut tx, &settings).await?.1)
}

pub async fn start(db: &Database, command_ref: Uuid) -> Result<RunReceipt, FoundationError> {
    let mut command = Command::begin(
        db,
        command_ref,
        "ops_title_label",
        &json!({"kind":"primary"}),
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_primary(&mut command.tx).await?;
    let s = Settings::current(&mut command.tx).await?;
    let (titles, plan) = titles_in(&mut command.tx, &s).await?;
    // Manual starts remain available, but they must leave titles already in an
    // automatic primary or its independent audit to that pipeline.
    let scheduled: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT item->>'titleKey'
         FROM linggan_ops_lens_label_run r
         JOIN linggan_ai_batch b ON b.batch_ref=r.foundation_batch_ref
         JOIN linggan_ai_batch_control bc USING(batch_ref)
         JOIN linggan_ai_command c ON c.command_ref=b.command_ref
         CROSS JOIN LATERAL jsonb_array_elements(r.frozen_input->'shards') shard
         CROSS JOIN LATERAL jsonb_array_elements(shard) item
         WHERE r.run_kind='primary' AND c.operation='ops_title_label_incremental'
           AND r.taxonomy_ref=$1 AND r.task_config_ref=$2 AND r.prompt_version_ref=$3
           AND (bc.state='open' OR (bc.state='completed'
               AND EXISTS(SELECT FROM linggan_ops_lens_label l WHERE l.run_ref=r.run_ref
                   AND l.title_key=item->>'titleKey')
               AND NOT EXISTS(SELECT FROM linggan_ops_lens_label_run a
                   JOIN linggan_ai_batch_control ac ON ac.batch_ref=a.foundation_batch_ref
                   WHERE a.primary_run_ref=r.run_ref AND ac.state<>'open')))",
    )
    .bind(s.taxonomy)
    .bind(s.config)
    .bind(s.prompt)
    .fetch_all(&mut *command.tx)
    .await?;
    let scheduled: std::collections::BTreeSet<_> = scheduled.into_iter().collect();
    let titles: Vec<_> = titles
        .into_iter()
        .filter(|t| !scheduled.contains(&t.title_key))
        .collect();
    let (run_ref, batch_ref) = if titles.is_empty() {
        (None, None)
    } else {
        let (r, b) = enqueue(&mut command.tx, command_ref, &s, None, &titles).await?;
        (Some(r), Some(b))
    };
    command
        .finish(RunReceipt {
            run_ref,
            batch_ref,
            plan,
        })
        .await
}

pub(super) async fn lock_primary(tx: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('ops_title_label:primary',234))")
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    command: Uuid,
    s: &Settings,
    primary: Option<Uuid>,
    titles: &[Title],
) -> Result<(Uuid, Uuid), FoundationError> {
    let kind = if primary.is_some() {
        "audit"
    } else {
        "primary"
    };
    let size = s.definition.label_batch_size as usize;
    if size == 0 || titles.is_empty() {
        return Err(FoundationError::Invalid);
    }
    let shards: Vec<_> = titles.chunks(size).collect();
    let frozen = json!({"shards":shards});
    let input_hash = content_hash(&frozen.to_string());
    let key = content_hash(
        &json!([input_hash, s.taxonomy, s.config, s.prompt, kind, primary]).to_string(),
    );
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,234))")
        .bind(&key)
        .execute(&mut **tx)
        .await?;
    if let Some(existing) = sqlx::query_as(
        "SELECT run_ref,foundation_batch_ref FROM linggan_ops_lens_label_run WHERE run_key=$1",
    )
    .bind(&key)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(existing);
    }
    let run = Uuid::new_v4();
    let items: Vec<_> = (0..shards.len())
        .map(|index| BatchInput {
            item_key: format!("{run}:{index}"),
            input: InputRef::OpsTitleLabel {
                run_ref: run,
                shard_index: index as u32,
            },
        })
        .collect();
    let hash = batch_store::input_hash(TASK, &items, false)?;
    let batch = batch_store::insert_batch(
        tx,
        command,
        TASK,
        s.config,
        BatchKind::Interactive,
        &items,
        &hash,
    )
    .await?;
    sqlx::query("INSERT INTO linggan_ops_lens_label_run(run_ref,run_kind,taxonomy_ref,metric_ref,primary_run_ref,task_config_ref,prompt_version_ref,input_hash,run_key,foundation_batch_ref,frozen_input,title_count,sample_seed,blind)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,true)")
        .bind(run).bind(kind).bind(s.taxonomy).bind(s.metric).bind(primary).bind(s.config).bind(s.prompt)
        .bind(input_hash).bind(key).bind(batch).bind(frozen).bind(titles.len() as i32)
        .bind(primary.map(|_|s.definition.audit_sample_seed as i64)).execute(&mut **tx).await?;
    Ok((run, batch))
}
