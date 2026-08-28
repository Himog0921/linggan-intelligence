//! Immutable source evidence. SCOPE-001 authorizes only synthetic Package/Record ingress after
//! the semantic code gate; real producer evidence remains out of scope.

mod acquisition_chain;
mod archive_completeness;
mod collection_target;
mod dispatch;
mod execution_station;
mod ingress;
mod local_discovery;
mod local_producer;
mod patrol_scheduler;
mod producer_runtime;
mod receipt;
mod station_read;
mod target_enrichment;
mod work_order;
mod work_order_lease;

pub use acquisition_chain::{
    AcquisitionChainError, AuthorizationGrant, RequestOutcome, acquisition_chain_schema_is_ready,
    grant_authorization, request_and_admit,
};
pub use archive_completeness::{ArchiveCompleteness, read_archive_completeness};
pub use collection_target::{
    CollectionTargetError, ObservationTarget, StoreOutcome, collection_target_schema_is_ready,
    list_targets, list_targets_in_state, store_pending_target, transition_target,
};
pub use dispatch::{DispatchDecision, DispatchError, decide_dispatch, dispatch_schema_is_ready};
pub use execution_station::{
    CheckInOutcome, InstallationCheckIn, StationError, check_in_installation, claim_installation,
    close_claim_window, open_claim_window, register_station, retire_station,
    station_schema_is_ready,
};
pub use ingress::{
    IngressError, IngressFault, IngressOptions, PreRoutingCode, ingest_capture_package,
    ingest_capture_package_with,
};
pub use local_discovery::{
    DiscoveryIngressError, DiscoveryIngressOutcome, DiscoveryLibraryCard,
    DiscoveryLibraryProjection, ingest_discovery_package, local_discovery_schema_is_ready,
    read_discovery_library,
};
pub use local_producer::{
    LocalAttemptOutcome, LocalProducerError, LocalSubmissionOutcome, LocalTaskOutcome,
    create_manual_task, local_producer_schema_is_ready, start_local_attempt, submit_local_package,
};
pub use patrol_scheduler::{
    PatrolTickSummary, patrol_schema_is_ready, run_due_patrols, set_target_monitoring,
    target_monitoring_enabled,
};
pub use producer_runtime::{
    MediaBlobAdmission, MediaUploadFinalizeClaim, MediaUploadSession, ProducerRuntimeError,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, admit_media_blob,
    begin_media_upload, claim_media_upload_finalize, complete_media_upload, create_producer_task,
    producer_runtime_has_packages, producer_runtime_schema_is_ready, read_local_media_blob,
    read_media_upload_session, read_runtime_library, record_media_download_failure,
    record_media_upload_chunk, release_media_upload_finalize, start_producer_attempt,
    submit_producer_package,
};
pub use receipt::{IngressOutcome, RejectionCode};
pub use station_read::{
    StationOverview, UnclaimedInstallation, read_station_overview, station_daily_note_usage,
};
pub use target_enrichment::enrich_target_from_author_profile;
pub use work_order_lease::{
    DETAIL_STEP_DEFERRED_REASON, IssuedLease, LeaseError, complete_lease_for_task,
    expire_lapsed_leases, issue_work_order_lease, lease_schema_is_ready, release_work_order_lease,
};
