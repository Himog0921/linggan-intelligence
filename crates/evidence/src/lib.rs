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
mod material_admission;
mod material_asset_read;
mod material_contract_validation;
mod material_cursor;
mod material_detail_read;
mod material_disposition;
mod material_media;
mod material_media_read;
mod material_processing;
mod material_projection;
mod material_projection_types;
mod material_query_sql;
mod material_social_read;
mod material_storage_key;
mod patrol_scheduler;
mod producer_runtime;
mod receipt;
mod runtime_capacity;
mod station_read;
mod target_enrichment;
mod work_order;
mod work_order_lease;

pub use acquisition_chain::{
    AcquisitionChainError, AuthorizationGrant, RequestOutcome, acquisition_chain_schema_is_ready,
    grant_authorization, read_capacity, request_and_admit,
};
pub use archive_completeness::{ArchiveCompleteness, read_archive_completeness};
pub use collection_target::{
    CollectionTargetError, ObservationTarget, StoreOutcome, TargetCounts,
    collection_target_schema_is_ready, count_targets, list_targets, list_targets_in_state,
    read_target, store_pending_target, transition_target,
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
pub use material_asset_read::{
    LocalMaterialAsset, read_local_derivative, read_local_materialization,
};
pub use material_detail_read::read_material_detail;
pub use material_disposition::{
    MaterialMediaDisposition, record_blob_disposition, record_derivative_disposition,
    record_materialization_disposition, record_slot_disposition,
};
pub use material_processing::record_media_derivative_completion;
pub use material_projection::{
    MaterialDisplay, MaterialIdentity, MaterialLaneSummary, MaterialLibraryItem,
    MaterialLibraryProjection, MaterialPreview, MaterialReadError, MaterialSummary,
    material_projection_schema_is_ready, read_material_library,
};
pub use material_social_read::read_authorized_research_comments;
pub use patrol_scheduler::{
    PatrolTickSummary, patrol_schema_is_ready, run_due_patrols, set_group_for_many,
    set_monitoring_for_many, set_target_monitoring, target_monitoring_enabled,
};
pub use producer_runtime::{
    MediaBlobAdmission, MediaUploadFinalizeClaim, MediaUploadSession, ProducerRuntimeError,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, admit_media_blob,
    begin_media_upload, claim_media_upload_finalize, complete_media_upload, create_producer_task,
    producer_runtime_has_packages, producer_runtime_schema_is_ready, read_media_upload_session,
    read_runtime_library, record_media_download_failure, record_media_upload_chunk,
    release_media_upload_finalize, start_producer_attempt, submit_producer_package,
};
pub use receipt::{IngressOutcome, RejectionCode};
pub use runtime_capacity::{
    ACCOUNT_CHECK_NOT_CONNECTED, ActiveRiskPause, LaneVerdict, LiveLease, PatrolOutlook,
    RuntimeCapacityOverview, read_runtime_capacity,
};
pub use station_read::{
    StationOverview, UnclaimedInstallation, read_station_overview, station_daily_note_usage,
};
pub use target_enrichment::enrich_target_from_author_profile;
pub use work_order_lease::{
    DETAIL_STEP_DEFERRED_REASON, IssuedLease, LeaseError, expire_lapsed_leases,
    issue_work_order_lease, lease_schema_is_ready, release_work_order_lease,
};
