//! WEB-REACT-001 阶段 D2（DR-4）：证据库依赖的四条接口经过本机来源闸门——作品列表、作品详情、立即复观测
//! 与复观测状态。复观测会创建工单，而无请求体的 `POST` 是浏览器的「简单请求」，跨站网页可以直接发出，
//! 不经预检，所以闸门是这条写路径唯一的来源防线。这里不需要数据库：闸门在读库之前判断。
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;

const DOMAIN: &str = "00000000-0000-4000-8000-000000000001";
const WORK: &str = "00000000-0000-4000-8000-0000000000aa";
const LEASE: &str = "00000000-0000-4000-8000-0000000000bb";

fn routes() -> [(&'static str, String); 4] {
    let root = "/api/local/work-resources";
    [
        ("GET", format!("{root}?domain={DOMAIN}")),
        ("GET", format!("{root}/{WORK}?domain={DOMAIN}")),
        ("POST", format!("{root}/{WORK}/reobserve?domain={DOMAIN}")),
        (
            "GET",
            format!("{root}/{WORK}/reobserve/{LEASE}?domain={DOMAIN}"),
        ),
    ]
}

async fn call(method: &str, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, bool, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let no_store = response
        .headers()
        .get(header::CACHE_CONTROL)
        .is_some_and(|value| value == "no-store");
    let body = to_bytes(response.into_body(), 1 << 16).await.unwrap();
    (
        status,
        no_store,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

/// 不是「本机页面同源发起」的请求一律 403，并且在读库之前就被拒绝。
#[tokio::test]
async fn evidence_routes_reject_requests_that_are_not_same_origin_local() {
    let hostile: [(&str, &[(&str, &str)]); 5] = [
        ("没有 Host", &[]),
        ("非本机主机名", &[("Host", "example.com:3000")]),
        (
            "跨站发起",
            &[("Host", "127.0.0.1:3000"), ("Sec-Fetch-Site", "cross-site")],
        ),
        (
            "同站不同源",
            &[("Host", "localhost:3000"), ("Sec-Fetch-Site", "same-site")],
        ),
        (
            "Origin 不是本机页面",
            &[
                ("Host", "127.0.0.1:3000"),
                ("Origin", "https://untrusted.example"),
            ],
        ),
    ];
    for (method, uri) in routes() {
        for (label, headers) in hostile {
            let (status, _, body) = call(method, &uri, headers).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}（{label}）");
            assert!(
                body.contains("local_comment_study_origin_rejected"),
                "{method} {uri}（{label}）：{body}"
            );
        }
    }
}

/// 本机页面同源发起、地址栏直接访问（`Sec-Fetch-Site: none`）与不带来源头的本机调用都通过闸门；
/// 通过之后才轮到读库（这里没有连库，所以是 503，而不是 403），响应带 `no-store`。
#[tokio::test]
async fn evidence_routes_accept_same_origin_local_requests() {
    let accepted: [(&str, &[(&str, &str)]); 4] = [
        (
            "同源",
            &[
                ("Host", "127.0.0.1:3000"),
                ("Sec-Fetch-Site", "same-origin"),
            ],
        ),
        (
            "地址栏直接访问",
            &[("Host", "localhost:3000"), ("Sec-Fetch-Site", "none")],
        ),
        ("不带来源头的本机调用", &[("Host", "127.0.0.1:3000")]),
        (
            "Origin 就是本机页面",
            &[
                ("Host", "127.0.0.1:3000"),
                ("Origin", "http://127.0.0.1:3000"),
            ],
        ),
    ];
    for (method, uri) in routes() {
        for (label, headers) in accepted {
            let (status, no_store, body) = call(method, &uri, headers).await;
            assert_ne!(
                status,
                StatusCode::FORBIDDEN,
                "{method} {uri}（{label}）：{body}"
            );
            assert_eq!(
                status,
                StatusCode::SERVICE_UNAVAILABLE,
                "{method} {uri}（{label}）：{body}"
            );
            assert!(no_store, "{method} {uri}（{label}）：响应应带 no-store");
        }
    }
}
