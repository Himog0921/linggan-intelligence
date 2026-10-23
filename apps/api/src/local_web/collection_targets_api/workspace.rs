//! WEB-REACT-001 G: the Collection target workspace's narrow, read-only JSON projection.
//! Domain owners still decide qualification and actions. No HTML, platform access or SQL is
//! introduced here; unknown reads stay null and are never converted into empty facts.
use super::{
    LocalWebState,
    collection_target_presentation::{
        self as target_drawer, KeywordArchiveRead, TargetArchiveRead, TargetPrimaryAction,
    },
    local_read_json_error,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_evidence::*;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/collection/workspace/targets", get(list))
        .route("/api/collection/workspace/target/{target}", get(drawer))
        .route(
            "/api/collection/workspace/target/{target}/rules",
            get(rules),
        )
        .route(
            "/api/collection/workspace/target/{target}/assignment",
            get(assignment),
        )
        .route(
            "/api/collection/workspace/target/{target}/deletion",
            get(deletion),
        )
}

#[derive(Default, serde::Deserialize)]
struct Params {
    dtab: Option<String>,
    wview: Option<String>,
    domain: Option<String>,
    filter: Option<String>,
    life_window: Option<String>,
    life_metric: Option<String>,
    rule_slot: Option<String>,
    rule_receipt: Option<Uuid>,
}

fn unavailable() -> Response {
    local_read_json_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "collection_targets_unavailable",
    )
}

fn avatar(value: Option<&ObservationTargetAvatar>) -> Value {
    match value {
        Some(ObservationTargetAvatar::Local { local_asset_path }) => {
            json!({"state":"local","path":local_asset_path})
        }
        Some(ObservationTargetAvatar::Pending) => json!({"state":"pending","path":null}),
        Some(ObservationTargetAvatar::Unavailable) => json!({"state":"unavailable","path":null}),
        Some(ObservationTargetAvatar::NotObserved) | None => {
            json!({"state":"not_observed","path":null})
        }
    }
}

fn action(value: TargetPrimaryAction) -> Value {
    let (kind, label, tab, fragment) = match value {
        TargetPrimaryAction::AssignDomain => ("assign_domain", "分配领域", "overview", ""),
        TargetPrimaryAction::EstablishArchive => ("archive", "建立档案", "overview", ""),
        TargetPrimaryAction::RebuildDirectory => ("archive", "处理异常", "overview", ""),
        TargetPrimaryAction::ContinueArchive => ("gaps", "补采缺口", "overview", ""),
        TargetPrimaryAction::ViewArchiveProgress => {
            ("view", "查看任务", "overview", "archive-task")
        }
        TargetPrimaryAction::ViewArchiveProblems => {
            ("view", "处理异常", "overview", "archive-problems")
        }
        TargetPrimaryAction::ViewArchiveUnavailable => {
            ("view", "查看档案", "overview", "target-archive")
        }
        TargetPrimaryAction::OpenPatrol(label) => ("rule", label, "patrol", ""),
        TargetPrimaryAction::ViewCreator => ("view", "查看档案", "works", "target-works"),
        TargetPrimaryAction::ViewKeyword => ("view", "查看结果", "works", "target-works"),
    };
    json!({"kind":kind,"label":label,"tab":tab,"fragment":fragment})
}

fn archive_state(
    target: &ObservationTarget,
    archive: TargetArchiveRead<'_>,
) -> (&'static str, &'static str) {
    if target.domain_name.is_none() {
        return ("warn", "待分配领域");
    }
    match archive {
        TargetArchiveRead::Unavailable => ("neutral", "档案暂不可读"),
        TargetArchiveRead::Known(Some(v))
            if v.directory_baseline == ArchiveDirectoryBaseline::NotStarted =>
        {
            ("neutral", "尚未建立")
        }
        TargetArchiveRead::Known(Some(v)) if v.has_actionable_problems() => ("warn", "档案有问题"),
        TargetArchiveRead::Known(Some(v)) if v.work_in_progress => ("warn", "建档中"),
        TargetArchiveRead::Known(Some(v))
            if v.directory_baseline == ArchiveDirectoryBaseline::HistoricalDirectory =>
        {
            ("ok", "已建立")
        }
        TargetArchiveRead::Known(Some(v)) if v.requires_directory_rebuild() => ("warn", "异常"),
        TargetArchiveRead::Known(None) => ("neutral", "尚未建立"),
        TargetArchiveRead::Known(Some(v)) if v.is_untouched() => ("neutral", "尚未建立"),
        TargetArchiveRead::Known(Some(v)) if v.works_listed > 0 && v.pending_details > 0 => {
            ("warn", "详情有缺口")
        }
        TargetArchiveRead::Known(Some(v))
            if v.works_listed > 0
                && matches!(
                    target.lifecycle_state.as_str(),
                    "archived" | "monitoring" | "paused"
                ) =>
        {
            ("ok", "档案已建立")
        }
        TargetArchiveRead::Known(Some(v)) if v.started || v.attempted => ("warn", "档案有问题"),
        TargetArchiveRead::Known(Some(v)) if v.works_listed > 0 => ("warn", "详情有缺口"),
        TargetArchiveRead::Known(Some(_)) => ("neutral", "等待作品目录"),
    }
}

fn row(
    target: &ObservationTarget,
    image: Option<&ObservationTargetAvatar>,
    archive: TargetArchiveRead<'_>,
    observation: Option<&TargetObservationSummary>,
    keyword: KeywordArchiveRead,
    counts: Option<&KeywordCatalogCounts>,
    counts_available: bool,
) -> Value {
    let (archive_tone, archive_label) = if target.target_kind == "creator" {
        archive_state(target, archive)
    } else {
        match keyword {
            KeywordArchiveRead::Unavailable => ("neutral", "读不到"),
            KeywordArchiveRead::NotArchived => ("neutral", "尚未建立"),
            KeywordArchiveRead::DetailPending => ("info", "建档中"),
            KeywordArchiveRead::Complete => ("ok", "档案已建立"),
        }
    };
    let (patrol_tone, patrol_label) = match observation.map(|v| v.patrol_state) {
        Some(PatrolReadState::Disabled) => ("neutral", "已暂停"),
        Some(PatrolReadState::Waiting) => ("neutral", "等待巡查"),
        Some(PatrolReadState::Running) => ("info", "巡查中"),
        Some(PatrolReadState::Normal) => ("ok", "正常"),
        Some(PatrolReadState::Partial) => ("warn", "最近巡查未完成"),
        Some(PatrolReadState::Blocked) => ("warn", "受阻"),
        _ => target_drawer::lifecycle_patrol_copy(target),
    };
    let (works, details) = if target.target_kind == "creator" {
        match archive {
            TargetArchiveRead::Unavailable => ("当前读不到".to_owned(), "当前读不到".to_owned()),
            TargetArchiveRead::Known(Some(v)) if v.works_listed > 0 => {
                if v.has_displayable_directory() {
                    (
                        format!("{} 篇", v.works_listed),
                        format!("{}/{}", v.details_captured, v.works_listed),
                    )
                } else {
                    (
                        format!("{} 篇已见", v.works_listed),
                        format!("{}/{} 已见", v.details_captured, v.works_listed),
                    )
                }
            }
            _ => ("—".to_owned(), "—".to_owned()),
        }
    } else {
        match counts {
            Some(v) => (
                v.works.to_string(),
                if v.works == 0 {
                    "—".to_owned()
                } else {
                    format!("{}/{}", v.details, v.works)
                },
            ),
            None if counts_available => ("0".to_owned(), "—".to_owned()),
            None => ("读不到".to_owned(), "读不到".to_owned()),
        }
    };
    json!({"targetRef":target.target_ref,"kind":target.target_kind,"platform":target.platform,"identity":target.identity_key,
        "handle":target.identity_facts.as_ref().and_then(|v|v.get("redId")).and_then(Value::as_str).filter(|v|!v.trim().is_empty()).unwrap_or(&target.identity_key),
        "name":target.display_name.as_deref().filter(|v|!v.trim().is_empty()).unwrap_or(&target.identity_key),"group":target.group_name.as_deref().unwrap_or("未分组"),"domainName":target.domain_name,
        "lifecycle":target.lifecycle_state,"monitoring":target.monitoring_enabled,"avatar":avatar(image),"archiveLabel":archive_label,"archiveTone":archive_tone,"patrolLabel":patrol_label,"patrolTone":patrol_tone,
        "works":works,"details":details,"recentNew":observation.and_then(|v|v.latest_new),"recentHits":observation.and_then(|v|v.latest_hits),
        "last":target.last_patrol_succeeded_at,"next":target.next_patrol_at,"action":action(target_drawer::target_primary_action(target,target.target_kind=="creator",archive,keyword))})
}

async fn list(State(state): State<LocalWebState>, Query(params): Query<Params>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(domains) = observation_domain::read_observation_domains(db).await else {
        return unavailable();
    };
    let current = observation_domain::resolve_collection_domain(&domains, params.domain.as_deref());
    let domain_ref = current.map(|v| v.domain_ref);
    let (targets, counts, archives) = tokio::join!(
        list_targets(db, params.filter.as_deref(), domain_ref, 200),
        count_targets(db, domain_ref),
        read_archive_completeness(db, linggan_contracts::OPEN_PLATFORM)
    );
    let Ok(targets) = targets else {
        return unavailable();
    };
    let refs: Vec<_> = targets
        .iter()
        .filter(|v| v.target_kind == "keyword")
        .map(|v| v.target_ref)
        .collect();
    let (images, observations, baselines, pending, keyword_counts) = tokio::join!(
        read_target_avatars(db, &targets),
        read_target_observation_summaries(db, &targets),
        keyword_baselines_qualified(db, &refs),
        keyword_targets_pending_detail(db, &refs),
        read_keyword_catalog_counts(db, &refs, domain_ref)
    );
    let archives = archives.ok();
    let images = images.ok();
    let observations = observations.ok();
    let baselines = baselines.ok();
    let pending = pending.ok();
    let keyword_counts = keyword_counts.ok();
    let rows: Vec<_> = targets
        .iter()
        .map(|v| {
            row(
                v,
                images.as_ref().and_then(|m| m.get(&v.target_ref)),
                TargetArchiveRead::from_map(archives.as_ref(), &v.identity_key),
                observations.as_ref().and_then(|m| m.get(&v.target_ref)),
                target_drawer::keyword_archive_read(
                    baselines.as_ref(),
                    pending.as_ref(),
                    v.target_ref,
                ),
                keyword_counts.as_ref().and_then(|m| m.get(&v.target_ref)),
                keyword_counts.is_some(),
            )
        })
        .collect();
    let count_json=counts.ok().map(|v|json!({"total":v.total,"creator":v.creator,"keyword":v.keyword,"archiving":v.archiving,"monitoring":v.monitoring}));
    Json(json!({"rows":rows,"counts":count_json,"currentDomain":domain_ref,"limit":200,"domains":domains.iter().map(|v|json!({"domainRef":v.domain_ref,"name":v.name,"status":v.status})).collect::<Vec<_>>()})).into_response()
}

fn catalog_work(v: &CatalogWork) -> Value {
    let (detail_label, detail_state) =
        match (v.detail_state, v.execution_state.as_ref().map(|v| v.kind)) {
            (CatalogDetailState::Complete, _) => ("已完成", "complete"),
            (_, Some(MaterialExecutionKind::RetryPending)) => ("延迟重试", "retry_pending"),
            (_, Some(MaterialExecutionKind::InputBlocked)) => ("输入不可执行", "input_blocked"),
            (_, Some(MaterialExecutionKind::BudgetExhausted)) => {
                ("自动重试已停止", "budget_exhausted")
            }
            _ => ("待采集", "pending"),
        };
    json!({"publicRef":v.public_ref,"externalId":v.content_external_id,"title":v.title,"creatorName":v.creator_display_name,"creatorId":v.creator_external_id,"position":v.match_position,"published":v.published_at,"source":match v.source {CatalogSource::InitialArchive=>"archive",CatalogSource::PatrolDiscovery=>"patrol"},"kind":v.recorded_kind,"complete":v.detail_state==CatalogDetailState::Complete,"detailLabel":detail_label,"detailState":detail_state,"media":v.media_state,"comments":v.comment_count,"last":v.last_captured_at})
}

async fn drawer(
    State(state): State<LocalWebState>,
    Path(target): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let found = match read_target(db, target).await {
        Ok(Some(v)) => v,
        Ok(None) => return local_read_json_error(StatusCode::NOT_FOUND, "target_not_found"),
        Err(_) => return unavailable(),
    };
    let domains = observation_domain::read_observation_domains(db).await.ok();
    let domain_ref = domains
        .as_ref()
        .and_then(|d| observation_domain::resolve_collection_domain(d, params.domain.as_deref()))
        .map(|d| d.domain_ref);
    let refs = [target];
    let (archives, images, inspector, observations, baselines, pending, counts, rules, blocked) = tokio::join!(
        read_archive_completeness(db, linggan_contracts::OPEN_PLATFORM),
        read_target_avatars(db, std::slice::from_ref(&found)),
        read_target_inspector(db, target),
        read_target_observation_summaries(db, std::slice::from_ref(&found)),
        keyword_baselines_qualified(db, &refs),
        keyword_targets_pending_detail(db, &refs),
        read_keyword_catalog_counts(db, &refs, domain_ref),
        read_target_monitor_rules(db, target),
        read_blocked_materials(db, target)
    );
    let archives = archives.ok();
    let baselines = baselines.ok();
    let pending = pending.ok();
    let keyword = target_drawer::keyword_archive_read(baselines.as_ref(), pending.as_ref(), target);
    let catalog = if found.target_kind == "creator" {
        read_creator_directory(db, target, domain_ref)
            .await
            .ok()
            .flatten()
            .map(|v| v.works)
    } else {
        read_keyword_hits(db, target, domain_ref)
            .await
            .ok()
            .flatten()
            .map(|v| v.works)
    };
    let wants_performance = params
        .dtab
        .as_deref()
        .is_none_or(|tab| matches!(tab, "works" | "baseline" | "archive"))
        && params.wview.as_deref().map_or(
            params.life_window.is_some() || params.life_metric.is_some(),
            |view| view == "performance",
        );
    let life = if found.target_kind == "creator" && wants_performance {
        match CreatorLifecycleQuery::parse_optional(
            params.life_window.as_deref(),
            params.life_metric.as_deref(),
        ) {
            Ok(query) => match read_creator_lifecycle(db, target, &query).await {
                Ok(Some(p)) => json!({"state":"ready","projection":life_projection(&p)}),
                _ => json!({"state":"unavailable"}),
            },
            Err(_) => json!({"state":"invalid"}),
        }
    } else {
        json!({"state":"not_read"})
    };
    let decision = match inspector.as_ref().ok().and_then(Option::as_ref) {
        Some(i)
            if found.target_kind == "creator"
                && found.domain_name.is_some()
                && found.lifecycle_state != "dismissed" =>
        {
            let (title, note) = target_drawer::inspector_action_copy(i.required_action);
            let control = match i.required_action {
                TargetInspectorAction::StartArchive => {
                    Some(action(TargetPrimaryAction::EstablishArchive))
                }
                TargetInspectorAction::RebuildDirectory => {
                    Some(action(TargetPrimaryAction::RebuildDirectory))
                }
                TargetInspectorAction::ContinueArchive => {
                    Some(action(TargetPrimaryAction::ContinueArchive))
                }
                TargetInspectorAction::EnablePatrol => {
                    Some(action(TargetPrimaryAction::OpenPatrol("开启巡查")))
                }
                TargetInspectorAction::HandleArchiveProblems => Some(
                    json!({"kind":"view_pending","label":"查看待取得作品","tab":"works","fragment":"target-works"}),
                ),
                _ => None,
            };
            json!({"title":title,"note":note,"action":control})
        }
        _ => {
            let a = target_drawer::target_primary_action(
                &found,
                found.target_kind == "creator",
                TargetArchiveRead::from_map(archives.as_ref(), &found.identity_key),
                keyword,
            );
            let (title, note) = target_drawer::required_action_copy(a);
            json!({"title":title,"note":note,"action":action(a)})
        }
    };
    let primary = target_drawer::target_primary_action(
        &found,
        found.target_kind == "creator",
        TargetArchiveRead::from_map(archives.as_ref(), &found.identity_key),
        keyword,
    );
    let content_gap_action = if found.target_kind == "creator"
        && !matches!(
            primary,
            TargetPrimaryAction::ContinueArchive | TargetPrimaryAction::AssignDomain
        )
        && found.lifecycle_state != "dismissed"
        && archives
            .as_ref()
            .and_then(|m| m.get(&found.identity_key))
            .is_some_and(|v| v.works_listed > 0)
    {
        Some(json!({"kind":"gaps","label":"核查并补采内容缺口","tab":"overview","fragment":""}))
    } else {
        None
    };
    let row = row(
        &found,
        images.as_ref().ok().and_then(|m| m.get(&target)),
        TargetArchiveRead::from_map(archives.as_ref(), &found.identity_key),
        observations.as_ref().ok().and_then(|m| m.get(&target)),
        keyword,
        counts.as_ref().ok().and_then(|m| m.get(&target)),
        counts.is_ok(),
    );
    Json(json!({"target":row,"contentGapAction":content_gap_action,"decision":decision,"inspector":inspector.ok().flatten(),"catalog":catalog.as_ref().map(|v|v.iter().map(catalog_work).collect::<Vec<_>>()),"lifecycle":life,"rules":rules.ok().flatten().map(|v|v.iter().map(|r|json!({"ruleRef":r.rule_ref,"slot":r.slot_key,"enabled":r.automatic_enabled,"interval":r.interval_seconds,"scroll":r.scroll_rounds,"top":r.top_by_likes,"days":r.published_within_days,"next":r.next_run_at,"last":r.last_succeeded_at,"revision":r.revision})).collect::<Vec<_>>()),"blocked":blocked.ok().map(|v|v.iter().map(|b|json!({"publicRef":b.content_public_ref,"externalId":b.content_external_id,"title":b.title})).collect::<Vec<_>>())})).into_response()
}

fn life_projection(p: &CreatorLifecycleProjection) -> Value {
    json!({"status":p.status,"asOf":p.as_of,"window":p.window,"metric":p.metric,"summary":p.summary,"exclusions":p.exclusions,"receipt":p.receipt,"points":p.points.iter().map(|v|json!({"publicRef":v.work_public_ref,"title":v.title,"titleState":v.title_state,"published":v.published_at,"date":v.published_local_date,"epochMs":v.published_at_epoch_ms,"value":v.metric_value,"discussionRate":v.discussion_rate,"association":v.association_state,"new":v.new_in_latest_patrol})).collect::<Vec<_>>()})
}

async fn rules(
    State(state): State<LocalWebState>,
    Path(target): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    use super::collection::collection_control_rule_view::*;
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let selection = match params.rule_slot.as_deref() {
        Some("new") => MonitorRuleSelection::NewRule,
        Some(slot) => MonitorRuleSelection::Slot(slot),
        None => MonitorRuleSelection::CurrentRule,
    };
    let panel = match read_monitor_rule_panel(db, target, selection, params.rule_receipt).await {
        Ok(MonitorRulePanelRead::Found(p)) => p,
        Ok(MonitorRulePanelRead::TargetNotFound) => {
            return local_read_json_error(StatusCode::NOT_FOUND, "target_not_found");
        }
        Ok(MonitorRulePanelRead::SchemaUnavailable) => {
            return local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "rule_schema_unavailable",
            );
        }
        _ => return unavailable(),
    };
    let f = MonitorRuleFormState::from_panel(&panel);
    Json(json!({"targetRef":panel.target_ref,"name":panel.target_name,"kind":panel.target_kind,"lifecycle":panel.lifecycle_state,"slot":panel.editing_slot,"rankings":panel.available_rankings,"form":{"expected_revision":f.expected_revision,"idempotency_key":f.idempotency_key,"automatic_enabled":f.automatic_enabled,"creator_follow_details":f.creator_follow_details,"fixed_interval_seconds":f.fixed_interval_seconds,"surface_key":f.surface_key,"ranking_key":f.ranking_key,"scroll_rounds":f.scroll_rounds,"top_by_likes":f.top_by_likes,"published_within_days":f.published_within_days,"task_contract_version":f.task_contract_version},"receipt":panel.receipt.map(|r|json!({"receiptRef":r.receipt_ref,"outcome":r.outcome,"reason":r.reason_code,"expectedRevision":r.expected_revision,"recordedAt":r.recorded_at,"appliedRuleRevisionRef":r.applied_rule_revision_ref}))})).into_response()
}

async fn assignment(State(state): State<LocalWebState>, Path(target): Path<Uuid>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    match read_target(db, target).await {
        Ok(Some(value)) => Json(json!({"targetRef":value.target_ref,"name":value.display_name.as_deref().filter(|v|!v.trim().is_empty()).unwrap_or(&value.identity_key),"assignable":value.domain_name.is_none()})).into_response(),
        Ok(None) => local_read_json_error(StatusCode::NOT_FOUND,"target_not_found"),
        Err(_) => unavailable(),
    }
}

async fn deletion(State(state): State<LocalWebState>, Path(target): Path<Uuid>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    match read_target_deletion_preview(db,target).await {Ok(Some(p))=>Json(json!({"blocked":p.blocking_material_usages>0||p.blocking_work_order_usages>0||p.blocking_risk_signals>0,"targetRef":target,"name":p.confirmation_name,"kind":p.target_kind,"workOrders":p.work_orders,"leases":p.leases,"tasks":p.lease_tasks,"rules":p.rule_revisions,"requests":p.requests,"retainedWorks":p.retained_works,"retainedDetails":p.retained_details,"blockingMaterialUsages":p.blocking_material_usages,"blockingWorkOrderUsages":p.blocking_work_order_usages,"blockingRiskSignals":p.blocking_risk_signals,"retirements":p.material_retirements})).into_response(),Ok(None)=>local_read_json_error(StatusCode::NOT_FOUND,"target_not_found"),Err(_)=>unavailable()}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn keyword() -> ObservationTarget {
        ObservationTarget {
            target_ref: Uuid::nil(),
            platform: "xhs".into(),
            target_kind: "keyword".into(),
            identity_key: "合成关键词::most_liked".into(),
            display_name: Some("合成关键词".into()),
            identity_facts: None,
            source: "manual".into(),
            lifecycle_state: "pending_decision".into(),
            first_stored_at: "2026-10-16 09:00".into(),
            monitoring_enabled: false,
            group_name: None,
            last_patrol_dispatched_at: None,
            last_patrol_succeeded_at: None,
            next_patrol_at: None,
            domain_name: Some("合成领域".into()),
        }
    }
    #[test]
    fn successful_missing_keyword_count_is_zero_but_failed_read_is_unknown() {
        let target = keyword();
        let known = row(
            &target,
            None,
            TargetArchiveRead::Known(None),
            None,
            KeywordArchiveRead::NotArchived,
            None,
            true,
        );
        let unknown = row(
            &target,
            None,
            TargetArchiveRead::Unavailable,
            None,
            KeywordArchiveRead::Unavailable,
            None,
            false,
        );
        assert_eq!(known["works"], "0");
        assert_eq!(known["details"], "—");
        assert_eq!(known["action"]["kind"], "archive");
        assert_eq!(unknown["works"], "读不到");
        assert_eq!(unknown["action"]["kind"], "view");
        assert_eq!(unknown["recentNew"], Value::Null);
    }
    #[test]
    fn pending_keyword_details_preserve_shared_owner_action_even_when_monitoring() {
        let mut target = keyword();
        target.monitoring_enabled = true;
        target.lifecycle_state = "monitoring".into();
        let value = row(
            &target,
            None,
            TargetArchiveRead::Known(None),
            Some(&TargetObservationSummary {
                latest_hits: Some(0),
                latest_new: Some(0),
                patrol_state: PatrolReadState::Normal,
            }),
            KeywordArchiveRead::DetailPending,
            Some(&KeywordCatalogCounts {
                works: 4,
                details: 2,
            }),
            true,
        );
        assert_eq!(value["action"]["kind"], "gaps");
        assert_eq!(value["recentNew"], 0);
        assert_eq!(value["archiveLabel"], "建档中");
        assert_eq!(value["details"], "2/4");
    }
}
