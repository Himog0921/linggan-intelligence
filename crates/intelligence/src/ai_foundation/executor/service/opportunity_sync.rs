//! Feature-gated incremental consumption of already-published 0177 memberships.
//! A failed projection is retried by the existing automation tick and cannot roll back publication.

use crate::opportunity::{
    CardRebuildMode, CommenterDigest, RebuildMode, current_as_of, enqueue_source_events,
    load_current_metric_definition, rebuild_opportunity_cards, rebuild_problem_metrics,
    sync_comment_mentions_for_domain,
};
use linggan_storage_postgres::Database;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::{sync::Mutex, time::Instant};
use uuid::Uuid;

pub(super) struct OpportunitySync {
    pending: AtomicBool,
    failure_reported: AtomicBool,
    next_metrics: Mutex<Instant>,
}

impl OpportunitySync {
    pub(super) fn from_environment() -> Option<Arc<Self>> {
        if std::env::var("LINGGAN_OPPORTUNITY_CARD").as_deref() != Ok("enabled") {
            return None;
        }
        Some(Arc::new(Self {
            pending: AtomicBool::new(true),
            failure_reported: AtomicBool::new(false),
            next_metrics: Mutex::new(Instant::now()),
        }))
    }

    pub(super) fn request(&self) {
        self.pending.store(true, Ordering::Release);
    }

    pub(super) fn defer(&self, reason: &dyn std::fmt::Display) {
        self.request();
        if !self.failure_reported.swap(true, Ordering::AcqRel) {
            eprintln!("opportunity projection deferred: {reason}");
        }
    }

    pub(super) async fn run_if_pending(&self, database: &Database) {
        let requested = self.pending.swap(false, Ordering::AcqRel);
        let due = Instant::now() >= *self.next_metrics.lock().await;
        if let Err(error) = enqueue_source_events(database).await {
            self.defer(&error);
            return;
        }
        let card_domains = match self.due_card_domains(database).await {
            Ok(domains) => domains,
            Err(error) => {
                self.defer(&error);
                return;
            }
        };
        if !requested && !due && card_domains.is_empty() {
            return;
        }
        let digest = match CommenterDigest::load() {
            Ok(digest) => digest,
            Err(error) => {
                self.defer(&error);
                return;
            }
        };
        let result = async {
            let mut failures = Vec::new();
            if requested || due {
                if let Err(error) = self.refresh_metrics(database, &digest).await {
                    failures.push(error);
                }
            }
            for domain in card_domains {
                match rebuild_opportunity_cards(database, domain, &digest, CardRebuildMode::Due)
                    .await
                {
                    Ok(report) => eprintln!(
                        "opportunity card refresh: domain={domain} {}",
                        serde_json::to_string(&report).map_err(|error| error.to_string())?
                    ),
                    Err(error) => {
                        failures.push(format!("domain={domain} {error}"));
                        if !self.failure_reported.load(Ordering::Acquire) {
                            eprintln!("opportunity card refresh deferred: domain={domain} {error}");
                        }
                    }
                }
            }
            if failures.is_empty() {
                Ok(())
            } else {
                Err(failures.join("; "))
            }
        }
        .await;
        match result {
            Ok(()) => self.failure_reported.store(false, Ordering::Release),
            Err(error) => self.defer(&error),
        }
    }

    async fn due_card_domains(&self, database: &Database) -> Result<Vec<Uuid>, String> {
        sqlx::query_scalar("SELECT DISTINCT problem.domain_ref FROM linggan_opportunity_card_change change \
            JOIN linggan_opportunity_problem problem USING(problem_ref) \
            CROSS JOIN linggan_opportunity_metric_definition_current current \
            JOIN linggan_opportunity_metric_definition definition ON definition.version=current.version \
            WHERE current.singleton AND change.changed_at+make_interval(secs=>\
              (definition.params->>'card_rebuild_debounce_seconds')::double precision)<=clock_timestamp() \
            ORDER BY problem.domain_ref")
            .fetch_all(database.pool()).await.map_err(|error| error.to_string())
    }

    async fn sync_mentions(
        &self,
        database: &Database,
        digest: &CommenterDigest,
        domain: Uuid,
    ) -> Result<(), String> {
        match sync_comment_mentions_for_domain(database, digest, domain).await {
            Ok(report) => {
                let mismatches = report
                    .reconciliations
                    .iter()
                    .filter(|item| !item.reasons.is_empty())
                    .count();
                if mismatches != 0 {
                    eprintln!(
                        "opportunity mention reconciliation requires review: {mismatches} problems"
                    );
                }
                let conflicts = report.identity_conflicts();
                if conflicts != 0 {
                    eprintln!(
                        "opportunity commenter identity conflicts: {conflicts} comment-problem pairs"
                    );
                }
                Ok(())
            }
            Err(error) => Err(error.to_string()),
        }
    }

    async fn refresh_metrics(
        &self,
        database: &Database,
        digest: &CommenterDigest,
    ) -> Result<(), String> {
        let definition = load_current_metric_definition(database)
            .await
            .map_err(|e| e.to_string())?;
        let domains: Vec<Uuid> = sqlx::query_scalar(
            "SELECT domain_ref FROM linggan_comment_problem_definition UNION
             SELECT domain_ref FROM linggan_opportunity_problem ORDER BY domain_ref",
        )
        .fetch_all(database.pool())
        .await
        .map_err(|e| e.to_string())?;
        let mut failures = 0_usize;
        for domain in domains {
            if let Err(error) = self.sync_mentions(database, digest, domain).await {
                failures += 1;
                if !self.failure_reported.load(Ordering::Acquire) {
                    eprintln!("opportunity mention refresh deferred: domain={domain} {error}");
                }
                continue;
            }
            let as_of = current_as_of(database).await.map_err(|e| e.to_string())?;
            match rebuild_problem_metrics(
                database,
                domain,
                &as_of,
                digest,
                RebuildMode::Incremental,
            )
            .await
            {
                Ok(report) => {
                    let summary = serde_json::to_string(&report).map_err(|e| e.to_string())?;
                    eprintln!("opportunity metric refresh: domain={domain} {summary}");
                }
                Err(error) => {
                    failures += 1;
                    if !self.failure_reported.load(Ordering::Acquire) {
                        eprintln!("opportunity metric refresh deferred: domain={domain} {error}");
                    }
                }
            }
        }
        if failures != 0 {
            return Err(format!("{failures} domains need retry"));
        }
        *self.next_metrics.lock().await = Instant::now()
            + Duration::from_secs(definition.params.card_rebuild_debounce_seconds.into());
        Ok(())
    }
}
