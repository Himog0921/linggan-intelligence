//! Immutable source evidence. SCOPE-001 authorizes only synthetic Package/Record ingress after
//! the semantic code gate; real producer evidence remains out of scope.

mod acquisition_chain;
mod collection_target;
mod execution_station;
mod ingress;
mod local_discovery;
mod local_producer;
mod producer_runtime;
mod receipt;
mod work_order;

pub use acquisition_chain::{
    AcquisitionChainError, AuthorizationGrant, RequestOutcome, acquisition_chain_schema_is_ready,
    grant_authorization, request_and_admit,
};
pub use collection_target::{
    CollectionTargetError, ObservationTarget, StoreOutcome, collection_target_schema_is_ready,
    list_targets_in_state, store_pending_target, transition_target,
};
pub use execution_station::{
    CheckInOutcome, InstallationCheckIn, StationError, StationOverview, UnclaimedInstallation, check_in_installation, claim_installation,
    open_claim_window, read_station_overview, register_station, station_schema_is_ready,
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
