//! The user-work view. Original technical projections remain in the diagnosis disclosure.
use super::collection_task_actions::TaskActionParams;
use linggan_evidence::{
    collection_run::{RunQuery, list_runs},
    observation_domain::read_observation_domains,
};
use linggan_storage_postgres::Database;
use serde_json::Value;

fn e(v: &str) -> String {
    super::html_escape(v)
}
fn text(v: &Value, key: &str) -> String {
    v[key].as_str().map(e).unwrap_or_else(|| "未知".into())
}
fn number(v: &Value, key: &str) -> String {
    v[key]
        .as_i64()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "—".into())
}
fn discovery_label(reason: Option<&str>) -> &'static str {
    match reason {
        Some("surface_ended") => "本轮可见作品已发现完毕",
        Some("maximum_quota") => "已达到本次作品上限",
        Some("accepted_discovery_scope") => "已固定本次发现结果",
        Some("person_selected_exact_set") => "沿用勾选的作品集合",
        Some("discovery_execution_stopped") => "发现未完成，需要处理",
        Some(_) => "发现原因见执行诊断",
        None => "等待发现结果",
    }
}
pub(super) async fn render(db: Option<&Database>, base: &str, p: &TaskActionParams) -> String {
    let filter = p.filter.as_deref().unwrap_or("all");
    let q = p.q.as_deref().unwrap_or("");
    let domain = p.nav().and_then(|d| uuid::Uuid::parse_str(d).ok());
    let read = match db {
        Some(db) if p.nav().is_none() || p.nav() == Some("all") || domain.is_some() => list_runs(
            db,
            &RunQuery {
                domain_ref: domain,
                filter: Some(filter.into()),
                q: Some(q.into()),
                page: p.page,
            },
        )
        .await
        .ok(),
        _ => None,
    };
    let mut filters = String::new();
    for (key, label) in [
        ("all", "全部"),
        ("active", "进行中"),
        ("attention", "需我处理"),
        ("results", "已有成果"),
        ("ended", "已结束"),
    ] {
        filters.push_str(&format!(r#"<button class="c-run-filter" type="submit" name="filter" value="{key}" aria-current="{current}">{label}</button>"#,current=if key==filter{"page"}else{"false"}));
    }
    let domain_field = p
        .nav()
        .map(|d| format!(r#"<input type="hidden" name="domain" value="{}">"#, e(d)))
        .unwrap_or_default();
    let mut rows = String::new();
    if let Some(items) = read.as_ref().and_then(|v| v["items"].as_array()) {
        for r in items {
            let state = if r["control_intent"] == "stopped" {
                "已停止"
            } else if r["control_intent"] == "paused" {
                "已暂停"
            } else if r["phase"] == "blocked" {
                "需处理"
            } else if r["ended"] == true && r["needs_attention"] == true {
                "本轮结束，有缺口"
            } else if r["ended"] == true {
                "本轮已结束"
            } else if r["domain_status"] == "paused" {
                "领域暂停"
            } else if r["live_leases"].as_i64().is_some_and(|n| n > 0) {
                "执行中"
            } else {
                "等待派发"
            };
            let source = match r["source_kind"].as_str() {
                Some("temporary_keyword") => "临时关键词",
                Some("observation_target") => "观察目标",
                Some("selected_works") => "语料已选作品",
                Some("archive") => "建档",
                Some("patrol") => "本轮巡查",
                Some("supplement") => "显式补采",
                _ => "既有采集",
            };
            let scope = if r["legacy"] == true {
                "按原工单范围"
            } else if r["comment_mode"] == "all_public" {
                "尽量采全，含回复"
            } else {
                "每篇最多 30 条总样本"
            };
            rows.push_str(&format!(r#"<tr><td><button type="button" class="c-btn-quiet c-run-title" data-open-run="{id}" data-run-domain="{domain}" title="{name}">{name}</button><small>{source} · {domain_name}</small></td><td>{scope}<small>最多 {limit} 篇</small></td><td>{details} 篇详情<small>{comments} 条评论 · 复用 {reuse} 篇详情</small></td><td>{state}<small>已固定 {works} 篇 · {reason}</small></td><td><time datetime="{created}">{created}</time><small><time datetime="{updated}">{updated}</time></small></td><td><button class="c-btn-secondary" type="button" data-open-run="{id}" data-run-domain="{domain}">查看进展</button></td></tr>"#,
                id=text(r,"run_ref"),domain=text(r,"domain_ref"),name=text(r,"name"),domain_name=text(r,"domain_name"),limit=number(r,"work_limit"),details=number(r,"available_details"),comments=number(r,"delivered_comments"),reuse=number(r,"reused_details"),works=if r["scope_frozen_at"].is_null() && r["legacy"]!=true {"—".into()}else{number(r,"selected_works")},reason=discovery_label(r["discovery_reason"].as_str()),created=text(r,"created_at"),updated=r["latest_delivery_at"].as_str().map(e).unwrap_or_else(||text(r,"updated_at"))));
        }
    }
    let empty = if read.is_none() {
        "任务读取失败；不能据此认定没有任务。"
    } else {
        "当前筛选没有采集记录。"
    };
    if rows.is_empty() {
        rows = format!(r#"<tr><td colspan="6" class="c-run-empty">{empty}</td></tr>"#);
    }
    let count = read
        .as_ref()
        .map(|v| number(v, "total"))
        .unwrap_or_else(|| "—".into());
    let page = p.page.unwrap_or(1).max(1);
    let modal = modal(db, p).await;
    let new_body = format!(
        r#"<section data-collection-runs><form method="get" action="/collection/tasks" class="c-run-filters">{domain_field}<input type="hidden" name="q" value="{q}">{filters}</form><div class="c-run-toolbar"><form method="get" action="/collection/tasks" class="c-run-tools">{domain_field}<input type="hidden" name="filter" value="{filter}"><label><span class="c-run-sr">搜索采集任务</span><input type="search" name="q" value="{q}" placeholder="搜索关键词、目标或任务编号"></label><button class="c-btn-secondary" type="submit">搜索</button></form><button id="new-collection-run" class="c-btn-primary c-run-primary" type="button" data-new-run>＋ 新建单次采集</button></div><div class="c-run-scroll"><table class="c-run-table"><thead><tr><th>任务名称／来源</th><th>方式／范围</th><th>可用成果</th><th>本轮进展／下一步</th><th>发起／最近进展</th><th>操作</th></tr></thead><tbody>{rows}</tbody></table></div><form class="c-run-pagination" method="get">{domain_field}<input type="hidden" name="filter" value="{filter}"><input type="hidden" name="q" value="{q}"><span>共 {count} 次采集 · 第 {page} 页</span><button class="c-btn-quiet" name="page" value="{prev}" {prev_disabled}>上一页</button><button class="c-btn-quiet" name="page" value="{next}" {next_disabled}>下一页</button></form></section>{modal}<dialog class="c-run-dialog c-run-drawer" id="collection-run-drawer" aria-labelledby="collection-run-title"><header><h2 id="collection-run-title">采集进展</h2><div class="c-run-controls"><button type="button" class="c-btn-quiet" data-expand-run aria-label="调整详情宽度">展开</button><button type="button" class="c-btn-quiet" data-close-run-dialog>关闭</button></div></header><nav class="c-run-tabs" aria-label="任务详情"><button type="button" data-run-view="progress" aria-current="page">进展</button><button type="button" data-run-view="results">成果</button></nav><div class="c-run-dialog-body" data-run-detail></div><footer><span data-run-footer-state></span><button type="button" class="c-btn-primary c-run-primary" data-run-view="results">查看成果</button></footer></dialog>"#,
        q = e(q),
        prev = page.saturating_sub(1).max(1),
        next = page + 1,
        prev_disabled = if page <= 1 { "disabled" } else { "" },
        next_disabled = if read
            .as_ref()
            .and_then(|v| v["total"].as_i64())
            .is_none_or(|n| n <= page * 20)
        {
            "disabled"
        } else {
            ""
        }
    );
    let start = "<!-- collection-body:start -->";
    let end = "<!-- collection-body:end -->";
    let mut result = base.to_owned();
    if let (Some(a), Some(b)) = (result.find(start), result.find(end)) {
        let original = result[a + start.len()..b].to_owned();
        result.replace_range(a+start.len()..b,&format!("{new_body}<details class=\"c-run-diagnostics\"><summary>历史执行记录与诊断</summary>{original}</details>"));
    }
    let readout = super::collection::context_readout(&[(
        "采集记录",
        &count,
        "同一领域、搜索、状态范围；列表与分页一致",
    )]);
    super::collection::replace_bounded_slot(
        &result,
        super::collection::READOUT_SLOT_START,
        super::collection::READOUT_SLOT_END,
        &readout,
    )
}
async fn modal(db: Option<&Database>, p: &TaskActionParams) -> String {
    let mut options = r#"<option value="">请选择本次领域</option>"#.to_owned();
    if let Some(db) = db {
        if let Ok(domains) = read_observation_domains(db).await {
            for d in domains {
                if d.status == "active" {
                    options.push_str(&format!(
                        r#"<option value="{id}"{selected}>{name}</option>"#,
                        id = d.domain_ref,
                        name = e(&d.name),
                        selected = if p.nav() == Some(&d.domain_ref.to_string()) {
                            " selected"
                        } else {
                            ""
                        }
                    ));
                }
            }
        }
    }
    format!(
        r#"<dialog class="c-run-dialog" id="collection-run-form" aria-labelledby="collection-run-form-title"><form data-create-run><header><div><h2 id="collection-run-form-title">新建单次采集</h2><p>为当前问题取得一批材料，结束后不自动重复。</p></div><button class="c-btn-quiet" type="button" data-close-run-dialog aria-label="关闭单次采集">×</button></header><div class="c-run-dialog-body"><fieldset><legend>采集来源</legend><div class="c-run-source"><label><input type="radio" name="source" value="temporary_keyword" checked>临时关键词</label><label><input type="radio" name="source" value="observation_target">已有观察目标</label></div></fieldset><label>本次归属领域 *<select name="domain" required>{options}</select></label><label data-keyword-field>这次想找什么 *<input name="query" maxlength="80" placeholder="例如：对立违抗" required></label><section data-target-field hidden><label>搜索本领域观察目标<input type="search" name="target_search"></label><select name="target" aria-label="选择观察目标"></select><button type="button" class="c-btn-quiet" data-more-targets>加载下一页</button></section><label data-ranking-field>搜索排序<select name="ranking"><option value="comprehensive">综合</option><option value="latest">最新</option><option value="most_liked">最多点赞</option><option value="most_collected">最多收藏</option><option value="most_commented">最多评论</option></select></label><label><span data-run-work-label>发现作品上限</span><input name="work_limit" type="number" value="50" min="1" max="200" required><small data-run-work-help>1–200 篇；实际发现可能更少，已有作品仍占本次位置。</small></label><label>评论范围<select name="comment_mode"><option value="sample_30">每篇最多 30 条评论样本，含回复</option><option value="all_public">尽量采全，含回复</option></select></label><p>临时采集不加入长期名单；本次参数不修改已有目标的周期规则。</p><p role="alert" data-run-form-feedback></p></div><footer><button type="button" class="c-btn-quiet" data-close-run-dialog>取消</button><button type="submit" class="c-btn-primary c-run-primary">▷ 发起采集</button></footer></form></dialog>"#
    )
}
