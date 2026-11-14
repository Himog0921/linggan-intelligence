//! §8.3 invariant checks. Every check is a static read-only query; a check whose tables do not
//! exist yet is reported as unavailable rather than silently passing.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvariantCheck {
    pub id: &'static str,
    pub name: &'static str,
    pub available: bool,
    pub violations: i64,
    pub missing_tables: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvariantReport {
    pub checks: Vec<InvariantCheck>,
    pub total_violations: i64,
    pub incomplete: bool,
}

impl InvariantReport {
    pub fn failed(&self) -> bool {
        self.total_violations > 0
    }
}

struct Definition {
    id: &'static str,
    name: &'static str,
    requires: &'static [&'static str],
    sql: &'static str,
}

const DEFINITIONS: &[Definition] = &[
    Definition {
        id: "I1",
        name: "problem_metric_count_within_total",
        requires: &["linggan_opportunity_problem_metric_period"],
        sql: "SELECT count(*) FROM linggan_opportunity_problem_metric_period WHERE k > n",
    },
    Definition {
        id: "I2",
        name: "problem_metric_interval_bounds",
        requires: &["linggan_opportunity_problem_metric_period"],
        sql: "SELECT count(*) FROM linggan_opportunity_problem_metric_period
              WHERE ci_low IS NOT NULL
                AND NOT (0 <= ci_low AND ci_low <= ci_high AND ci_high <= 1)",
    },
    Definition {
        id: "I3",
        name: "problem_metric_empty_is_explicit",
        requires: &["linggan_opportunity_problem_metric_period"],
        sql: "SELECT count(*) FROM linggan_opportunity_problem_metric_period
              WHERE (n = 0 AND (ci_low IS NOT NULL OR ci_high IS NOT NULL))
                 OR (n > 0 AND (ci_low IS NULL OR ci_high IS NULL))",
    },
    Definition {
        id: "I4",
        name: "work_problem_judgment_has_quote",
        requires: &["linggan_opportunity_work_problem_judgment"],
        sql: "SELECT count(*) FROM linggan_opportunity_work_problem_judgment
              WHERE label <> 'none' AND (evidence_quote IS NULL OR evidence_quote = '')",
    },
    Definition {
        id: "I5",
        name: "human_judgment_has_reviewer",
        requires: &["linggan_opportunity_work_problem_judgment"],
        sql: "SELECT count(*) FROM linggan_opportunity_work_problem_judgment
              WHERE source = 'human' AND reviewer IS NULL",
    },
    Definition {
        id: "I6",
        name: "mention_points_to_stable_comment",
        requires: &[
            "linggan_opportunity_comment_mention",
            "linggan_opportunity_comment_fact",
        ],
        sql: "SELECT count(*) FROM linggan_opportunity_comment_mention mention
              LEFT JOIN linggan_opportunity_comment_fact fact
                ON fact.content_ref = mention.content_ref
               AND fact.comment_external_id = mention.comment_external_id
              WHERE fact.content_ref IS NULL",
    },
    Definition {
        id: "I6b",
        name: "unknown_commenter_has_no_hash",
        requires: &["linggan_opportunity_comment_mention"],
        sql: "SELECT count(*) FROM linggan_opportunity_comment_mention
              WHERE (commenter_hash IS NULL)
                    <> (excluded_reason IS NOT DISTINCT FROM 'commenter_unknown')",
    },
    Definition {
        id: "I7",
        name: "sufficient_creator_meets_minimum",
        requires: &[
            "linggan_opportunity_creator_problem_density",
            "linggan_opportunity_metric_definition",
        ],
        sql: "SELECT count(*) FROM linggan_opportunity_creator_problem_density density
              WHERE density.sufficient
                AND density.n < (
                  SELECT (definition.params->>'creator_min_n')::int
                  FROM linggan_opportunity_metric_definition definition
                  WHERE definition.version = density.metric_def_version
                )",
    },
    Definition {
        id: "I8",
        name: "problem_lineage_acyclic",
        requires: &["linggan_opportunity_problem_lineage"],
        sql: "WITH RECURSIVE walk(start_id, cur_id, depth) AS (
                SELECT from_problem_ref, to_problem_ref, 1
                FROM linggan_opportunity_problem_lineage
                WHERE relation IN ('merge', 'rename')
                UNION ALL
                SELECT walk.start_id, lineage.to_problem_ref, walk.depth + 1
                FROM walk
                JOIN linggan_opportunity_problem_lineage lineage
                  ON lineage.from_problem_ref = walk.cur_id
                 AND lineage.relation IN ('merge', 'rename')
                WHERE walk.depth < 100
              )
              SELECT count(*) FROM walk WHERE start_id = cur_id",
    },
    Definition {
        id: "I9",
        name: "card_samples_have_no_restricted_comment",
        requires: &[
            "linggan_opportunity_opportunity_card",
            "linggan_material_comment_restriction",
        ],
        sql: "SELECT count(*) FROM linggan_opportunity_opportunity_card card
              WHERE EXISTS (
                SELECT 1 FROM jsonb_array_elements(card.payload->'demand'->'sampleVoices') voice
                WHERE voice->>'text' IS NOT NULL
                  AND EXISTS (
                    SELECT 1 FROM linggan_material_comment_restriction restriction
                    WHERE restriction.content_public_ref::text = voice->'commentRef'->>'contentRef'
                      AND restriction.comment_external_id = voice->'commentRef'->>'commentExternalId'
                  )
              )",
    },
    Definition {
        id: "I10",
        name: "ai_supply_verified_only_after_quality_gate",
        requires: &["linggan_opportunity_opportunity_card", "quality_gate"],
        sql: "SELECT count(*) FROM linggan_opportunity_opportunity_card card
              WHERE card.payload->'supply'->>'judgmentStatus' = 'verified'
                AND EXISTS (
                  SELECT 1 FROM jsonb_array_elements(card.payload->'supply'->'works') work
                  WHERE work->>'source' = 'ai'
                    AND NOT EXISTS (
                      SELECT 1 FROM quality_gate gate
                      WHERE gate.task_type = 'work_problem_judgment' AND gate.passed
                        AND gate.prompt_version = card.payload->'versions'->>'judgmentPrompt'
                        AND gate.model = card.payload->'versions'->>'model'
                    )
                )",
    },
    Definition {
        id: "I11",
        name: "platform_time_not_after_first_observation",
        requires: &[
            "linggan_material_comment_platform_time_current",
            "linggan_opportunity_comment_fact",
        ],
        sql: "SELECT count(*) FROM linggan_material_comment_platform_time_current platform
              JOIN linggan_opportunity_comment_fact fact
                ON fact.content_ref = platform.content_public_ref
               AND fact.comment_external_id = platform.comment_external_id
              WHERE platform.published_at > fact.first_observed_at",
    },
];

pub async fn check_invariants(database: &Database) -> Result<InvariantReport, sqlx::Error> {
    let mut connection = database.pool().acquire().await?;
    check_on_connection(&mut connection, None).await
}

pub(super) async fn check_invariants_in(
    tx: &mut Transaction<'_, Postgres>,
    as_of: &str,
    domain: Uuid,
) -> Result<InvariantReport, sqlx::Error> {
    check_on_connection(tx, Some((as_of, domain))).await
}

fn as_of_sql(id: &str) -> Option<&'static str> {
    match id {
        "I1" => Some(
            "SELECT count(*) FROM linggan_opportunity_problem_metric_period metric JOIN linggan_opportunity_problem problem USING(problem_ref) WHERE metric.k > metric.n AND metric.computed_as_of <= $1::timestamptz AND problem.domain_ref=$2",
        ),
        "I2" => Some(
            "SELECT count(*) FROM linggan_opportunity_problem_metric_period metric JOIN linggan_opportunity_problem problem USING(problem_ref) WHERE metric.ci_low IS NOT NULL AND NOT (0 <= metric.ci_low AND metric.ci_low <= metric.ci_high AND metric.ci_high <= 1) AND metric.computed_as_of <= $1::timestamptz AND problem.domain_ref=$2",
        ),
        "I3" => Some(
            "SELECT count(*) FROM linggan_opportunity_problem_metric_period metric JOIN linggan_opportunity_problem problem USING(problem_ref) WHERE ((metric.n = 0 AND (metric.ci_low IS NOT NULL OR metric.ci_high IS NOT NULL)) OR (metric.n > 0 AND (metric.ci_low IS NULL OR metric.ci_high IS NULL))) AND metric.computed_as_of <= $1::timestamptz AND problem.domain_ref=$2",
        ),
        "I6" => Some(
            "SELECT count(*) FROM linggan_opportunity_comment_mention mention JOIN linggan_opportunity_problem problem USING(problem_ref) JOIN linggan_comment_problem_membership source ON source.request_ref=mention.source_request_ref AND source.signal_ordinal=mention.source_signal_ordinal LEFT JOIN linggan_opportunity_comment_fact_at($1::timestamptz) fact ON fact.content_ref=mention.content_ref AND fact.comment_external_id=mention.comment_external_id WHERE source.created_at <= $1::timestamptz AND problem.domain_ref=$2 AND fact.content_ref IS NULL",
        ),
        "I6b" => Some(
            "SELECT count(*) FROM linggan_opportunity_comment_mention mention JOIN linggan_opportunity_problem problem USING(problem_ref) JOIN linggan_comment_problem_membership source ON source.request_ref=mention.source_request_ref AND source.signal_ordinal=mention.source_signal_ordinal WHERE source.created_at <= $1::timestamptz AND problem.domain_ref=$2 AND (mention.commenter_hash IS NULL) <> (mention.excluded_reason IS NOT DISTINCT FROM 'commenter_unknown')",
        ),
        "I7" => Some(
            "SELECT count(*) FROM linggan_opportunity_creator_problem_density density JOIN linggan_opportunity_problem problem USING(problem_ref) JOIN linggan_opportunity_metric_definition definition ON definition.version=density.metric_def_version WHERE problem.domain_ref=$2 AND density.computed_as_of <= $1::timestamptz AND density.sufficient AND density.n < (definition.params->>'creator_min_n')::int",
        ),
        "I8" => Some(
            "WITH RECURSIVE walk(start_id,cur_id,depth) AS (SELECT lineage.from_problem_ref,lineage.to_problem_ref,1 FROM linggan_opportunity_problem_lineage lineage JOIN linggan_opportunity_problem problem ON problem.problem_ref=lineage.from_problem_ref WHERE problem.domain_ref=$2 AND lineage.relation IN ('merge','rename') AND lineage.created_at <= $1::timestamptz UNION ALL SELECT walk.start_id,lineage.to_problem_ref,walk.depth+1 FROM walk JOIN linggan_opportunity_problem_lineage lineage ON lineage.from_problem_ref=walk.cur_id AND lineage.created_at <= $1::timestamptz AND lineage.relation IN ('merge','rename') WHERE walk.depth < 100) SELECT count(*) FROM walk WHERE start_id=cur_id",
        ),
        "I9" => Some(
            "SELECT count(*) FROM linggan_opportunity_opportunity_card card JOIN linggan_opportunity_problem problem USING(problem_ref) WHERE problem.domain_ref=$2 AND card.as_of<=$1::timestamptz AND EXISTS (SELECT 1 FROM jsonb_array_elements(card.payload->'demand'->'sampleVoices') voice WHERE voice->>'text' IS NOT NULL AND EXISTS (SELECT 1 FROM linggan_material_comment_restriction restriction WHERE restriction.content_public_ref::text=voice->'commentRef'->>'contentRef' AND restriction.comment_external_id=voice->'commentRef'->>'commentExternalId'))",
        ),
        "I11" => Some(
            "SELECT count(*) FROM linggan_opportunity_platform_time_at($1::timestamptz) platform JOIN linggan_opportunity_comment_fact_at($1::timestamptz) fact ON fact.content_ref=platform.content_public_ref AND fact.comment_external_id=platform.comment_external_id WHERE platform.published_at > fact.first_observed_at AND EXISTS (SELECT 1 FROM linggan_material_domain_usage usage WHERE usage.content_public_ref=platform.content_public_ref AND usage.domain_ref=$2 AND (usage.basis_kind='legacy_domain_migration' OR usage.created_at <= $1::timestamptz))",
        ),
        _ => None,
    }
}

async fn check_on_connection(
    connection: &mut PgConnection,
    as_of: Option<(&str, Uuid)>,
) -> Result<InvariantReport, sqlx::Error> {
    let mut checks = Vec::with_capacity(DEFINITIONS.len());
    for definition in DEFINITIONS {
        let mut missing_tables = Vec::new();
        for table in definition.requires {
            let present: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
                .bind(*table)
                .fetch_one(&mut *connection)
                .await?;
            if !present {
                missing_tables.push(*table);
            }
        }
        if as_of.is_some() && as_of_sql(definition.id).is_none() {
            missing_tables.push("as_of_check_not_implemented");
        }
        let violations = if missing_tables.is_empty()
            && let (Some((as_of, domain)), Some(sql)) = (as_of, as_of_sql(definition.id))
        {
            sqlx::query_scalar::<_, i64>(sql)
                .bind(as_of)
                .bind(domain)
                .fetch_one(&mut *connection)
                .await?
        } else if missing_tables.is_empty() {
            sqlx::query_scalar::<_, i64>(definition.sql)
                .fetch_one(&mut *connection)
                .await?
        } else {
            0
        };
        checks.push(InvariantCheck {
            id: definition.id,
            name: definition.name,
            available: missing_tables.is_empty(),
            violations,
            missing_tables,
        });
    }
    let total_violations = checks.iter().map(|check| check.violations).sum();
    let incomplete = checks.iter().any(|check| !check.available);
    Ok(InvariantReport {
        checks,
        total_violations,
        incomplete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_covers_every_playbook_check_once() {
        let ids: Vec<&str> = DEFINITIONS.iter().map(|definition| definition.id).collect();
        assert_eq!(
            ids,
            vec![
                "I1", "I2", "I3", "I4", "I5", "I6", "I6b", "I7", "I8", "I9", "I10", "I11"
            ]
        );
        assert_eq!(ids.len(), ids.iter().collect::<HashSet<_>>().len());
        for definition in DEFINITIONS {
            assert!(!definition.name.is_empty());
            assert!(!definition.requires.is_empty());
            assert!(!definition.sql.is_empty());
        }
    }
}
