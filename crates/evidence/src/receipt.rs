//! What a caller is told about one delivery, and where its public refs come from.

use std::collections::VecDeque;
use std::sync::Mutex;

use uuid::Uuid;

/// The closed set of ingress results. There is no aggregate `ok` or `success`: acceptance,
/// replay, conflict and rejection stay distinguishable all the way out to the caller.
#[derive(Debug, PartialEq, Eq)]
pub enum IngressOutcome {
    Accepted {
        delivery_ref: Uuid,
        package_ref: Uuid,
        accepted_receipt_ref: Uuid,
    },
    Replay {
        delivery_ref: Uuid,
        package_ref: Uuid,
        accepted_receipt_ref: Uuid,
        original_accepted_delivery_ref: Uuid,
    },
    Conflict {
        delivery_ref: Uuid,
        package_ref: Uuid,
    },
    Rejected {
        delivery_ref: Uuid,
        code: RejectionCode,
    },
}

/// Why a routing-confirmed delivery was rejected. These reach the caller only after the routing
/// rows were resolved and locked, so they are safe to disclose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionCode {
    LeaseEpochMismatch,
    TargetMismatch,
    AuthorityRevoked,
    AuthorityExpired,
}

impl RejectionCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LeaseEpochMismatch => "lease_epoch_mismatch",
            Self::TargetMismatch => "target_mismatch",
            Self::AuthorityRevoked => "authority_revoked",
            Self::AuthorityExpired => "authority_expired",
        }
    }
}

/// Public refs are minted in memory before the transaction writes anything. Proof runs pin them
/// to the frozen fixture manifest so a run is reproducible; real runs mint random v4 UUIDs.
#[derive(Debug)]
pub(crate) struct RefSequence {
    fixed: Mutex<VecDeque<Uuid>>,
}

impl RefSequence {
    pub(crate) fn random() -> Self {
        Self {
            fixed: Mutex::new(VecDeque::new()),
        }
    }

    pub(crate) fn fixed(values: Vec<Uuid>) -> Self {
        Self {
            fixed: Mutex::new(values.into()),
        }
    }

    pub(crate) fn mint(&self) -> Uuid {
        self.fixed
            .lock()
            .expect("the ref sequence lock is never held across a panic")
            .pop_front()
            .unwrap_or_else(Uuid::new_v4)
    }
}
