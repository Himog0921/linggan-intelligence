//! Native registrations accumulate fifty distinct unlabelled titles before entering the existing blind pipeline.
use super::{
    Title,
    run::{Settings, enqueue, lock_primary},
};
use crate::{
    ai_foundation::store::{Command, FoundationError},
    ops_content_lens::TITLED_PUBLICATIONS_CTE,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Wait for a substantive independent audit instead of creating a one-title batch.
const AUTO_LABEL_MIN_TITLES: usize = 50;

type PublicationGeneration = (Uuid, i64);

struct CandidateGroup {
    title: String,
    publications: Vec<PublicationGeneration>,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IncrementalLabels {
    Idle,
    Deferred,
    Waiting {
        titles: usize,
        target: usize,
    },
    Queued {
        run_ref: Uuid,
        batch_ref: Uuid,
        titles: usize,
    },
    Blocked {
        reasons: Vec<String>,
    },
}

pub(in crate::ops_content_lens) async fn start_pending(
    db: &Database,
) -> Result<IncrementalLabels, FoundationError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT FROM linggan_ops_lens_refresh WHERE label_pending)",
    )
    .fetch_one(db.pool())
    .await?;
    if !exists {
        return Ok(IncrementalLabels::Idle);
    }
    let command_ref = Uuid::new_v4();
    let mut tx = db.pool().begin().await?;
    lock_primary(&mut tx).await?;
    let settings = match Settings::current(&mut tx).await {
        Ok(settings) => settings,
        Err(FoundationError::ConfigurationIncomplete) => {
            return Ok(IncrementalLabels::Blocked {
                reasons: vec!["ai_configuration_incomplete".into()],
            });
        }
        Err(error) => return Err(error),
    };

    let candidates = read_candidates(&mut tx, &settings).await?;
    if candidates.is_empty() {
        tx.commit().await?;
        return Ok(IncrementalLabels::Idle);
    }
    let target = AUTO_LABEL_MIN_TITLES;
    if settings.definition.audit_sample_size < 2 || settings.definition.label_batch_size == 0 {
        tx.commit().await?;
        return Ok(IncrementalLabels::Blocked {
            reasons: vec!["ops_lens_incremental_audit_size_invalid".into()],
        });
    }
    if candidates.len() < target {
        tx.commit().await?;
        return Ok(IncrementalLabels::Waiting {
            titles: candidates.len(),
            target,
        });
    }

    let mut queued = Vec::new();
    let titles: Vec<_> = candidates
        .into_iter()
        .take(target)
        .map(|(title_key, candidate)| {
            queued.extend(candidate.publications);
            Title {
                title_key,
                title: candidate.title,
            }
        })
        .collect();
    if clear_pending(&mut tx, &queued).await? != queued.len() as u64 {
        tx.rollback().await?;
        return Ok(IncrementalLabels::Deferred);
    }
    let mut command = Command::begin_in(
        tx,
        command_ref,
        "ops_title_label_incremental",
        &json!({"kind":"primary"}),
    )
    .await?;
    let (run_ref, batch_ref) =
        match enqueue(&mut command.tx, command_ref, &settings, None, &titles).await {
            Ok(receipt) => receipt,
            Err(FoundationError::AdmissionDenied(reasons)) => {
                return Ok(IncrementalLabels::Blocked { reasons });
            }
            Err(error) => return Err(error),
        };
    command
        .finish(IncrementalLabels::Queued {
            run_ref,
            batch_ref,
            titles: titles.len(),
        })
        .await
}

async fn read_candidates(
    tx: &mut Transaction<'_, Postgres>,
    settings: &Settings,
) -> Result<BTreeMap<String, CandidateGroup>, sqlx::Error> {
    let sql = format!(
        "WITH {TITLED_PUBLICATIONS_CTE}, candidates AS (
          SELECT q.publication_ref, q.generation, t.title, linggan_ops_lens_title_key(t.title) AS key
          FROM linggan_ops_lens_refresh q LEFT JOIN titled_publications t
            ON t.publication_ref=q.publication_ref AND t.title ~ '[^[:space:]]'
          WHERE q.label_pending
        ), attempted AS (
          SELECT DISTINCT item->>'titleKey' AS key
          FROM linggan_ops_lens_label_run r,
            LATERAL jsonb_array_elements(r.frozen_input->'shards') shard,
            LATERAL jsonb_array_elements(shard) item
          WHERE r.run_kind='primary' AND r.taxonomy_ref=$1 AND r.task_config_ref=$2 AND r.prompt_version_ref=$3
        )
        SELECT q.publication_ref, q.generation, q.title, q.key,
          EXISTS(SELECT FROM attempted a WHERE a.key=q.key)
          OR EXISTS(SELECT FROM linggan_ops_lens_effective_label l
            WHERE l.taxonomy_ref=$1 AND l.task_config_ref=$2 AND l.prompt_version_ref=$3 AND l.title_key=q.key) AS already_requested
        FROM candidates q ORDER BY q.key,q.publication_ref"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(settings.taxonomy)
        .bind(settings.config)
        .bind(settings.prompt)
        .fetch_all(&mut **tx)
        .await?;
    let mut processed = Vec::new();
    let mut candidates = BTreeMap::new();
    for row in rows {
        let generation: PublicationGeneration =
            (row.try_get("publication_ref")?, row.try_get("generation")?);
        let title: Option<String> = row.try_get("title")?;
        let key: Option<String> = row.try_get("key")?;
        let already_requested: bool = row.try_get("already_requested")?;
        match (title, key, already_requested) {
            (Some(title), Some(key), false) => {
                candidates
                    .entry(key)
                    .or_insert_with(|| CandidateGroup {
                        title,
                        publications: Vec::new(),
                    })
                    .publications
                    .push(generation);
            }
            _ => processed.push(generation),
        }
    }
    clear_pending(tx, &processed).await?;
    Ok(candidates)
}

async fn clear_pending(
    tx: &mut Transaction<'_, Postgres>,
    changes: &[PublicationGeneration],
) -> Result<u64, sqlx::Error> {
    if changes.is_empty() {
        return Ok(0);
    }
    let (publications, generations): (Vec<_>, Vec<_>) = changes.iter().copied().unzip();
    let result = sqlx::query(
        "UPDATE linggan_ops_lens_refresh q SET label_pending=false
         FROM unnest($1::uuid[],$2::bigint[]) AS observed(publication_ref,generation)
         WHERE q.publication_ref=observed.publication_ref AND q.generation=observed.generation
           AND q.label_pending",
    )
    .bind(publications)
    .bind(generations)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}
