//! Durable follow-up: completed primary -> independent audit -> immutable agreements.
use super::{
    DIMENSIONS, Title,
    run::{Settings, enqueue},
};
use crate::{
    ai_foundation::store::{Command, FoundationError},
    ops_content_lens::{MetricDefinition, TITLED_PUBLICATIONS_CTE, agreement, shuffle_with_seed},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

/// Local taxonomy files can name examples by their private export IDs. Only resolved
/// title keys are persisted in newly loaded versions; old, unresolved versions cannot run.
pub(super) fn example_keys(taxonomy: &Value) -> Result<BTreeSet<&str>, FoundationError> {
    let mut keys = BTreeSet::new();
    for dim in DIMENSIONS {
        for entry in taxonomy[dim].as_array().ok_or(FoundationError::Invalid)? {
            for id in entry["example_ids"]
                .as_array()
                .ok_or(FoundationError::Invalid)?
            {
                let key = id.as_str().ok_or(FoundationError::Invalid)?;
                if !crate::ops_content_lens::example_titles::is_key(key) {
                    return Err(FoundationError::Invalid);
                }
                keys.insert(key);
            }
        }
    }
    Ok(keys)
}

pub(crate) async fn advance(db: &Database) -> Result<(), FoundationError> {
    // Prefer settlement so a primary with paused egress cannot block a completed audit.
    let ready: Vec<(Uuid,bool)>=sqlx::query_as("SELECT run_ref,r.run_kind='audit' FROM linggan_ops_lens_label_run r
        JOIN linggan_ai_batch_control b ON b.batch_ref=r.foundation_batch_ref AND b.state='completed'
        WHERE r.title_count=(SELECT count(*) FROM linggan_ops_lens_label l WHERE l.run_ref=r.run_ref)
        AND ((r.run_kind='primary' AND NOT EXISTS(SELECT 1 FROM linggan_ops_lens_label_run a WHERE a.primary_run_ref=r.run_ref))
          OR (r.run_kind='audit' AND NOT EXISTS(SELECT 1 FROM linggan_ops_lens_agreement a WHERE a.audit_run_ref=r.run_ref)))
        ORDER BY (r.run_kind='audit') DESC,b.finished_at,r.run_ref").fetch_all(db.pool()).await?;
    for (id, audit) in ready {
        if audit {
            let mut tx = db.pool().begin().await?;
            save_agreements(&mut tx, id).await?;
            tx.commit().await?;
        } else {
            match start_audit(db, id).await {
                Ok(()) | Err(FoundationError::AdmissionDenied(_)) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

async fn start_audit(db: &Database, primary: Uuid) -> Result<(), FoundationError> {
    let cmd = Uuid::new_v4();
    let mut command = Command::begin(
        db,
        cmd,
        "ops_title_label_audit",
        &json!({"primaryRunRef":primary}),
    )
    .await?;
    let (taxonomy,metric,config,prompt,frozen,sqlx::types::Json(definition))=sqlx::query_as::<_,(Uuid,Uuid,Uuid,Uuid,Value,sqlx::types::Json<MetricDefinition>)>(
        "SELECT r.taxonomy_ref,r.metric_ref,r.task_config_ref,r.prompt_version_ref,r.frozen_input,m.body
         FROM linggan_ops_lens_label_run r JOIN linggan_ops_lens_definition m ON m.definition_ref=r.metric_ref
         WHERE r.run_ref=$1 FOR UPDATE OF r").bind(primary).fetch_one(&mut *command.tx).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_ops_lens_label_run WHERE primary_run_ref=$1)",
    )
    .bind(primary)
    .fetch_one(&mut *command.tx)
    .await?;
    if exists {
        return Ok(());
    }
    let shards: Vec<Vec<Title>> =
        serde_json::from_value(frozen["shards"].clone()).map_err(|_| FoundationError::Invalid)?;
    let mut titles: Vec<_> = shards.into_iter().flatten().collect();
    titles.sort_by(|a, b| a.title_key.cmp(&b.title_key));
    shuffle_with_seed(&mut titles, definition.audit_sample_seed)
        .map_err(|_| FoundationError::Invalid)?;
    titles.truncate(definition.audit_sample_size as usize);
    let settings = Settings {
        taxonomy,
        metric,
        config,
        prompt,
        definition,
    };
    let (run, batch) = enqueue(&mut command.tx, cmd, &settings, Some(primary), &titles).await?;
    command
        .finish(json!({"runRef":run,"batchRef":batch}))
        .await?;
    Ok(())
}

async fn save_agreements(
    tx: &mut Transaction<'_, Postgres>,
    audit: Uuid,
) -> Result<(), FoundationError> {
    let (primary, taxonomy, sqlx::types::Json(metric)) =
        sqlx::query_as::<_, (Uuid, Value, sqlx::types::Json<MetricDefinition>)>(
            "SELECT r.primary_run_ref,t.body,m.body FROM linggan_ops_lens_label_run r
         JOIN linggan_ops_lens_definition t ON t.definition_ref=r.taxonomy_ref
         JOIN linggan_ops_lens_definition m ON m.definition_ref=r.metric_ref WHERE r.run_ref=$1",
        )
        .bind(audit)
        .fetch_one(&mut **tx)
        .await?;
    let examples = example_keys(&taxonomy)?;
    let rows:Vec<(String,Value,Value)>=sqlx::query_as("SELECT a.title_key,to_jsonb(p),to_jsonb(a)
        FROM linggan_ops_lens_label a JOIN linggan_ops_lens_label p ON p.title_key=a.title_key AND p.run_ref=$2
        WHERE a.run_ref=$1 ORDER BY a.title_key").bind(audit).bind(primary).fetch_all(&mut **tx).await?;
    let mut settled = false;
    for dimension in DIMENSIONS {
        let pairs: Vec<_> = rows
            .iter()
            .map(|(k, p, a)| {
                Ok((
                    p[dimension].as_str().ok_or(FoundationError::Invalid)?,
                    a[dimension].as_str().ok_or(FoundationError::Invalid)?,
                    examples.contains(k.as_str()),
                ))
            })
            .collect::<Result<_, FoundationError>>()?;
        let answer = agreement(&pairs, &metric);
        let inserted = sqlx::query("INSERT INTO linggan_ops_lens_agreement(primary_run_ref,audit_run_ref,dimension,sample_count,
            excluded_example_count,agreement,kappa,reason) VALUES($1,$2,$3,$4,$5,$6::float8,$7::float8,$8)
            ON CONFLICT (primary_run_ref,audit_run_ref,dimension) DO NOTHING")
            .bind(primary).bind(audit).bind(dimension).bind(answer.sample_count as i32).bind(answer.excluded_example_count as i32)
            .bind(answer.agreement).bind(answer.kappa).bind(answer.reason).execute(&mut **tx).await?;
        settled |= inserted.rows_affected() > 0;
    }
    if settled {
        // The settlement and its refresh signal must be visible together.
        let sql = format!(
            "WITH {TITLED_PUBLICATIONS_CTE}
          UPDATE linggan_ops_lens_refresh q SET generation=q.generation+1
          FROM titled_publications t
          WHERE t.publication_ref=q.publication_ref AND t.title ~ '[^[:space:]]'
            AND EXISTS (SELECT FROM linggan_ops_lens_label l
              WHERE l.run_ref=$1 AND l.title_key=linggan_ops_lens_title_key(t.title))"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(primary)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
