//! The team's "post written from this comment" mark (COMMENT-LIKE-LIBRARY-001).
//!
//! A mark is a person's record of material use, not a research result and not a publication
//! fact. Only state changes are stored; the latest event per stable comment is the current state.
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostUsageAction {
    Mark,
    Unmark,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PostUsageCommand {
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub comment_external_id: String,
    pub action: PostUsageAction,
    /// Client-generated per click; replaying it returns the same state instead of a second event.
    pub request_ref: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum PostUsageError {
    #[error("invalid post usage command")]
    InvalidCommand,
    #[error("the comment is absent or outside the selected domain")]
    CommentNotFound,
    #[error("the request reference was already used for a different mark")]
    IdempotencyConflict,
    #[error("the post usage schema is unavailable")]
    SchemaUnavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl PostUsageCommand {
    fn validate(&self) -> Result<(), PostUsageError> {
        let id = self.comment_external_id.as_str();
        if self.domain_ref.is_nil()
            || self.work_ref.is_nil()
            || self.request_ref.is_nil()
            || id.trim() != id
            || id.is_empty()
            || id.len() > 512
            || id.contains('\0')
        {
            return Err(PostUsageError::InvalidCommand);
        }
        Ok(())
    }

    fn event_action(&self) -> &'static str {
        match self.action {
            PostUsageAction::Mark => "marked",
            PostUsageAction::Unmark => "unmarked",
        }
    }

    fn digest(&self) -> String {
        super::sha256_hex(
            json!({
                "contract": "comment-post-usage.v1",
                "domainRef": self.domain_ref, "workRef": self.work_ref,
                "commentExternalId": self.comment_external_id, "action": self.event_action()
            })
            .to_string()
            .as_bytes(),
        )
    }
}

/// Record a mark or unmark for one stable comment visible in `domain_ref`.
///
/// Outcomes: `applied` (state changed), `unchanged` (only a command receipt is stored),
/// `replayed` (same accepted request and intent; returns its original postUsage).
pub async fn record_comment_post_usage(
    database: &Database,
    command: &PostUsageCommand,
) -> Result<Value, PostUsageError> {
    command.validate()?;
    record(database, command)
        .await
        .map_err(|error| match error {
            PostUsageError::Database(ref database_error)
                if database_error
                    .as_database_error()
                    .and_then(|value| value.code())
                    .is_some_and(|code| matches!(code.as_ref(), "42P01" | "42703" | "42883")) =>
            {
                PostUsageError::SchemaUnavailable
            }
            other => other,
        })
}

async fn record(database: &Database, command: &PostUsageCommand) -> Result<Value, PostUsageError> {
    let digest = command.digest();
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET LOCAL lock_timeout = '3s'")
        .execute(&mut *tx)
        .await?;
    // Request first, then comment: different comments cannot race on the same request reference,
    // and no-op requests reserve their intent just as state-changing commands do.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "comment-post-usage-request:{}",
            command.request_ref
        ))
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "comment-post-usage:{}:{}",
            command.work_ref, command.comment_external_id
        ))
        .execute(&mut *tx)
        .await?;

    let previous: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT request_digest, post_usage_state, \
         to_char(marked_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') \
         FROM linggan_comment_post_usage_receipt WHERE request_ref = $1",
    )
    .bind(command.request_ref)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((previous_digest, _, _)) = &previous {
        if previous_digest != &digest {
            return Err(PostUsageError::IdempotencyConflict);
        }
    }

    let visible: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
           SELECT 1 FROM linggan_material_comment comment \
           WHERE comment.content_public_ref = $1 AND comment.comment_external_id = $2 \
             AND EXISTS (SELECT 1 FROM linggan_material_domain_usage usage \
                         WHERE usage.content_public_ref = comment.content_public_ref \
                           AND usage.domain_ref = $3))",
    )
    .bind(command.work_ref)
    .bind(&command.comment_external_id)
    .bind(command.domain_ref)
    .fetch_one(&mut *tx)
    .await?;
    if !visible {
        return Err(PostUsageError::CommentNotFound);
    }

    if let Some((_, state, marked_at)) = previous {
        tx.commit().await?;
        return Ok(receipt(command, "replayed", &state, marked_at));
    }

    let outcome = {
        let current = current_state(&mut tx, command).await?;
        let marked = current
            .as_ref()
            .is_some_and(|(action, _)| action == "marked");
        let wants_marked = command.action == PostUsageAction::Mark;
        if marked == wants_marked {
            "unchanged"
        } else {
            sqlx::query(
                "INSERT INTO linggan_comment_post_usage_event \
                   (event_ref, request_ref, content_public_ref, comment_external_id, action, \
                    domain_ref, actor, request_digest) \
                 VALUES ($1, $2, $3, $4, $5, $6, 'person', $7)",
            )
            .bind(Uuid::new_v4())
            .bind(command.request_ref)
            .bind(command.work_ref)
            .bind(&command.comment_external_id)
            .bind(command.event_action())
            .bind(command.domain_ref)
            .bind(&digest)
            .execute(&mut *tx)
            .await?;
            "applied"
        }
    };
    let state = current_state(&mut tx, command).await?;
    let (state, marked_at) = match state {
        Some((action, recorded_at)) if action == "marked" => ("used", Some(recorded_at)),
        _ => ("unused", None),
    };
    sqlx::query(
        "INSERT INTO linggan_comment_post_usage_receipt \
         (request_ref, request_digest, content_public_ref, comment_external_id, domain_ref, \
          action, outcome, post_usage_state, marked_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::text::timestamptz)",
    )
    .bind(command.request_ref)
    .bind(&digest)
    .bind(command.work_ref)
    .bind(&command.comment_external_id)
    .bind(command.domain_ref)
    .bind(command.event_action())
    .bind(outcome)
    .bind(state)
    .bind(&marked_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(receipt(command, outcome, state, marked_at))
}

fn receipt(
    command: &PostUsageCommand,
    outcome: &str,
    state: &str,
    marked_at: Option<String>,
) -> Value {
    json!({
        "contract": "comment-post-usage.v1",
        "requestRef": command.request_ref,
        "outcome": outcome,
        "commentKey": {"workRef": command.work_ref, "commentExternalId": command.comment_external_id},
        "postUsage": {"state": state, "markedAt": marked_at}
    })
}

async fn current_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    command: &PostUsageCommand,
) -> Result<Option<(String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT action, to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') \
         FROM linggan_comment_post_usage_event \
         WHERE content_public_ref = $1 AND comment_external_id = $2 \
         ORDER BY recorded_at DESC, event_ref DESC LIMIT 1",
    )
    .bind(command.work_ref)
    .bind(&command.comment_external_id)
    .fetch_optional(&mut **tx)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> PostUsageCommand {
        serde_json::from_value(json!({
            "domainRef": crate::comment_study_source::ADHD_DOMAIN_REF,
            "workRef": Uuid::from_u128(7), "commentExternalId": "comment-1",
            "action": "mark", "requestRef": Uuid::from_u128(9)
        }))
        .unwrap()
    }

    #[test]
    fn command_shape_is_closed() {
        assert!(command().validate().is_ok());
        for patch in [
            json!({"action": "publish"}),
            json!({"actor": "agent"}),
            json!({"requestRef": "not-a-uuid"}),
        ] {
            let mut value = json!({
                "domainRef": crate::comment_study_source::ADHD_DOMAIN_REF,
                "workRef": Uuid::from_u128(7), "commentExternalId": "comment-1",
                "action": "mark", "requestRef": Uuid::from_u128(9)
            });
            value
                .as_object_mut()
                .unwrap()
                .extend(patch.as_object().unwrap().clone());
            assert!(serde_json::from_value::<PostUsageCommand>(value).is_err());
        }
        for bad_id in ["", " comment-1", "a\0b"] {
            let mut bad = command();
            bad.comment_external_id = bad_id.to_owned();
            assert!(matches!(
                bad.validate(),
                Err(PostUsageError::InvalidCommand)
            ));
        }
        let mut bad = command();
        bad.request_ref = Uuid::nil();
        assert!(matches!(
            bad.validate(),
            Err(PostUsageError::InvalidCommand)
        ));
    }

    /// The digest binds the intent, not the request reference: the same reference reused for the
    /// opposite action must conflict instead of silently replaying the first result.
    #[test]
    fn digest_distinguishes_mark_from_unmark_and_ignores_the_request_reference() {
        let mark = command();
        let mut unmark = command();
        unmark.action = PostUsageAction::Unmark;
        assert_ne!(mark.digest(), unmark.digest());
        let mut other_request = command();
        other_request.request_ref = Uuid::from_u128(10);
        assert_eq!(mark.digest(), other_request.digest());
    }
}
