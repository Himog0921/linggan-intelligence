//! OPS-MIGRATION-001 D2: the read-only 「运营」 pages (PAGE-OPS-001). Server-rendered shell,
//! client-rendered tables over the OPS-HTTP-001 reads. Nothing here writes.
use super::*;
use linggan_intelligence::ops_ledger::read::{
    OpsAsOf, OpsBusiness, OpsBusinessOption, read_ops_as_of, read_ops_businesses,
};
use serde::Deserialize;
use shell::OpsPage;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/ops", get(entry))
        .route("/ops/overview", get(overview_page))
        .route("/ops/publications", get(publications_page))
        .route("/ops/accounts", get(accounts_page))
        .route("/ops/daily", get(daily_page))
        .route("/ops/io", get(io_page))
        .route("/assets/ops.css", get(css))
        .route("/assets/ops.js", get(js))
        .route("/assets/ops-overview.js", get(overview_js))
        .merge(super::ops_api::routes())
        .merge(super::ops_overview_api::routes())
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

#[derive(Deserialize)]
pub(super) struct BusinessQuery {
    pub(super) domain: Option<String>,
}

/// 点「运营」进入发布记录（选题池上线前的默认页）。
async fn entry(Query(query): Query<BusinessQuery>) -> Response {
    let business = OpsBusiness::parse(query.domain.as_deref()).unwrap_or(OpsBusiness::All);
    Redirect::to(&page_href(OpsPage::Publications, business)).into_response()
}

async fn overview_page(state: State<LocalWebState>, query: Query<BusinessQuery>) -> Response {
    page(state, query, OpsPage::Overview).await
}
async fn publications_page(state: State<LocalWebState>, query: Query<BusinessQuery>) -> Response {
    page(state, query, OpsPage::Publications).await
}
async fn accounts_page(state: State<LocalWebState>, query: Query<BusinessQuery>) -> Response {
    page(state, query, OpsPage::Accounts).await
}
async fn daily_page(state: State<LocalWebState>, query: Query<BusinessQuery>) -> Response {
    page(state, query, OpsPage::Daily).await
}
async fn io_page(state: State<LocalWebState>, query: Query<BusinessQuery>) -> Response {
    page(state, query, OpsPage::ImportExport).await
}

fn page_meta(page: OpsPage) -> (&'static str, &'static str, &'static str) {
    match page {
        OpsPage::Overview => ("/ops/overview", "总览", include_str!("ops_overview.html")),
        OpsPage::Publications => (
            "/ops/publications",
            "发布记录",
            include_str!("ops_publications.html"),
        ),
        OpsPage::Accounts => (
            "/ops/accounts",
            "账号管理",
            include_str!("ops_accounts.html"),
        ),
        OpsPage::Daily => ("/ops/daily", "每日数据", include_str!("ops_daily.html")),
        OpsPage::ImportExport => ("/ops/io", "导入导出", include_str!("ops_io.html")),
    }
}

fn business_param(business: OpsBusiness) -> String {
    match business {
        OpsBusiness::All => linggan_evidence::observation_domain::ALL_DOMAINS.to_owned(),
        OpsBusiness::Domain(domain_ref) => domain_ref.to_string(),
    }
}

fn page_href(page: OpsPage, business: OpsBusiness) -> String {
    format!("{}?domain={}", page_meta(page).0, business_param(business))
}

/// A stale or unknown business selection falls back to 「全部业务」 through a redirect, so the
/// address bar always names what the page shows.
async fn page(
    State(state): State<LocalWebState>,
    Query(query): Query<BusinessQuery>,
    page: OpsPage,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let (Ok(businesses), Ok(as_of)) = (
        read_ops_businesses(database).await,
        read_ops_as_of(database).await,
    ) else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "ops_read_unavailable",
        );
    };
    let business = match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(OpsBusiness::Domain(domain_ref))
            if !businesses.iter().any(|b| b.domain_ref == domain_ref) =>
        {
            return Redirect::to(&page_href(page, OpsBusiness::All)).into_response();
        }
        Ok(business) => business,
        Err(_) => return Redirect::to(&page_href(page, OpsBusiness::All)).into_response(),
    };
    Html(page_markup(page, business, &businesses, as_of.as_ref())).into_response()
}

pub(super) fn page_markup(
    page: OpsPage,
    business: OpsBusiness,
    businesses: &[OpsBusinessOption],
    as_of: Option<&OpsAsOf>,
) -> String {
    let (path, label, main) = page_meta(page);
    let picker = business_picker(businesses, business, path);
    let header = shell::global_header(
        shell::PrimarySurface::Ops,
        "迁入数据 · 只读",
        &format!("运营 / {picker} / <b>{label}</b>"),
        "<span>只读</span><span>迁入数据</span>",
        None,
    );
    let foot = as_of.map_or_else(
        || "尚未迁入数据".to_owned(),
        |as_of| {
            let beijing = &as_of.exported_at_beijing;
            format!(
                "数据截至 {}",
                html_escape(beijing.get(5..).unwrap_or(beijing))
            )
        },
    );
    let business = business_param(business);
    let nav = shell::ops_side_nav(page, Some(&business), &foot);
    include_str!("ops.html")
        .replace("{{TITLE}}", label)
        .replace("{{HEADER}}", &header)
        .replace("{{SIDE_NAV}}", &nav)
        .replace("{{MAIN}}", main)
        .replace("{{PAGE}}", path.trim_start_matches("/ops/"))
        .replace("{{SCRIPTS}}", scripts(page))
        .replace("{{BUSINESS}}", &business)
}

/// Same markup and classes as the shared domain picker, with business wording: 「全部业务」 plus
/// the domains that own accounts. It stays visible with one business so 「全部业务」 is reachable.
fn business_picker(businesses: &[OpsBusinessOption], current: OpsBusiness, action: &str) -> String {
    let all_selected = current == OpsBusiness::All;
    let mut current_label = "全部业务".to_owned();
    let mut current_meta = "全部账号".to_owned();
    let mut options = format!(
        r#"<a href="{action}?domain={all}"{selected} aria-label="全部业务"><span>全部业务</span><small>全部账号</small></a>"#,
        all = linggan_evidence::observation_domain::ALL_DOMAINS,
        selected = if all_selected {
            " aria-current=\"page\""
        } else {
            ""
        },
    );
    for option in businesses {
        let selected = current == OpsBusiness::Domain(option.domain_ref);
        let meta = format!("{} 个账号", option.account_count);
        if selected {
            current_label = option.name.clone();
            current_meta = meta.clone();
        }
        options.push_str(&format!(
            r#"<a href="{action}?domain={domain_ref}"{selected} aria-label="{name}，{meta}"><span>{name}</span><small>{meta}</small></a>"#,
            domain_ref = option.domain_ref,
            selected = if selected { " aria-current=\"page\"" } else { "" },
            name = html_escape(&option.name),
        ));
    }
    format!(
        r#"<details class="v7-domain-picker">
             <summary aria-label="切换业务"><span>{label}</span><small>{meta}</small><i aria-hidden="true"></i></summary>
             <nav aria-label="可选业务">{options}</nav>
           </details>"#,
        label = html_escape(&current_label),
        meta = html_escape(&current_meta),
    )
}

/// Every page loads the shared script; the overview adds its own charts and target dialog.
fn scripts(page: OpsPage) -> &'static str {
    match page {
        OpsPage::Overview => {
            r#"<script src="/assets/ops.js" defer></script><script src="/assets/ops-overview.js" defer></script>"#
        }
        _ => r#"<script src="/assets/ops.js" defer></script>"#,
    }
}

async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!(
            "{}\n{}\n{}\n{}",
            super::LIDS_TOKENS,
            super::SHELL_CSS,
            include_str!("ops.css"),
            include_str!("ops_overview.css")
        ),
    )
}

async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("ops.js"),
    )
}

async fn overview_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("ops_overview.js"),
    )
}

#[cfg(test)]
mod tests {
    use super::{OpsPage, page_markup};
    use linggan_intelligence::ops_ledger::read::{OpsAsOf, OpsBusiness, OpsBusinessOption};
    use uuid::Uuid;

    fn businesses() -> Vec<OpsBusinessOption> {
        vec![OpsBusinessOption {
            domain_ref: Uuid::from_u128(1),
            name: "ADHD".into(),
            account_count: 131,
        }]
    }

    #[test]
    fn every_ops_page_uses_the_shared_shell_with_the_business_crumb_and_side_nav() {
        let as_of = OpsAsOf {
            exported_at: "2026-10-12T14:43:32.384Z".into(),
            exported_at_beijing: "2026-10-12 22:43".into(),
            batch_ref: Uuid::from_u128(9),
        };
        for (page, label, path) in [
            (OpsPage::Overview, "总览", "overview"),
            (OpsPage::Publications, "发布记录", "publications"),
            (OpsPage::Accounts, "账号管理", "accounts"),
            (OpsPage::Daily, "每日数据", "daily"),
            (OpsPage::ImportExport, "导入导出", "io"),
        ] {
            let html = page_markup(page, OpsBusiness::All, &businesses(), Some(&as_of));
            assert!(
                html.contains("aria-current=\"page\"><b class=\"v7-nav-zh\">运营</b>"),
                "运营 is current"
            );
            assert!(
                html.contains(&format!("<b>{label}</b>")),
                "{label} is the last crumb"
            );
            assert!(
                html.contains("切换业务") && html.contains("全部业务"),
                "business picker in the crumb"
            );
            assert!(
                html.contains("href=\"/ops/overview?domain=all\"") && !html.contains("未开放"),
                "the overview is a link since D3"
            );
            assert_eq!(
                html.contains("/assets/ops-overview.js"),
                page == OpsPage::Overview,
                "only the overview loads its charts"
            );
            assert!(html.contains("/assets/ops.js"));
            assert!(
                html.contains("数据截至 10-12 22:43"),
                "side foot names the export time"
            );
            assert!(html.contains(&format!("data-ops-page=\"{path}\"")));
            assert!(!html.contains("{{"), "no unreplaced template slot");
        }
    }

    #[test]
    fn the_business_picker_stays_reachable_with_one_business_selected() {
        let html = page_markup(
            OpsPage::Accounts,
            OpsBusiness::Domain(Uuid::from_u128(1)),
            &businesses(),
            None,
        );
        assert!(html.contains("全部业务") && html.contains("131 个账号"));
        assert!(html.contains("尚未迁入数据") || html.contains("数据截至"));
        assert!(html.contains("/ops/accounts?domain=all"));
    }
}
