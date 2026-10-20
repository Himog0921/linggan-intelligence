//! Synthetic configuration only: no provider requests or shared database access.
#[path = "ai_foundation_postgres/batches.rs"]
mod batches;
#[path = "ai_foundation_postgres/config.rs"]
mod config;
#[path = "ai_foundation_postgres/executor.rs"]
mod executor;
#[path = "ai_foundation_postgres/executor_probe.rs"]
mod executor_probe;
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "ai_foundation_postgres/ledger.rs"]
mod ledger;
#[path = "ai_foundation_postgres/rounds.rs"]
mod rounds;
#[path = "ai_foundation_postgres/skills.rs"]
mod skills;
use linggan_intelligence::ai_foundation::{
    TaskKey,
    capability::Capability,
    control_store::{Actor, ChangeControl, ControlScope, change_control},
    provider_store::{
        CapabilitySource, ChangeEgress, MaterialClass, OutputMode, PriceSource, SaveCapability,
        SavePrice, change_egress, save_capability, save_price,
    },
    store::FoundationError,
    usage::TokenPrices,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

async fn new_connection(db: &Database) -> Uuid {
    let connection = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref) VALUES($1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    connection
}
async fn version(db: &Database, connection: Uuid, revision: i32, url: &str) -> Uuid {
    let version = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,$3,'Synthetic configuration','openai-completions',$4,false,$5)")
    .bind(version).bind(connection).bind(revision).bind(url).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    version
}
async fn endpoint(db: &Database, version: Uuid) -> Uuid {
    sqlx::query_scalar(
        "SELECT endpoint_ref FROM linggan_ai_connection_endpoint WHERE version_ref=$1",
    )
    .bind(version)
    .fetch_one(db.pool())
    .await
    .unwrap()
}
async fn model(db: &Database, version: Uuid) -> Uuid {
    let model = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-model','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    model
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_database_resource_ceilings_match_the_shared_contract() {
    let db = fixture::proof_database("ai_foundation_limits").await;
    let conn = new_connection(&db).await;
    let version = version(&db, conn, 1, "https://limits.example/api").await;
    let identity = model(&db, version).await;
    let endpoint = endpoint(&db, version).await;
    let limits = &linggan_contracts::ai_limits::AI_LIMITS;
    for (context, output, timeout, allowed) in [
        (
            limits.max_context_tokens,
            limits.max_output_tokens,
            limits.max_timeout_seconds,
            true,
        ),
        (
            limits.max_context_tokens + 1,
            limits.max_output_tokens,
            limits.max_timeout_seconds,
            false,
        ),
        (
            limits.max_context_tokens,
            limits.max_output_tokens + 1,
            limits.max_timeout_seconds,
            false,
        ),
        (
            limits.max_context_tokens,
            limits.max_output_tokens,
            limits.max_timeout_seconds + 1,
            false,
        ),
    ] {
        let saved=sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode) VALUES($1,$2,$3,'manual','resource-contract-proof',$4,$5,$6,'[\"off\"]','{}','{}','prompt_only')")
            .bind(Uuid::new_v4()).bind(identity).bind(endpoint).bind(context as i32).bind(output as i32).bind(timeout as i32)
            .execute(db.pool()).await;
        assert_eq!(
            saved.is_ok(),
            allowed,
            "context={context}, output={output}, timeout={timeout}"
        );
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_endpoint_epochs_do_not_resurrect_approval_on_a_b_a() {
    let db = fixture::proof_database("ai_foundation_endpoints").await;
    let connection = new_connection(&db).await;
    let a1 = version(&db, connection, 1, "https://a.example/api").await;
    let first = endpoint(&db, a1).await;
    let a2 = version(&db, connection, 2, "https://a.example/api").await;
    assert_eq!(
        first,
        endpoint(&db, a2).await,
        "key/name rotation keeps endpoint epoch"
    );
    let b = version(&db, connection, 3, "https://b.example/api").await;
    assert_ne!(first, endpoint(&db, b).await);
    let a3 = version(&db, connection, 4, "https://a.example/api").await;
    let latest = endpoint(&db, a3).await;
    assert_ne!(first, latest, "returning to A creates another epoch");
    let grant = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason) VALUES($1,$2,'note_interpretation','note_material',true,'human','synthetic grant')").bind(grant).bind(first).execute(db.pool()).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND allowed"
        )
        .bind(latest)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert!(
        sqlx::query("UPDATE linggan_model_egress_event SET allowed=false WHERE event_ref=$1")
            .bind(grant)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(sqlx::query("INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason) VALUES($1,$2,'note_interpretation','note_material',true,'system','cannot restore')").bind(Uuid::new_v4()).bind(first).execute(db.pool()).await.is_err());
    assert!(sqlx::query("INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason) VALUES($1,$2,'model_probe','note_material',true,'human','wrong task material')").bind(Uuid::new_v4()).bind(first).execute(db.pool()).await.is_err());
    sqlx::query("INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason) VALUES($1,$2,'note_interpretation','note_material',false,'system','synthetic revoke')").bind(Uuid::new_v4()).bind(first).execute(db.pool()).await.unwrap();
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT allowed FROM linggan_ai_current_egress WHERE endpoint_ref=$1"
        )
        .bind(first)
        .fetch_one(db.pool())
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_model_identity_is_stable_but_route_capability_and_price_are_not_assumed() {
    let db = fixture::proof_database("ai_foundation_capabilities").await;
    let connection = new_connection(&db).await;
    let one = version(&db, connection, 1, "https://a.example/api").await;
    let original = model(&db, one).await;
    let two = version(&db, connection, 2, "https://a.example/api").await;
    model(&db, two).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ai_model_identity WHERE connection_ref=$1"
        )
        .bind(connection)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    let route = endpoint(&db, two).await;
    let capability = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode) VALUES($1,$2,$3,'manual','synthetic-fixture',1000000,384000,600,$4,'{}','{}','prompt_only')").bind(capability).bind(original).bind(route).bind(json!(["off","high","max"])).execute(db.pool()).await.unwrap();
    assert!(
        sqlx::query("UPDATE linggan_model_capability SET context_tokens=1 WHERE capability_ref=$1")
            .bind(capability)
            .execute(db.pool())
            .await
            .is_err()
    );
    let other = new_connection(&db).await;
    let other_version = version(&db, other, 1, "https://other.example/api").await;
    let other_route = endpoint(&db, other_version).await;
    let bad=sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode) VALUES($1,$2,$3,'manual','synthetic-fixture',1000000,384000,600,'[\"off\"]','{}','{}','prompt_only')").bind(Uuid::new_v4()).bind(original).bind(other_route).execute(db.pool()).await;
    assert!(
        bad.is_err(),
        "a capability cannot cross a connection boundary"
    );
    let invalid=sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode) VALUES($1,$2,$3,'manual','synthetic-fixture',1000000,384000,600,'[\"guessed\"]','{}','{}','prompt_only')").bind(Uuid::new_v4()).bind(original).bind(route).execute(db.pool()).await;
    assert!(invalid.is_err());
    sqlx::query("INSERT INTO linggan_model_price(price_ref,identity_ref,endpoint_ref,source_kind,source_version) VALUES($1,$2,$3,'manual','unknown by fixture')").bind(Uuid::new_v4()).bind(original).bind(route).execute(db.pool()).await.unwrap();
    let unknown: Option<i64> = sqlx::query_scalar(
        "SELECT input_micro_usd_per_million FROM linggan_model_price WHERE identity_ref=$1",
    )
    .bind(original)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(unknown, None);
    let fresh = version(&db, connection, 3, "https://b.example/api").await;
    let fresh_route = endpoint(&db, fresh).await;
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_model_capability WHERE identity_ref=$1 AND endpoint_ref=$2").bind(original).bind(fresh_route).fetch_one(db.pool()).await.unwrap(),0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT linggan_ai_budget_day(timestamptz '2026-10-19T15:59:59Z')::text"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "2026-10-19"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT linggan_ai_budget_day(timestamptz '2026-10-19T16:00:00Z')::text"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "2026-10-20"
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_control_commands_are_cas_idempotent_and_system_never_resumes() {
    let db = fixture::proof_database("ai_foundation_controls").await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_control WHERE enabled")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    let command = ChangeControl {
        command_ref: Uuid::new_v4(),
        scope: ControlScope::Global,
        expected_revision: 0,
        enabled: true,
        reason: "human enabled synthetic workspace".into(),
    };
    assert!(matches!(
        change_control(&db, &command, Actor::System).await,
        Err(FoundationError::SystemCannotResume)
    ));
    let (one, two) = tokio::join!(
        change_control(&db, &command, Actor::Human),
        change_control(&db, &command, Actor::Human)
    );
    assert_eq!(
        one.unwrap(),
        two.unwrap(),
        "duplicate commands replay the same receipt"
    );
    let mut changed = command.clone();
    changed.reason = "different command payload".into();
    assert!(matches!(
        change_control(&db, &changed, Actor::Human).await,
        Err(FoundationError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_control_event")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    let pause = ChangeControl {
        command_ref: Uuid::new_v4(),
        expected_revision: 1,
        enabled: false,
        reason: "synthetic safety brake".into(),
        ..command.clone()
    };
    let mut stale = pause.clone();
    stale.command_ref = Uuid::new_v4();
    let (a, b) = tokio::join!(
        change_control(&db, &pause, Actor::System),
        change_control(&db, &stale, Actor::Human)
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "only one writer can advance the same expected revision"
    );
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(FoundationError::Conflict)
    ));
    let pending = ChangeControl {
        command_ref: Uuid::new_v4(),
        scope: ControlScope::Task(TaskKey::NoteInterpretation),
        expected_revision: 0,
        enabled: true,
        reason: "must not invent handler availability".into(),
    };
    assert!(matches!(
        change_control(&db, &pending, Actor::Human).await,
        Err(FoundationError::NotConnected)
    ));
    for task in [
        TaskKey::CommentStudy,
        TaskKey::CommentEmbedding,
        TaskKey::LegacyCreatorDiscovery,
        TaskKey::LegacyTopicMap,
    ] {
        let request = ChangeControl {
            command_ref: Uuid::new_v4(),
            scope: ControlScope::Task(task),
            ..pending.clone()
        };
        assert!(matches!(
            change_control(&db, &request, Actor::Human).await,
            Err(FoundationError::NotConnected)
        ));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_control_event")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_command")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_provider_commands_are_versioned_local_facts_and_cannot_reactivate_a_revoke()
{
    let db = fixture::proof_database("ai_foundation_provider_commands").await;
    let conn = new_connection(&db).await;
    let first = version(&db, conn, 1, "https://a.example/api").await;
    let identity = model(&db, first).await;
    let route = endpoint(&db, first).await;
    let capability = SaveCapability {
        command_ref: Uuid::new_v4(),
        identity_ref: identity,
        endpoint_ref: route,
        expected_capability_ref: None,
        source: CapabilitySource::Manual,
        source_version: "synthetic".into(),
        probe_invocation_ref: None,
        limits: Capability {
            context_tokens: 1_000_000,
            max_output_tokens: 384_000,
            max_timeout_seconds: 600,
            thinking_levels: ["off", "high", "max"].map(str::to_owned).into(),
        },
        compat: json!({"supportsStore":false,"thinkingFormat":"deepseek"}),
        thinking_level_map: json!({"low":null,"high":"high","max":"max"}),
        output_mode: OutputMode::PromptOnly,
        strict_schema: false,
    };
    let initial = save_capability(&db, &capability).await.unwrap();
    assert_eq!(save_capability(&db, &capability).await.unwrap(), initial);
    let mut altered = capability.clone();
    altered.limits.max_timeout_seconds = 599;
    assert!(matches!(
        save_capability(&db, &altered).await,
        Err(FoundationError::Conflict)
    ));
    let mut next = capability.clone();
    next.command_ref = Uuid::new_v4();
    next.expected_capability_ref = Some(initial.revision_ref);
    let mut competing = next.clone();
    competing.command_ref = Uuid::new_v4();
    let (a, b) = tokio::join!(
        save_capability(&db, &next),
        save_capability(&db, &competing)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(FoundationError::Conflict)
    ));
    let grant = ChangeEgress {
        command_ref: Uuid::new_v4(),
        endpoint_ref: route,
        task_key: TaskKey::NoteInterpretation,
        material_class: MaterialClass::NoteMaterial,
        expected_event_ref: None,
        allowed: true,
        reason: "synthetic material grant".into(),
    };
    let granted = change_egress(&db, &grant, Actor::Human).await.unwrap();
    let revoke = ChangeEgress {
        command_ref: Uuid::new_v4(),
        allowed: false,
        expected_event_ref: Some(granted.event_ref),
        reason: "synthetic revoke".into(),
        ..grant.clone()
    };
    let revoked = change_egress(&db, &revoke, Actor::System).await.unwrap();
    assert!(!revoked.allowed);
    assert_eq!(
        change_egress(&db, &grant, Actor::Human).await.unwrap(),
        granted,
        "a replay acknowledges its historical command only"
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT allowed FROM linggan_ai_current_egress WHERE endpoint_ref=$1"
        )
        .bind(route)
        .fetch_one(db.pool())
        .await
        .unwrap()
    );
    let restore = ChangeEgress {
        command_ref: Uuid::new_v4(),
        expected_event_ref: Some(revoked.event_ref),
        ..grant.clone()
    };
    assert!(matches!(
        change_egress(&db, &restore, Actor::System).await,
        Err(FoundationError::SystemCannotResume)
    ));
    let price = SavePrice {
        command_ref: Uuid::new_v4(),
        identity_ref: identity,
        endpoint_ref: route,
        expected_price_ref: None,
        source: PriceSource::Manual,
        source_version: "synthetic unknown price".into(),
        prices: TokenPrices {
            input: None,
            output: Some(0),
            cached_read: None,
            cached_write: None,
        },
    };
    let receipt = save_price(&db, &price).await.unwrap();
    assert_eq!(save_price(&db, &price).await.unwrap(), receipt);
    let unknown: Option<i64> = sqlx::query_scalar(
        "SELECT input_micro_usd_per_million FROM linggan_model_price WHERE price_ref=$1",
    )
    .bind(receipt.revision_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let zero: Option<i64> = sqlx::query_scalar(
        "SELECT output_micro_usd_per_million FROM linggan_model_price WHERE price_ref=$1",
    )
    .bind(receipt.revision_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(unknown, None);
    assert_eq!(zero, Some(0));
    version(&db, conn, 2, "https://b.example/api").await;
    assert!(
        matches!(
            change_egress(&db, &restore, Actor::Human).await,
            Err(FoundationError::Conflict)
        ),
        "stale endpoint cannot receive a grant"
    );
    let old_route_cap = SaveCapability {
        command_ref: Uuid::new_v4(),
        ..next
    };
    assert!(matches!(
        save_capability(&db, &old_route_cap).await,
        Err(FoundationError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[path = "ai_foundation_postgres/retirement.rs"]
mod retirement;
