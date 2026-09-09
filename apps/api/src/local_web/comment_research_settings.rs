//! Business-rule and automatic comparison settings remain under comment research.
use super::{LocalWebState, response, unavailable};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::{get, post},
};
use linggan_intelligence::{
    comment_replay as replay, comment_research_rules as rules, model_settings::ModelError,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/comment-intelligence/research-settings",
            get(read),
        )
        .route("/api/local/comment-intelligence/rules", post(candidate))
        .route(
            "/api/local/comment-intelligence/upgrade-policy",
            post(policy),
        )
        .route(
            "/api/local/comment-intelligence/replays",
            get(replays).post(start_replay),
        )
        .route(
            "/api/local/comment-intelligence/replays/{run}",
            get(replay_detail),
        )
}
async fn read(State(s): State<LocalWebState>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(async {
        let active=rules::read_active_rule(db).await?;
        let refs:Vec<Uuid>=sqlx::query_scalar("SELECT rule_revision_ref FROM linggan_comment_research_rule_revision ORDER BY created_at DESC,rule_revision_ref DESC LIMIT 50").fetch_all(db.pool()).await?;
        let mut candidates=Vec::new();
        for id in refs {candidates.push(rules::read_rule(db,id).await?);}
        let policy:Value=sqlx::query_scalar("SELECT jsonb_build_object('revision',revision,'enabled',enabled,'selectedCandidateRuleRevisionRef',selected_candidate_rule_revision_ref) FROM linggan_comment_auto_upgrade_policy WHERE singleton").fetch_one(db.pool()).await?;
        Ok(json!({"activeRule":active,"candidates":candidates,"upgradePolicy":policy,
          "replays":replay::read_replays(db).await?,"continuity":linggan_intelligence::comment_replay_continuity::read_replay_continuity(db).await?,"programSafeguards":"来源身份、证据引用和输出结构由程序固定校验；业务规则不能关闭这些保护。",
          "replaySampling":"系统按长短、回复关系、上下文依赖和已有结果分层，并兼顾作品分布，选择最多120条当前可研究原声，固定样本后对比，不需要逐条标注。"}))
    }.await)
}
async fn candidate(
    State(s): State<LocalWebState>,
    Json(r): Json<rules::CreateRuleRevision>,
) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(
        rules::create_candidate(db, &r)
            .await
            .and_then(|v| serde_json::to_value(v).map_err(|_| ModelError::Invalid)),
    )
}
async fn policy(
    State(s): State<LocalWebState>,
    Json(r): Json<replay::ConfigureAutoUpgradePolicy>,
) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(replay::configure_auto_upgrade_policy(db, &r).await)
}
async fn replays(State(s): State<LocalWebState>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(replay::read_replays(db).await)
}
async fn replay_detail(State(s): State<LocalWebState>, Path(run): Path<Uuid>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(replay::read_replay(db, run).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartReplay {
    run_ref: Uuid,
    candidate_rule_revision_ref: Uuid,
    config_ref: Uuid,
    authorized_token_budget: i64,
}
async fn start_replay(State(s): State<LocalWebState>, Json(r): Json<StartReplay>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(async {
        match replay::read_replay(db,r.run_ref).await {
            Ok(existing) => {
                if existing["run"]["candidateRuleRevisionRef"]!=json!(r.candidate_rule_revision_ref)
                    || existing["run"]["configRef"]!=json!(r.config_ref)
                    || existing["run"]["authorizedTokenBudget"]!=json!(r.authorized_token_budget) {
                    return Err(ModelError::Conflict);
                }
                return Ok(existing);
            },
            Err(ModelError::NotFound) => {},
            Err(error) => return Err(error),
        }
        let active=rules::read_active_rule(db).await?;
        let context=linggan_intelligence::comment_runtime::settings(db).await?;
        let refs=linggan_intelligence::comment_research_sampling::replay_sources(db,17).await?;
        if refs.is_empty(){return Ok(json!({"state":"insufficient_evidence","reason":"尚无清洗完成且可研究的评论，系统取得材料后才能比较规则。"}));}
        replay::create_candidate_replay(db,&replay::CreateCandidateReplay {
            run_ref:r.run_ref,sample_set_ref:Uuid::new_v4(),baseline_rule_revision_ref:active.rule_revision_ref,
            candidate_rule_revision_ref:r.candidate_rule_revision_ref,config_ref:r.config_ref,source_refs:refs,
            seed:17,ttl_seconds:86400,context_policy:linggan_intelligence::comment_runtime::ContextPolicy::parse(context["policy"].clone())?,
            thresholds:Default::default(),repeat_member_limit:20,authorized_token_budget:r.authorized_token_budget,
        }).await
    }.await)
}
