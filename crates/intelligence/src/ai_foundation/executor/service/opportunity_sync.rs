//! Feature-gated incremental consumption of already-published 0177 memberships.
//! A failed projection is retried by the existing automation tick and cannot roll back publication.

use crate::opportunity::{CommenterDigest, sync_comment_mentions};
use linggan_storage_postgres::Database;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct OpportunitySync {
    pending: AtomicBool,
    failure_reported: AtomicBool,
}

impl OpportunitySync {
    pub(super) fn from_environment() -> Option<Arc<Self>> {
        if std::env::var("LINGGAN_OPPORTUNITY_CARD").as_deref() != Ok("enabled") {
            return None;
        }
        Some(Arc::new(Self {
            pending: AtomicBool::new(true),
            failure_reported: AtomicBool::new(false),
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
        if !self.pending.swap(false, Ordering::AcqRel) {
            return;
        }
        let digest = match CommenterDigest::load() {
            Ok(digest) => digest,
            Err(error) => {
                self.defer(&error);
                return;
            }
        };
        match sync_comment_mentions(database, &digest).await {
            Ok(report) => {
                self.failure_reported.store(false, Ordering::Release);
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
            }
            Err(error) => self.defer(&error),
        }
    }
}
