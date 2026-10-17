//! WEB-REACT-001 阶段 D2：证据库页面证明共用的合成夹具（仅测试编译）。
//! 只造合成作者、作品与材料；不调用模型、不访问平台、不碰开发库或本机常驻服务。
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
pub(super) mod fixture;

/// 本机请求头：与浏览器同源访问时服务器看到的 Host 一致（闸门要求本机主机名）。
pub(super) const LOCAL_HOST: &str = "127.0.0.1:3000";
const DEFAULT_OBSERVED_AT: &str = "2026-08-28T10:00:00Z";

/// 一次响应的原样记录：状态、纯文本、尝试解析出的 JSON（错误体可能是纯文本，不是 JSON）。
pub(super) struct Reply {
    pub(super) status: StatusCode,
    pub(super) text: String,
    pub(super) json: Value,
}

/// 前缀至多 15 个字符：存储层把方案名限制在 48 个字符内，后面还要接 32 位十六进制。
pub(super) async fn isolated(prefix: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("isolated proof URL supplied");
    let schema = format!("{prefix}_{}", Uuid::new_v4().simple());
    linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .expect("full migrations apply to isolated schema")
}

/// 以指定请求头发一次请求。`headers` 为空时不带 Host，用来证明闸门的拒绝。
pub(super) async fn send(
    app: Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> Reply {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    if body.is_some() {
        builder = builder.header("Content-Type", "application/json");
    }
    let response = app
        .oneshot(
            builder
                .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 8 << 20).await.unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply { status, text, json }
}

/// 同源本机 GET（带本机 Host）。
pub(super) async fn get(app: &Router, uri: &str) -> Reply {
    send(app.clone(), "GET", uri, &[("Host", LOCAL_HOST)], None).await
}

/// 同源本机 POST（带本机 Host）；`body` 为 `None` 时不带请求体。
pub(super) async fn post(app: &Router, uri: &str, body: Option<Value>) -> Reply {
    send(app.clone(), "POST", uri, &[("Host", LOCAL_HOST)], body).await
}

/// 一篇合成作品的可变部分。其余字段取固定默认值。
pub(super) struct Work<'a> {
    pub(super) external: &'a str,
    pub(super) title: &'a str,
    pub(super) body: &'a str,
    pub(super) author_id: &'a str,
    pub(super) author_name: &'a str,
    pub(super) likes: i64,
    /// 平台显示的评论数；`None` 表示来源没有给出。
    pub(super) public_comments: Option<i64>,
    pub(super) collects: i64,
    pub(super) shares: i64,
    /// 来源发布时间（毫秒时间戳）；`None` 表示来源没有给出发布时间。
    pub(super) published_ms: Option<i64>,
    /// 详情材料的观察时刻；决定默认列表里的先后。
    pub(super) observed_at: &'a str,
}

impl<'a> Work<'a> {
    pub(super) fn new(external: &'a str, title: &'a str) -> Self {
        Self {
            external,
            title,
            body: "合成正文：先做两分钟，再决定要不要继续。",
            author_id: "author-01",
            author_name: "合成作者 01",
            likes: 10,
            public_comments: None,
            collects: 0,
            shares: 0,
            published_ms: None,
            observed_at: DEFAULT_OBSERVED_AT,
        }
    }
}

fn detail_payload(work: &Work<'_>) -> Value {
    let mut payload = json!({
        "title": work.title,
        "bodyText": work.body,
        "authorId": work.author_id,
        "authorName": work.author_name,
        "likedCount": work.likes,
        "collectedCount": work.collects,
        "shareCount": work.shares
    });
    let map = payload.as_object_mut().unwrap();
    if let Some(comments) = work.public_comments {
        map.insert("publicCommentCount".into(), json!(comments));
    }
    if let Some(published) = work.published_ms {
        map.insert("publishedAt".into(), json!(published));
        map.insert(
            "publishedAtText".into(),
            json!((published / 1000).to_string()),
        );
        map.insert("publishedAtSourceField".into(), json!("publishTime"));
        map.insert("publishedAtSourceKind".into(), json!("platform_epoch"));
        map.insert("publishedAtPrecision".into(), json!("second"));
        map.insert(
            "publishedAtParserVersion".into(),
            json!("xhs-detail-time-v2"),
        );
    }
    payload
}

/// 写入一篇作品的详情材料并把它归到某个领域的主研究用途下，返回作品稳定引用。
pub(super) async fn seed_work(db: &Database, domain: Uuid, work: &Work<'_>) -> Uuid {
    let package = fixture::submit_package_at(
        db,
        "content_detail",
        json!({"contentExternalId": work.external}),
        json!({
            "kind": "content_detail",
            "sourceObject": {"platform": "xhs", "type": "content", "externalId": work.external},
            "payload": detail_payload(work)
        }),
        work.observed_at,
    )
    .await;
    let reference = public_ref(db, work.external).await;
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4())
        .bind(reference)
        .bind(domain)
        .bind(package)
        .execute(db.pool())
        .await
        .unwrap();
    reference
}

pub(super) async fn public_ref(db: &Database, external: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(external)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

pub(super) async fn create_domain(db: &Database, name: &str) -> Uuid {
    linggan_evidence::observation_domain::create_observation_domain_with_details(
        db, name, None, None,
    )
    .await
    .expect("domain")
    .domain_ref
}

pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

pub(super) async fn count(db: &Database, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap()
}

/// 页面路径里本不该发生的副作用：采集请求、工单、模型调用。
pub(super) async fn side_effects(db: &Database) -> (i64, i64, i64) {
    (
        count(db, "SELECT count(*) FROM collection_acquisition_request").await,
        count(db, "SELECT count(*) FROM collection_work_order").await,
        count(db, "SELECT count(*) FROM linggan_model_invocation").await,
    )
}

/// 合成图片：竖版或横版的渐变 PNG，便于肉眼看出缩放、平移与 3:4 裁切。
pub(super) fn png_bytes(width: u32, height: u32, tint: [u8; 3]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let mut data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                data.push(tint[0].saturating_add((x * 100 / width) as u8));
                data.push(tint[1].saturating_add((y * 100 / height) as u8));
                data.push(tint[2]);
            }
        }
        writer.write_image_data(&data).unwrap();
    }
    bytes
}

/// 把字节写进受控媒体目录并登记为已接纳的字节；返回接纳回执（含本机句柄）。
pub(super) async fn admit_bytes(
    db: &Database,
    observation_ref: Uuid,
    bytes: &[u8],
    mime: &str,
) -> linggan_evidence::MediaBlobAdmission {
    let sha: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let key = format!("blobs/{}/{sha}", &sha[..2]);
    let path = local_media_root().join(&key);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    linggan_evidence::admit_media_blob(db, observation_ref, &sha, mime, bytes.len() as i64, &key)
        .await
        .expect("synthetic media bytes are admitted")
}
