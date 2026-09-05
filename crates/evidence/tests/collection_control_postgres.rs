use linggan_contracts::{AdmissionOutcome, Capacity};
use linggan_evidence::{
    AccountEligibilitySignal, AccountEligibilityState, AuthorizationGrant, CheckInOutcome,
    CollectionControlError, ComparableObservationRound, DispatchDecision, DynamicCadence,
    InstallationCheckIn, MAXIMUM_MONITOR_INTERVAL_SECONDS, MINIMUM_MONITOR_INTERVAL_SECONDS,
    MonitorCommandActor, MonitorCommandKind, MonitorCommandOutcomeKind, MonitorRuleCommand,
    MonitorRuleDraft, MonitorRuleMode, activate_installation_credential,
    apply_monitor_rule_command, bind_observation_account, check_in_installation,
    collection_control_schema_is_ready, decide_dispatch, dynamic_cadence, grant_authorization,
    open_claim_window, read_capacity, register_station, release_work_order_lease, rename_station,
    report_account_eligibility, request_and_admit, retire_station, rotate_installation_credential,
    set_station_accepting,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;
use uuid::Uuid;

const DIGEST_KEY: &[u8] = b"collection-control-proof-digest-key-v1";

const MIGRATIONS: &str = concat!(
    include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    include_str!("../../../database/migrations/0005_collection_observation_target.sql"),
    "\n",
    include_str!("../../../database/migrations/0006_collection_acquisition_chain.sql"),
    "\n",
    include_str!("../../../database/migrations/0007_execution_station.sql"),
    "\n",
    include_str!("../../../database/migrations/0008_collection_risk_pause.sql"),
    "\n",
    include_str!("../../../database/migrations/0009_work_order_station.sql"),
    "\n",
    include_str!("../../../database/migrations/0010_work_order_lease.sql"),
    "\n",
    include_str!("../../../database/migrations/0011_execution_gate.sql"),
    "\n",
    include_str!("../../../database/migrations/0012_target_monitor_schedule.sql"),
    "\n",
    include_str!("../../../database/migrations/0013_drop_execution_gate.sql"),
    "\n",
    include_str!("../../../database/migrations/0014_target_group.sql"),
    "\n",
    include_str!("../../../database/migrations/0015_material_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0016_material_social_lanes.sql"),
    "\n",
    include_str!("../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0018_material_discovery_lane.sql"),
    "\n",
    include_str!("../../../database/migrations/0019_work_order_lease_task_sequence.sql"),
    "\n",
    include_str!("../../../database/migrations/0020_observation_runtime_automation.sql"),
    "\n",
    include_str!("../../../database/migrations/0021_discovery_cover_media_acquisition.sql"),
    "\n",
    include_str!("../../../database/migrations/0022_material_deepening_scope.sql"),
    "\n",
    include_str!("../../../database/migrations/0023_material_engagement_and_media_components.sql"),
    "\n",
    include_str!("../../../database/migrations/0024_media_processing_runtime.sql"),
    "\n",
    include_str!("../../../database/migrations/0025_comment_current_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0026_work_resource_read.sql"),
    "\n",
    include_str!("../../../database/migrations/0027_unified_media_resource.sql"),
    "\n",
    include_str!("../../../database/migrations/0029_author_avatar_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0030_comment_image_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0031_topic_workspace.sql"),
    "\n",
    include_str!("../../../database/migrations/0032_author_profile_avatar_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0033_dispatch_failure_recovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0034_collection_control_closure.sql"),
    "\n",
    include_str!("../../../database/migrations/0035_claimed_station_auto_acceptance.sql"),
    "\n",
    include_str!("../../../database/migrations/0036_monitor_scheduling_clarity.sql"),
);

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn complete_migration_set_applies_collection_control_0036() {
    let database = proof_database("collection_control_full_migrations").await;

    assert!(
        collection_control_schema_is_ready(&database)
            .await
            .expect("schema readiness is readable")
    );
    let relations: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT to_regclass(name)::text FROM unnest(ARRAY[ \
             'installation_credential', \
             'platform_observation_account', \
             'platform_observation_account_binding', \
             'platform_observation_account_eligibility_observation', \
             'collection_monitor_rule_revision', \
             'collection_monitor_rule_command_receipt', \
             'collection_scheduler_target_decision' \
         ]) AS name",
    )
    .fetch_all(database.pool())
    .await
    .expect("0034 relations are inspectable");
    assert!(relations.iter().all(Option::is_some));

    let accepting_default: String = sqlx::query_scalar(
        "SELECT column_default FROM information_schema.columns \
         WHERE table_schema=current_schema() AND table_name='execution_station' \
           AND column_name='accepting_tasks'",
    )
    .fetch_one(database.pool())
    .await
    .expect("station acceptance gate exists");
    assert_eq!(accepting_default, "false");
    let transition_constraint: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint \
         WHERE conname='execution_station_acceptance_transition_reason_code_check')",
    )
    .fetch_one(database.pool())
    .await
    .expect("0035 transition contract remains inspectable after 0036");
    assert!(transition_constraint);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn claimed_installation_auto_accepts_but_a_person_pause_survives_replacement() {
    let database = proof_database("control_claim_auto_accept").await;
    let station_ref = register_station(&database, "临时工位名", 200)
        .await
        .expect("person registers the station");
    rename_station(&database, station_ref, "本机 Chrome")
        .await
        .expect("person sets the canonical station name in Runtime");
    open_claim_window(&database, station_ref, 1)
        .await
        .expect("claim window opens");

    let first = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: "auto-accept-first",
            installation_credential: None,
            plugin_version: "0.8.36",
            browser_label: Some("Chrome"),
            capabilities: serde_json::json!(["author_profile"]),
        },
    )
    .await
    .expect("claimed installation checks in");
    let first_installation = match first {
        CheckInOutcome::Claimed {
            installation_ref,
            station_ref: claimed_station_ref,
            station_display_name,
            accepting_tasks,
            ..
        } => {
            assert_eq!(claimed_station_ref, station_ref);
            assert_eq!(station_display_name, "本机 Chrome");
            assert!(accepting_tasks, "claim enables the default acceptance mode");
            installation_ref
        }
        other => panic!("claim window must match the first installation; got {other:?}"),
    };
    let accepting: bool =
        sqlx::query_scalar("SELECT accepting_tasks FROM execution_station WHERE station_ref=$1")
            .bind(station_ref)
            .fetch_one(database.pool())
            .await
            .expect("acceptance state is readable");
    assert!(accepting);

    set_station_accepting(&database, station_ref, false, "person")
        .await
        .expect("person can explicitly pause future work");
    let replacement = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: "auto-accept-replacement",
            installation_credential: None,
            plugin_version: "0.8.36",
            browser_label: Some("Chrome"),
            capabilities: serde_json::json!(["author_profile"]),
        },
    )
    .await
    .expect("replacement checks in through the still-open claim window");
    match replacement {
        CheckInOutcome::Claimed {
            station_display_name,
            accepting_tasks,
            superseded,
            ..
        } => {
            assert_eq!(station_display_name, "本机 Chrome");
            assert!(
                !accepting_tasks,
                "a replacement must not undo person_disabled"
            );
            assert_eq!(superseded, Some(first_installation));
        }
        other => panic!("replacement must remain matched; got {other:?}"),
    }
    let latest_reason: String = sqlx::query_scalar(
        "SELECT reason_code FROM execution_station_acceptance_transition \
         WHERE station_ref=$1 ORDER BY occurred_at DESC,transition_ref DESC LIMIT 1",
    )
    .bind(station_ref)
    .fetch_one(database.pool())
    .await
    .expect("acceptance audit trail is readable");
    assert_eq!(latest_reason, "person_disabled");
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn legacy_registered_closed_default_also_auto_accepts_when_claimed() {
    let database = proof_database("control_legacy_claim_default").await;
    let station_ref = register_station(&database, "旧版本机 Chrome", 200)
        .await
        .expect("person registers the legacy default station");
    sqlx::query(
        "UPDATE execution_station_acceptance_transition \
         SET reason_code='registered_closed' WHERE station_ref=$1",
    )
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("fixture represents the pre-0035 registered default");
    open_claim_window(&database, station_ref, 1)
        .await
        .expect("claim window opens");

    let outcome = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: "legacy-default-claim",
            installation_credential: None,
            plugin_version: "0.8.36",
            browser_label: Some("Chrome"),
            capabilities: serde_json::json!(["author_profile"]),
        },
    )
    .await
    .expect("legacy default station checks in");
    match outcome {
        CheckInOutcome::Claimed {
            station_display_name,
            accepting_tasks,
            ..
        } => {
            assert_eq!(station_display_name, "旧版本机 Chrome");
            assert!(
                accepting_tasks,
                "the historical default was never a person pause"
            );
        }
        other => panic!("claim window must match the legacy station; got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn account_identity_and_installation_credential_are_hash_only() {
    let database = proof_database("collection_control_hash_only").await;
    let installation = install(&database, "hash-only", "0.8.34").await;
    let raw_credential = installation
        .credential
        .as_deref()
        .expect("compatible check-in returns one credential");
    let raw_account_id = "raw-platform-account-id-must-not-persist";

    let receipt = report_account_eligibility(
        &database,
        installation.installation_ref,
        raw_credential,
        Some(raw_account_id),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("bounded account fact is accepted");
    assert_eq!(receipt.state, AccountEligibilityState::Usable);
    assert!(receipt.binding_required);

    let credential_row = sqlx::query(
        "SELECT credential_hash,hash_version FROM installation_credential \
         WHERE installation_ref=$1 AND revoked_at IS NULL",
    )
    .bind(installation.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("credential digest is persisted");
    let credential_hash: String = credential_row.get("credential_hash");
    assert_eq!(credential_hash.len(), 64);
    assert_eq!(credential_row.get::<String, _>("hash_version"), "sha256-v1");
    assert_ne!(credential_hash, raw_credential);

    let account_row = sqlx::query(
        "SELECT identity_digest,digest_version FROM platform_observation_account \
         WHERE account_ref=$1",
    )
    .bind(
        receipt
            .account_ref
            .expect("server returns the stable account ref"),
    )
    .fetch_one(database.pool())
    .await
    .expect("account digest is persisted");
    let identity_digest: String = account_row.get("identity_digest");
    assert_eq!(identity_digest.len(), 64);
    assert_eq!(
        account_row.get::<String, _>("digest_version"),
        "hmac-sha256-v1"
    );
    assert_ne!(identity_digest, raw_account_id);

    let persisted_control_rows: String = sqlx::query_scalar(
        "SELECT concat_ws('', \
             COALESCE((SELECT string_agg(to_jsonb(row_value)::text,'') \
                       FROM installation_credential row_value),''), \
             COALESCE((SELECT string_agg(to_jsonb(row_value)::text,'') \
                       FROM platform_observation_account row_value),''), \
             COALESCE((SELECT string_agg(to_jsonb(row_value)::text,'') \
                       FROM platform_observation_account_eligibility_observation row_value),'') \
         )",
    )
    .fetch_one(database.pool())
    .await
    .expect("bounded control persistence is inspectable");
    assert!(!persisted_control_rows.contains(raw_credential));
    assert!(!persisted_control_rows.contains(raw_account_id));

    let forbidden_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns \
         WHERE table_schema=current_schema() \
           AND table_name IN ( \
               'installation_credential','platform_observation_account', \
               'platform_observation_account_binding', \
               'platform_observation_account_eligibility_observation') \
           AND (column_name LIKE '%raw%' OR column_name LIKE '%cookie%' \
                OR column_name LIKE '%token%' OR column_name LIKE '%password%' \
                OR column_name LIKE '%html%')",
    )
    .fetch_one(database.pool())
    .await
    .expect("privacy schema is inspectable");
    assert_eq!(forbidden_columns, 0);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn credentials_require_compatible_version_and_rotation_activates_before_revoking_old() {
    let database = proof_database("collection_control_credentials").await;
    let legacy = install(&database, "legacy-credential", "0.8.33").await;
    assert!(legacy.credential.is_none());
    assert!(
        rotate_installation_credential(&database, legacy.installation_ref)
            .await
            .is_err(),
        "an explicit rotation cannot bypass the minimum plugin version"
    );
    let legacy_credential_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM installation_credential WHERE installation_ref=$1",
    )
    .bind(legacy.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("legacy credential absence is readable");
    assert_eq!(legacy_credential_count, 0);

    let current = install(&database, "current-credential", "0.8.34").await;
    let first_secret = current
        .credential
        .as_deref()
        .expect("minimum version receives a one-time secret")
        .to_owned();
    let rotated = rotate_installation_credential(&database, current.installation_ref)
        .await
        .expect("compatible installation rotates its credential");
    let second_secret = rotated.raw_credential.expose_once().to_owned();
    assert_ne!(first_secret, second_secret);
    let debug = format!("{rotated:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains(&second_secret));

    report_account_eligibility(
        &database,
        current.installation_ref,
        &first_secret,
        Some("rotation-proof-account"),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("the old active credential remains usable while rotation is pending");
    assert!(matches!(
        report_account_eligibility(
            &database,
            current.installation_ref,
            &second_secret,
            None,
            AccountEligibilitySignal::AuthenticatedObserved,
            DIGEST_KEY,
        )
        .await,
        Err(CollectionControlError::InvalidCredential)
    ));
    activate_installation_credential(
        &database,
        current.installation_ref,
        rotated.credential_ref,
        &second_secret,
    )
    .await
    .expect("the pending rotation activates from its raw hash acknowledgement");
    assert!(matches!(
        report_account_eligibility(
            &database,
            current.installation_ref,
            &first_secret,
            None,
            AccountEligibilitySignal::AuthenticatedObserved,
            DIGEST_KEY,
        )
        .await,
        Err(CollectionControlError::InvalidCredential)
    ));
    report_account_eligibility(
        &database,
        current.installation_ref,
        &second_secret,
        Some("rotation-proof-account"),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("rotated credential is the only usable secret");

    let rows = sqlx::query(
        "SELECT activated_at IS NOT NULL AND revoked_at IS NULL AS active,revoke_reason_code \
         FROM installation_credential WHERE installation_ref=$1 ORDER BY issued_at",
    )
    .bind(current.installation_ref)
    .fetch_all(database.pool())
    .await
    .expect("credential history is readable");
    assert_eq!(rows.len(), 2);
    assert!(!rows[0].get::<bool, _>("active"));
    assert_eq!(
        rows[0]
            .get::<Option<String>, _>("revoke_reason_code")
            .as_deref(),
        Some("rotated")
    );
    assert!(rows[1].get::<bool, _>("active"));
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn account_binding_is_one_to_one_and_expiry_fails_closed() {
    let database = proof_database("collection_control_binding").await;
    let first = ready_installation_without_account(&database, "binding-first").await;
    let second = ready_installation_without_account(&database, "binding-second").await;
    let first_secret = first.credential.as_deref().expect("first secret exists");
    let second_secret = second.credential.as_deref().expect("second secret exists");

    let first_observation = report_account_eligibility(
        &database,
        first.installation_ref,
        first_secret,
        Some("same-account-across-installations"),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("first installation reports the account");
    let account_ref = first_observation.account_ref.expect("account ref exists");
    bind_observation_account(&database, account_ref, first.installation_ref, "person")
        .await
        .expect("person confirms the first binding");

    let second_observation = report_account_eligibility(
        &database,
        second.installation_ref,
        second_secret,
        Some("same-account-across-installations"),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("second installation reports the same account");
    assert_eq!(second_observation.account_ref, Some(account_ref));
    bind_observation_account(&database, account_ref, second.installation_ref, "person")
        .await
        .expect("moving the account ends the old binding");

    let active_bindings: Vec<Uuid> = sqlx::query_scalar(
        "SELECT installation_ref FROM platform_observation_account_binding \
         WHERE account_ref=$1 AND ended_at IS NULL",
    )
    .bind(account_ref)
    .fetch_all(database.pool())
    .await
    .expect("active bindings are readable");
    assert_eq!(active_bindings, vec![second.installation_ref]);
    let ended_reason: String = sqlx::query_scalar(
        "SELECT end_reason_code FROM platform_observation_account_binding \
         WHERE account_ref=$1 AND installation_ref=$2 AND ended_at IS NOT NULL",
    )
    .bind(account_ref)
    .bind(first.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("old binding history is retained");
    assert_eq!(ended_reason, "person_unbound");

    retire_station(&database, first.station_ref, "binding proof complete")
        .await
        .expect("the earlier station no longer competes in capacity selection");
    sqlx::query(
        "UPDATE platform_observation_account_binding \
         SET bound_at=scope_001_now()-interval '31 days', \
             confirmed_until=scope_001_now()-interval '1 day' \
         WHERE account_ref=$1 AND installation_ref=$2 AND ended_at IS NULL",
    )
    .bind(account_ref)
    .bind(second.installation_ref)
    .execute(database.pool())
    .await
    .expect("proof clock moves the confirmation beyond its validity window");
    assert_capacity_reason(&database, "account_binding_expired").await;
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn server_projects_each_closed_account_signal_into_capacity() {
    let database = proof_database("collection_control_signal_projection").await;
    let installation = ready_installation_without_account(&database, "signal-projection").await;
    let secret = installation
        .credential
        .as_deref()
        .expect("compatible installation has a secret");
    let initial = report_account_eligibility(
        &database,
        installation.installation_ref,
        secret,
        Some("signal-projection-account"),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("account identity is established");
    let account_ref = initial.account_ref.expect("account ref exists");
    bind_observation_account(
        &database,
        account_ref,
        installation.installation_ref,
        "person",
    )
    .await
    .expect("person confirms the account binding");

    let cases = [
        (
            AccountEligibilitySignal::AuthenticatedObserved,
            AccountEligibilityState::Usable,
            "available",
        ),
        (
            AccountEligibilitySignal::CooldownObserved,
            AccountEligibilityState::Cooling,
            "account_cooling",
        ),
        (
            AccountEligibilitySignal::LoginRequired,
            AccountEligibilityState::NeedsLogin,
            "account_needs_login",
        ),
        (
            AccountEligibilitySignal::AccessRestricted,
            AccountEligibilityState::Restricted,
            "account_restricted",
        ),
        (
            AccountEligibilitySignal::SignalIncomplete,
            AccountEligibilityState::Unknown,
            "account_unknown",
        ),
    ];
    for (signal, expected_state, expected_capacity) in cases {
        let receipt = report_account_eligibility(
            &database,
            installation.installation_ref,
            secret,
            None,
            signal,
            DIGEST_KEY,
        )
        .await
        .expect("closed producer signal is accepted");
        assert_eq!(receipt.state, expected_state);
        assert_capacity_reason(&database, expected_capacity).await;
    }
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn unified_capacity_exposes_distinct_recoverable_reasons() {
    let database = proof_database("collection_control_capacity_reasons").await;
    let installation = install(&database, "capacity-reasons", "0.8.34").await;
    let original_secret = installation
        .credential
        .as_deref()
        .expect("current installation has a credential")
        .to_owned();

    set_station_accepting(&database, installation.station_ref, false, "person")
        .await
        .expect("person explicitly pauses the otherwise automatic station");
    assert_capacity_reason(&database, "station_not_accepting").await;
    set_station_accepting(&database, installation.station_ref, true, "person")
        .await
        .expect("person opens the station gate");
    sqlx::query("DELETE FROM installation_credential WHERE installation_ref=$1")
        .bind(installation.installation_ref)
        .execute(database.pool())
        .await
        .expect("proof removes the credential fact");
    assert_capacity_reason(&database, "installation_credential_missing").await;

    let restored = rotate_installation_credential(&database, installation.installation_ref)
        .await
        .expect("compatible installation receives a replacement credential");
    let secret = restored.raw_credential.expose_once().to_owned();
    assert_ne!(secret, original_secret);
    activate_installation_credential(
        &database,
        installation.installation_ref,
        restored.credential_ref,
        &secret,
    )
    .await
    .expect("replacement credential activates before later capacity gates are tested");
    sqlx::query("UPDATE plugin_installation SET plugin_version='0.8.33' WHERE installation_ref=$1")
        .bind(installation.installation_ref)
        .execute(database.pool())
        .await
        .expect("proof selects an unsupported version");
    assert_capacity_reason(&database, "plugin_version_unsupported").await;

    sqlx::query(
        "UPDATE plugin_installation SET plugin_version='0.8.34', \
             last_seen_at=scope_001_now()-interval '21 minutes' WHERE installation_ref=$1",
    )
    .bind(installation.installation_ref)
    .execute(database.pool())
    .await
    .expect("proof makes the installation stale");
    assert_capacity_reason(&database, "installation_stale").await;

    sqlx::query(
        "UPDATE plugin_installation SET last_seen_at=scope_001_now() WHERE installation_ref=$1",
    )
    .bind(installation.installation_ref)
    .execute(database.pool())
    .await
    .expect("proof restores installation freshness");
    assert_capacity_reason(&database, "account_unbound").await;

    let observation = report_account_eligibility(
        &database,
        installation.installation_ref,
        &secret,
        Some("capacity-reason-account"),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("account fact is reported");
    let account_ref = observation.account_ref.expect("account ref exists");
    bind_observation_account(
        &database,
        account_ref,
        installation.installation_ref,
        "person",
    )
    .await
    .expect("person binds the account");
    assert_capacity_reason(&database, "available").await;

    sqlx::query(
        "UPDATE platform_observation_account_eligibility_observation \
         SET observed_at=scope_001_now()-interval '21 minutes', \
             expires_at=scope_001_now()-interval '1 second' \
         WHERE account_ref=$1 AND installation_ref=$2",
    )
    .bind(account_ref)
    .bind(installation.installation_ref)
    .execute(database.pool())
    .await
    .expect("proof expires account eligibility");
    assert_capacity_reason(&database, "account_eligibility_stale").await;
    report_account_eligibility(
        &database,
        installation.installation_ref,
        &secret,
        None,
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("fresh eligibility is restored");

    let target_ref = seed_target(&database, "creator", "pending_decision", "capacity-busy").await;
    grant_authorization(
        &database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "deep_archive",
            purpose: "capacity busy proof",
            max_targets: Some(1),
            max_works_per_target: Some(20),
            valid_for_days: 1,
        },
    )
    .await
    .expect("bounded authorization is granted");
    let admitted = request_and_admit(
        &database,
        target_ref,
        "deep_archive",
        "capacity busy proof",
        "person",
    )
    .await
    .expect("ready capacity admits one work order");
    assert!(matches!(
        admitted.outcome,
        AdmissionOutcome::Admitted { .. }
    ));
    assert!(
        admitted.work_order_ref.is_some(),
        "admission creates queued work"
    );
    let lease_ref = match decide_dispatch(&database, &installation.install_key, &secret)
        .await
        .expect("the claimed installation atomically selects and claims queued work")
    {
        DispatchDecision::Dispatch { lease_ref, .. } => lease_ref,
        other => panic!("ready claimed installation must receive the queued work; got {other:?}"),
    };
    assert_capacity_reason(&database, "account_busy").await;
    release_work_order_lease(&database, lease_ref, "revoked")
        .await
        .expect("proof releases the busy lease");

    seed_accepted_daily_notes(&database, &installation, 200).await;
    assert_capacity_reason(&database, "station_daily_budget_reached").await;
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn authorization_is_bounded_by_scope_and_targets_not_purpose_text() {
    let database = proof_database("collection_control_authorization_bounds").await;
    let installation = ready_installation_without_account(&database, "authorization-bounds").await;
    make_account_usable(&database, &installation, "authorization-account").await;
    let authorization_ref = grant_authorization(
        &database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "deep_archive",
            purpose: "bounded baseline",
            max_targets: Some(2),
            max_works_per_target: Some(20),
            valid_for_days: 1,
        },
    )
    .await
    .expect("one-target authorization is granted");
    let first_target = seed_target(&database, "creator", "pending_decision", "bound-first").await;
    let second_target = seed_target(&database, "creator", "pending_decision", "bound-second").await;

    let (first, second) = tokio::join!(
        request_and_admit(
            &database,
            first_target,
            "deep_archive",
            "first audit annotation",
            "person"
        ),
        request_and_admit(
            &database,
            second_target,
            "deep_archive",
            "different audit annotation",
            "person"
        )
    );
    let outcomes = [
        first.expect("first concurrent admission decides"),
        second.expect("second concurrent admission decides"),
    ];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| outcome.work_order_ref.is_some())
            .count(),
        2,
        "purpose is audit text; two in-scope targets are admitted within the explicit bound"
    );
    let authorized_work_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order work_order \
         JOIN collection_admission_decision decision USING(decision_ref) \
         WHERE decision.authorization_ref=$1",
    )
    .bind(authorization_ref)
    .fetch_one(database.pool())
    .await
    .expect("authorized work count is readable");
    assert_eq!(authorized_work_count, 2);

    let bounded_target =
        seed_target(&database, "creator", "pending_decision", "target-limit").await;
    let bounded = request_and_admit(
        &database,
        bounded_target,
        "deep_archive",
        "third audit annotation",
        "person",
    )
    .await
    .expect("target limit produces a durable decision");
    assert!(bounded.work_order_ref.is_none());
    let bounded_reason: String = sqlx::query_scalar(
        "SELECT reason_code FROM collection_admission_decision WHERE decision_ref=$1",
    )
    .bind(bounded.decision_ref)
    .fetch_one(database.pool())
    .await
    .expect("target-limit reason is readable");
    assert_eq!(bounded_reason, "authorization_target_limit_reached");
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn monitor_rule_commands_are_revisioned_idempotent_and_side_effect_bounded() {
    let database = proof_database("collection_control_monitor_rules").await;
    let keyword_ref = seed_target(&database, "keyword", "pending_decision", "adhd-family").await;
    let command = MonitorRuleCommand {
        target_ref: keyword_ref,
        expected_revision: 0,
        idempotency_key: Uuid::new_v4(),
        kind: MonitorCommandKind::SaveRule,
        actor: MonitorCommandActor::Person,
        source: "targets_ui",
        draft: Some(fixed_rule(Some("hot"))),
    };

    let applied = apply_monitor_rule_command(&database, &command)
        .await
        .expect("valid keyword rule is applied");
    assert_eq!(applied.outcome, MonitorCommandOutcomeKind::Applied);
    assert_eq!(applied.reason_code, "rule_saved");
    assert_eq!(applied.current_revision, 1);
    assert!(applied.applied_rule_revision_ref.is_some());

    let replay = apply_monitor_rule_command(&database, &command)
        .await
        .expect("same identity and payload replays");
    assert_eq!(replay.outcome, MonitorCommandOutcomeKind::Replay);
    assert_eq!(
        replay.applied_rule_revision_ref,
        applied.applied_rule_revision_ref
    );

    let mut conflict = command.clone();
    conflict
        .draft
        .as_mut()
        .expect("draft exists")
        .fixed_interval_seconds = Some(43_201);
    let conflict = apply_monitor_rule_command(&database, &conflict)
        .await
        .expect("identity conflict is a durable command result");
    assert_eq!(
        conflict.outcome,
        MonitorCommandOutcomeKind::IdentityConflict
    );
    assert_eq!(conflict.reason_code, "identity_conflict");

    let stale = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            idempotency_key: Uuid::new_v4(),
            ..command.clone()
        },
    )
    .await
    .expect("stale revision is a durable command result");
    assert_eq!(stale.outcome, MonitorCommandOutcomeKind::StaleRevision);
    assert_eq!(stale.current_revision, 1);

    let invalid_target =
        seed_target(&database, "keyword", "pending_decision", "invalid-window").await;
    let mut invalid_rule = fixed_rule(None);
    invalid_rule.all_day = false;
    invalid_rule.window_start_minute = Some(1_320);
    invalid_rule.window_end_minute = Some(360);
    let invalid = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref: invalid_target,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::SaveRule,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            draft: Some(invalid_rule),
        },
    )
    .await
    .expect("cross-midnight window is durably rejected");
    assert_eq!(invalid.outcome, MonitorCommandOutcomeKind::Rejected);
    assert_eq!(invalid.reason_code, "invalid_schedule");

    let unsupported_interval_target = seed_target(
        &database,
        "keyword",
        "pending_decision",
        "unsupported-interval",
    )
    .await;
    let mut unsupported_interval = fixed_rule(None);
    unsupported_interval.fixed_interval_seconds = Some(43_201);
    unsupported_interval.fallback_interval_seconds = 43_201;
    let unsupported_interval = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref: unsupported_interval_target,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::SaveRule,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            draft: Some(unsupported_interval),
        },
    )
    .await
    .expect("an unsupported fixed interval is durably rejected");
    assert_eq!(
        unsupported_interval.outcome,
        MonitorCommandOutcomeKind::Rejected
    );
    assert_eq!(unsupported_interval.reason_code, "invalid_interval");

    let creator_ref = seed_target(
        &database,
        "creator",
        "pending_decision",
        "creator-no-baseline",
    )
    .await;
    let creator = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref: creator_ref,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::SaveRule,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            draft: Some(fixed_rule(None)),
        },
    )
    .await
    .expect("creator observation rule does not require a historic baseline");
    assert_eq!(creator.outcome, MonitorCommandOutcomeKind::Applied);
    assert_eq!(creator.reason_code, "rule_saved");
    let creator_schedule: (String, bool, Option<String>) = sqlx::query_as(
        "SELECT lifecycle_state,monitoring_enabled,monitor_next_run_at::text \
         FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(creator_ref)
    .fetch_one(database.pool())
    .await
    .expect("creator rule and schedule update atomically");
    assert_eq!(creator_schedule.0, "monitoring");
    assert!(creator_schedule.1);
    assert!(creator_schedule.2.is_some());

    let side_effect_counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM collection_work_order), \
                (SELECT count(*) FROM collection_work_order_lease), \
                (SELECT count(*) FROM linggan_runtime_attempt)",
    )
    .fetch_one(database.pool())
    .await
    .expect("execution side effects are inspectable");
    assert_eq!(side_effect_counts, (0, 0, 0));
    let keyword_state: (String, bool) = sqlx::query_as(
        "SELECT lifecycle_state,monitoring_enabled FROM collection_observation_target \
         WHERE target_ref=$1",
    )
    .bind(keyword_ref)
    .fetch_one(database.pool())
    .await
    .expect("keyword target state is readable");
    assert_eq!(keyword_state, ("monitoring".to_owned(), true));
}

#[test]
fn dynamic_cadence_requires_distinct_comparable_rounds_and_clamps_median_half() {
    let account_ref = Uuid::new_v4();
    let rule_revision_ref = Uuid::new_v4();
    let first_round_ref = Uuid::new_v4();
    let first = comparable_round(first_round_ref, rule_revision_ref, account_ref, 0);
    let duplicate_round = comparable_round(first_round_ref, rule_revision_ref, account_ref, 86_400);
    assert_eq!(
        dynamic_cadence(
            &[first.clone(), duplicate_round],
            "creator_profile",
            Some("default"),
            "linggan.producer.task-spec.v1",
            account_ref,
            rule_revision_ref,
        ),
        DynamicCadence::Unavailable {
            reason_code: "dynamic_unavailable"
        }
    );

    let rounds = [
        first.clone(),
        comparable_round(Uuid::new_v4(), rule_revision_ref, account_ref, 86_400),
        comparable_round(Uuid::new_v4(), rule_revision_ref, account_ref, 259_200),
        comparable_round(Uuid::new_v4(), rule_revision_ref, account_ref, 518_400),
    ];
    assert_eq!(
        dynamic_cadence(
            &rounds,
            "creator_profile",
            Some("default"),
            "linggan.producer.task-spec.v1",
            account_ref,
            rule_revision_ref,
        ),
        DynamicCadence::Available {
            interval_seconds: 86_400
        }
    );

    let too_fast = [
        first.clone(),
        comparable_round(Uuid::new_v4(), rule_revision_ref, account_ref, 3_600),
    ];
    assert_eq!(
        dynamic_cadence(
            &too_fast,
            "creator_profile",
            Some("default"),
            "linggan.producer.task-spec.v1",
            account_ref,
            rule_revision_ref,
        ),
        DynamicCadence::Available {
            interval_seconds: MINIMUM_MONITOR_INTERVAL_SECONDS
        }
    );

    let too_slow = [
        first.clone(),
        comparable_round(Uuid::new_v4(), rule_revision_ref, account_ref, 30 * 86_400),
    ];
    assert_eq!(
        dynamic_cadence(
            &too_slow,
            "creator_profile",
            Some("default"),
            "linggan.producer.task-spec.v1",
            account_ref,
            rule_revision_ref,
        ),
        DynamicCadence::Available {
            interval_seconds: MAXIMUM_MONITOR_INTERVAL_SECONDS
        }
    );

    let mut wrong_lens = first;
    wrong_lens.exact_publication_time = false;
    assert_eq!(
        dynamic_cadence(
            &[
                wrong_lens,
                comparable_round(Uuid::new_v4(), rule_revision_ref, account_ref, 86_400,),
            ],
            "creator_profile",
            Some("default"),
            "linggan.producer.task-spec.v1",
            account_ref,
            rule_revision_ref,
        ),
        DynamicCadence::Unavailable {
            reason_code: "dynamic_unavailable"
        }
    );
}

#[derive(Debug)]
struct Installed {
    station_ref: Uuid,
    installation_ref: Uuid,
    install_key: String,
    credential: Option<String>,
}

async fn install(database: &Database, label: &str, version: &str) -> Installed {
    let station_ref = register_station(database, label, 200)
        .await
        .expect("station is registered");
    open_claim_window(database, station_ref, 1)
        .await
        .expect("station claim window opens");
    let install_key = Uuid::new_v4().to_string();
    let outcome = check_in_installation(
        database,
        &InstallationCheckIn {
            install_key: &install_key,
            installation_credential: None,
            plugin_version: version,
            browser_label: Some(label),
            capabilities: serde_json::json!([
                "author_profile",
                "profile_discovery",
                "discovery_search"
            ]),
        },
    )
    .await
    .expect("installation checks in");
    match outcome {
        CheckInOutcome::Claimed {
            installation_ref,
            station_ref: claimed_station,
            credential,
            ..
        } => {
            assert_eq!(claimed_station, station_ref);
            let credential = if let Some(issued) = credential {
                let raw = issued.raw_credential.expose_once().to_owned();
                activate_installation_credential(
                    database,
                    installation_ref,
                    issued.credential_ref,
                    &raw,
                )
                .await
                .expect("proof installation activates its pending credential");
                Some(raw)
            } else {
                None
            };
            Installed {
                station_ref,
                installation_ref,
                install_key,
                credential,
            }
        }
        other => panic!("open claim window must claim the installation; got {other:?}"),
    }
}

async fn ready_installation_without_account(database: &Database, label: &str) -> Installed {
    let installation = install(database, label, "0.8.34").await;
    set_station_accepting(database, installation.station_ref, true, "person")
        .await
        .expect("person enables station acceptance");
    installation
}

async fn make_account_usable(database: &Database, installation: &Installed, raw_account_id: &str) {
    let receipt = report_account_eligibility(
        database,
        installation.installation_ref,
        installation
            .credential
            .as_deref()
            .expect("compatible installation has a credential"),
        Some(raw_account_id),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
    )
    .await
    .expect("account eligibility is reported");
    bind_observation_account(
        database,
        receipt.account_ref.expect("account ref exists"),
        installation.installation_ref,
        "person",
    )
    .await
    .expect("person binds the account");
}

async fn assert_capacity_reason(database: &Database, expected: &str) {
    let capacity = read_capacity(database, "xhs", "creator", "deep_archive")
        .await
        .expect("capacity is readable");
    assert_eq!(
        capacity.reason_code(),
        expected,
        "capacity was {capacity:?}"
    );
    if expected == "available" {
        assert!(matches!(capacity, Capacity::Available { .. }));
    }
}

async fn seed_target(
    database: &Database,
    target_kind: &str,
    lifecycle_state: &str,
    identity_key: &str,
) -> Uuid {
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
         VALUES ($1,'xhs',$2,$3,$3,'manual',$4)",
    )
    .bind(target_ref)
    .bind(target_kind)
    .bind(identity_key)
    .bind(lifecycle_state)
    .execute(database.pool())
    .await
    .expect("target fixture is seeded");
    target_ref
}

async fn seed_accepted_daily_notes(database: &Database, installation: &Installed, count: i32) {
    let task_id = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let package_ref = Uuid::new_v4();
    let producer_instance_id =
        Uuid::parse_str(&installation.install_key).expect("proof install key is a producer UUID");
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
             (task_id,task_spec_hash,task_spec,source,platform,page_type) \
         VALUES ($1,repeat('a',64),'{}'::jsonb,'manual','xhs','capacity_proof')",
    )
    .bind(task_id)
    .execute(database.pool())
    .await
    .expect("daily usage task is seeded");
    sqlx::query(
        "INSERT INTO linggan_runtime_attempt (attempt_id,task_id,producer_instance_id) \
         VALUES ($1,$2,$3)",
    )
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_instance_id)
    .execute(database.pool())
    .await
    .expect("daily usage attempt is seeded");
    sqlx::query(
        "INSERT INTO linggan_runtime_capture_package \
             (package_ref,attempt_id,task_id,producer_instance_id,package_kind,platform, \
              package_hash,observed_at,captured_at,coverage,payload) \
         VALUES ($1,$2,$3,$4,'profile_discovery','xhs',repeat('b',64), \
                 '2026-09-04T00:00:00Z','2026-09-04T00:00:01Z','{}'::jsonb,'[]'::jsonb)",
    )
    .bind(package_ref)
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_instance_id)
    .execute(database.pool())
    .await
    .expect("daily usage package is seeded");
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
             (package_ref,record_ordinal,disposition,reason) \
         SELECT $1,ordinal,'accepted_for_library_discovery','capacity proof' \
         FROM generate_series(0,$2-1) AS ordinal",
    )
    .bind(package_ref)
    .bind(count)
    .execute(database.pool())
    .await
    .expect("accepted note dispositions are seeded");
}

fn fixed_rule(ranking_key: Option<&str>) -> MonitorRuleDraft {
    MonitorRuleDraft {
        mode: MonitorRuleMode::Fixed,
        automatic_enabled: true,
        run_on_weekdays: true,
        run_on_weekends: true,
        all_day: true,
        window_start_minute: None,
        window_end_minute: None,
        fixed_interval_seconds: Some(43_200),
        fallback_interval_seconds: 43_200,
        surface_key: "keyword_search".to_owned(),
        ranking_key: ranking_key.map(str::to_owned),
        task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
    }
}

fn comparable_round(
    observation_round_ref: Uuid,
    rule_revision_ref: Uuid,
    account_ref: Uuid,
    publication_epoch_seconds: i64,
) -> ComparableObservationRound {
    ComparableObservationRound {
        observation_round_ref,
        rule_revision_ref,
        publication_epoch_seconds,
        exact_publication_time: true,
        accepted_receipt: true,
        coverage_qualified: true,
        surface_key: "creator_profile".to_owned(),
        ranking_key: Some("default".to_owned()),
        task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
        account_ref,
    }
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("COLLECTION_CONTROL_PROOF_DATABASE_URL")
        .or_else(|_| std::env::var("COLLECTION_DISPATCH_PROOF_DATABASE_URL"))
        .expect("an isolated proof database URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("complete migrations through 0036 apply")
}
