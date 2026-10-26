//! WEB-REACT-001 阶段 D2：证据库页面证明用的合成数据集（仅测试编译）。
//! 一个领域里放 57 篇作品（超过一页 50 条，翻页有内容），其中几篇各自代表一种状态：
//! 长文＋多图＋评论＋图片文字、视频、风险控制停止、部分取得、字节已清理、撤回或受限、可复观测。
use super::evidence_fixture::{
    Work, admit_bytes, create_domain, fixture, now_ms, png_bytes, seed_work,
};
use super::*;
use linggan_evidence::{
    AccountEligibilityObservation, MaterialMediaDisposition, activate_installation_credential,
    bind_observation_account, record_materialization_disposition, report_account_eligibility,
    rotate_installation_credential, set_station_accepting,
};
use uuid::Uuid;

const DAY_MS: i64 = 86_400_000;
/// 第一页 50 条之外还要有内容：49 篇普通作品加 8 篇特殊作品，共 57 篇（含一篇很久之前的）。
pub(super) const PLAIN_WORKS: usize = 49;

pub(super) struct Dataset {
    pub(super) domain_one: Uuid,
    pub(super) domain_two: Uuid,
    pub(super) rich: Uuid,
    pub(super) video: Uuid,
    pub(super) risk: Uuid,
    pub(super) partial: Uuid,
    pub(super) cleaned: Uuid,
    pub(super) restricted: Uuid,
    pub(super) reobserve: Uuid,
    /// 观察时刻最早：不在第一页，用来证明地址里的 `work` 会直读而不是回退到首项。
    pub(super) off_page: Uuid,
}

fn long_body() -> String {
    let paragraph = "合成长文段落：先把任务缩到两分钟，只做第一步；做完再决定要不要继续。\
                     这一段用来检查长文在阅读区的折行、折叠与展开，不含任何真实内容。";
    let mut text = String::new();
    for index in 1..=40 {
        text.push_str(&format!("第{index}段。{paragraph}\n"));
    }
    text.push_str("合成长文结尾标记。");
    text
}

async fn slot_package(db: &Database, external: &str, slots: &[(&str, i32)]) -> Vec<Uuid> {
    let target = json!({"contentExternalId": external});
    let mut observations = Vec::new();
    let mut records = Vec::new();
    for (role, ordinal) in slots {
        let observation = Uuid::new_v4();
        observations.push(observation);
        let (subject, key) = if *role == "avatar" {
            (
                "author",
                format!("xhs:author:{external}-author:avatar:{ordinal}"),
            )
        } else {
            ("content", format!("xhs:{external}:{role}:{ordinal}"))
        };
        let uri = format!("https://media.example/{external}/{role}/{ordinal}");
        let mut record = json!({
            "kind":"media_slot","slotKey":key,"observationRef":observation,
            "slot":{"role":role,"ordinal":ordinal},
            "observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-10T08:00:00Z"},
            "sourceObject":{"platform":"xhs","type":subject,"externalId":if subject=="author"{format!("{external}-author")}else{external.to_owned()}}
        });
        if subject == "author" {
            record["contextContentExternalId"] = json!(external);
        }
        records.push(record);
    }
    let coverage = json!({"target": target, "layers":[fixture::coverage_layer("media_slots", slots.len() as i64)]});
    fixture::submit_custom_package(
        db,
        "xhs",
        &["media_slots"],
        target,
        "media_slots",
        "xhs",
        coverage,
        records,
    )
    .await;
    observations
}

async fn complete_first_image_ocr(db: &Database) {
    let row:(Uuid,String,Uuid,String)=sqlx::query_as("SELECT o.content_public_ref,o.slot_key,m.materialization_ref,m.blob_sha256 FROM linggan_material_media_origin o JOIN linggan_media_download_attempt a ON a.media_observation_ref=o.observation_ref JOIN linggan_media_materialization m USING(download_attempt_ref) JOIN linggan_media_blob b ON b.sha256=m.blob_sha256 WHERE b.mime_type LIKE 'image/%' ORDER BY o.created_at LIMIT 1").fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    linggan_evidence::material_reading::publish_in(
        &mut tx,
        linggan_evidence::material_reading::NewReading {
            content_public_ref: row.0,
            unit_key: format!("image:{}", row.1),
            kind: "image".into(),
            slot_key: Some(row.1.clone()),
            page: None,
            source_version: row.2.to_string(),
            source_state: "present".into(),
            derivative_ref: None,
            producer_ref: Some(Uuid::new_v4()),
            generated_by: "cloud_vision".into(),
            rule_version: "dossier-content.v2:synthetic-current".into(),
            blocks: vec![
                json!({"kind":"paragraph","text":"图片正文完整保留。","speaker":"unconfirmed"}),
            ],
            issues: vec![],
            source_location: json!({"slotKey":row.1,"blobSha256":row.3,"materializationRef":row.2}),
            publication: linggan_evidence::material_reading::Publication::Automatic {
                expected_current: None,
            },
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

/// 评论：根评论一个包，回复一个包（回复指向第一条根评论）。
async fn seed_comments(db: &Database, external: &str, roots: usize) {
    let target = json!({"contentExternalId": external});
    let source = json!({"platform":"xhs","type":"content","externalId":external});
    let record =
        |kind: &str, payload: Value| json!({"kind":kind,"sourceObject":source,"payload":payload});
    let root_records: Vec<Value> = (0..roots)
        .map(|index| {
            record(
                "comment",
                json!({
                    "commentId":format!("c{index}"),"noteId":external,
                    "text":format!("合成评论 {index}：我也是这样开始的，第一周最难。"),
                    "authorId":format!("reader-{index}"),"likes":(index % 9) as i64
                }),
            )
        })
        .collect();
    let coverage =
        json!({"target": target, "layers":[fixture::coverage_layer("comments", roots as i64)]});
    fixture::submit_custom_package(
        db,
        "xhs",
        &["comments"],
        target.clone(),
        "comments",
        "xhs",
        coverage,
        root_records,
    )
    .await;
    let replies: Vec<Value> = (0..3)
        .map(|index| {
            record("reply", json!({
                "commentId":format!("r{index}"),"noteId":external,
                "text":format!("合成回复 {index}：谢谢分享。"),"authorId":format!("replier-{index}"),
                "likes":2,"parentCommentId":"c0","rootCommentId":"c0"
            }))
        })
        .collect();
    let coverage = json!({"target": target, "layers":[fixture::coverage_layer("replies", 3)]});
    fixture::submit_custom_package(
        db,
        "xhs",
        &["replies"],
        target,
        "replies",
        "xhs",
        coverage,
        replies,
    )
    .await;
}

/// 发现通道：同一个作品出现在一次主页发现里。`stopped` 决定通道状态（风险控制停止或部分取得）。
async fn seed_discovery(db: &Database, external: &str, stopped: &str, not_attempted: i64) {
    let target = json!({"authorExternalId":"creator-discovery"});
    let layer = json!({
        "capability":"profile_discovery","observed":3,"attempted":1,"acquired":1,"verified":0,
        "failed":0,"notAttempted":not_attempted,"unknown":0,"stoppedReason":stopped
    });
    let url = format!(
        "https://www.xiaohongshu.com/explore/{external}?xsec_token=fixture-token&xsec_source=pc_feed"
    );
    // 只有「已知集合」才允许写出「没有尝试」的剩余数。
    let coverage_target = json!({"basis":"known_set","authorExternalId":"creator-discovery"});
    fixture::submit_custom_package(
        db,
        "xhs",
        &["profile_discovery"],
        target,
        "profile_discovery",
        "xhs",
        json!({"target":coverage_target,"layers":[layer]}),
        vec![json!({"kind":"profile_discovery_card","resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":external},
            "payload":{"title":"合成发现卡","url":url}})],
    )
    .await;
}

fn work_at<'a>(external: &'a str, title: &'a str, observed_at: &'a str) -> Work<'a> {
    let mut work = Work::new(external, title);
    work.observed_at = observed_at;
    work
}

async fn seed_rich(db: &Database, domain: Uuid, now: i64) -> Uuid {
    let body = long_body();
    let mut work = work_at(
        "ev-rich",
        "合成长文：从两分钟开始建立习惯",
        "2026-10-10T09:00:00Z",
    );
    work.body = &body;
    work.author_id = "ev-rich-author";
    work.author_name = "合成作者 林";
    work.likes = 12_345;
    work.public_comments = Some(321);
    work.collects = 4_567;
    work.shares = 89;
    work.published_ms = Some(now - 3 * DAY_MS);
    let reference = seed_work(db, domain, &work).await;
    // 先接纳第一张正文图并完成识字，保证认领到的就是它；其余字节随后接纳。
    let first = slot_package(db, "ev-rich", &[("image", 1)]).await;
    admit_bytes(
        db,
        first[0],
        &png_bytes(240, 320, [60, 90, 140]),
        "image/png",
    )
    .await;
    complete_first_image_ocr(db).await;
    let rest = slot_package(
        db,
        "ev-rich",
        &[("cover", 1), ("image", 2), ("image", 3), ("avatar", 1)],
    )
    .await;
    let looks = [
        ([160, 70, 60], 240, 320),
        ([70, 150, 90], 320, 240),
        ([150, 120, 50], 300, 300),
        ([120, 70, 150], 96, 96),
    ];
    for (observation, (tint, width, height)) in rest.iter().zip(looks) {
        admit_bytes(
            db,
            *observation,
            &png_bytes(width, height, tint),
            "image/png",
        )
        .await;
    }
    seed_comments(db, "ev-rich", 45).await;
    reference
}

async fn seed_video(db: &Database, domain: Uuid) -> Uuid {
    let mut work = work_at(
        "ev-video",
        "合成视频作品：三十秒示范",
        "2026-10-10T08:50:00Z",
    );
    work.author_id = "ev-video-author";
    work.author_name = "合成作者 视频";
    let reference = seed_work(db, domain, &work).await;
    // 封面与正文第 1 张是同一份文件（小红书的封面就是第一张图）：两个槽位、同一字节指纹，图组只显示一张。
    let observations =
        slot_package(db, "ev-video", &[("cover", 1), ("image", 1), ("video", 1)]).await;
    let cover = png_bytes(240, 320, [100, 100, 160]);
    for observation in &observations[..2] {
        admit_bytes(db, *observation, &cover, "image/png").await;
    }
    admit_bytes(
        db,
        observations[2],
        b"synthetic-video-bytes-not-playable",
        "video/mp4",
    )
    .await;
    reference
}

/// 带一张图的作品，字节随后按指定处置结果改状态。
async fn seed_dispositioned(
    db: &Database,
    domain: Uuid,
    external: &str,
    title: &str,
    observed_at: &str,
    disposition: MaterialMediaDisposition,
) -> Uuid {
    let reference = seed_work(db, domain, &work_at(external, title, observed_at)).await;
    let observations = slot_package(db, external, &[("image", 1)]).await;
    let admission = admit_bytes(
        db,
        observations[0],
        &png_bytes(240, 320, [90, 90, 90]),
        "image/png",
    )
    .await;
    record_materialization_disposition(
        db,
        admission.materialization_ref,
        disposition,
        "proof-authority",
        "synthetic-proof",
    )
    .await
    .unwrap();
    reference
}

async fn seed_flagged(
    db: &Database,
    domain: Uuid,
    external: &str,
    title: &str,
    observed_at: &str,
    stopped: &str,
    not_attempted: i64,
) -> Uuid {
    let reference = seed_work(db, domain, &work_at(external, title, observed_at)).await;
    seed_discovery(db, external, stopped, not_attempted).await;
    reference
}

/// 完整的数据集。`now` 只用于相对的发布时间。
pub(super) async fn seed_dataset(db: &Database) -> Dataset {
    let now = now_ms();
    let domain_one = create_domain(db, "合成领域一").await;
    let domain_two = create_domain(db, "合成领域二").await;
    for index in 0..PLAIN_WORKS {
        let external = format!("ev-plain-{index:02}");
        let title = format!("合成作品 {index:02}");
        let observed = format!("2026-10-01T00:{index:02}:00Z");
        let mut work = work_at(&external, &title, &observed);
        let author = format!("author-{:02}", index % 5 + 1);
        let name = format!("合成作者 {:02}", index % 5 + 1);
        work.author_id = &author;
        work.author_name = &name;
        work.likes = (index as i64) * 10;
        work.public_comments = Some(index as i64);
        seed_work(db, domain_one, &work).await;
    }
    let off_page = seed_work(
        db,
        domain_one,
        &work_at("ev-off-page", "合成作品 很久之前", "2026-09-01T00:00:00Z"),
    )
    .await;
    for index in 1..=2 {
        let external = format!("ev-second-{index}");
        let title = format!("第二领域作品 {index}");
        seed_work(
            db,
            domain_two,
            &work_at(&external, &title, "2026-10-02T00:00:00Z"),
        )
        .await;
    }
    let rich = seed_rich(db, domain_one, now).await;
    let video = seed_video(db, domain_one).await;
    let risk = seed_flagged(
        db,
        domain_one,
        "ev-risk",
        "合成作品 风险控制停止",
        "2026-10-10T08:40:00Z",
        "risk_control",
        2,
    )
    .await;
    let partial = seed_flagged(
        db,
        domain_one,
        "ev-partial",
        "合成作品 部分取得",
        "2026-10-10T08:30:00Z",
        "maximum_quota",
        2,
    )
    .await;
    let cleaned = seed_dispositioned(
        db,
        domain_one,
        "ev-cleaned",
        "合成作品 字节已清理",
        "2026-10-10T08:20:00Z",
        MaterialMediaDisposition::BytesCleaned,
    )
    .await;
    let restricted = seed_dispositioned(
        db,
        domain_one,
        "ev-restricted",
        "合成作品 撤回或受限",
        "2026-10-10T08:10:00Z",
        MaterialMediaDisposition::WithdrawnOrRestricted,
    )
    .await;
    let reobserve = seed_reobservable(db, domain_one).await;
    seed_model_config(db, domain_one).await;
    Dataset {
        domain_one,
        domain_two,
        rich,
        video,
        risk,
        partial,
        cleaned,
        restricted,
        reobserve,
        off_page,
    }
}

/// 可复观测：作品有详情、有带地址的发现来源，并有一份关联到它的有效深度归档授权（与
/// `crates/evidence/tests/content_reobservation_postgres.rs` 同一套前置）。
async fn seed_reobservable(db: &Database, domain: Uuid) -> Uuid {
    let reference = seed_work(
        db,
        domain,
        &work_at("ev-reobserve", "合成作品 可复观测", "2026-10-10T08:00:00Z"),
    )
    .await;
    seed_discovery_for_creator(db, "ev-reobserve").await;
    seed_authorization(db, domain, reference).await;
    reference
}

async fn seed_discovery_for_creator(db: &Database, external: &str) {
    let target = json!({"authorExternalId":"creator-reobserve"});
    let url = format!(
        "https://www.xiaohongshu.com/explore/{external}?xsec_token=fixture-token&xsec_source=pc_feed"
    );
    fixture::submit_custom_package(
        db,
        "xhs",
        &["profile_discovery"],
        target.clone(),
        "profile_discovery",
        "xhs",
        json!({"target":target,"layers":[fixture::coverage_layer("profile_discovery", 1)]}),
        vec![json!({"kind":"profile_discovery_card","resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":external},
            "payload":{"title":"可执行来源","url":url}})],
    )
    .await;
}

async fn seed_authorization(db: &Database, domain: Uuid, content_public_ref: Uuid) {
    let target_ref = Uuid::new_v4();
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    let station_ref = Uuid::new_v4();
    let installation_ref = Uuid::new_v4();
    let install_key = Uuid::new_v4().to_string();
    let pool = db.pool();
    sqlx::query("INSERT INTO collection_observation_target (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) VALUES ($1,'xhs','creator','creator-reobserve','复观测夹具','manual','archived')").bind(target_ref).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO observation_domain_target (domain_ref,target_ref,role) VALUES ($1,$2,'primary')").bind(domain).bind(target_ref).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_authorization (authorization_ref,platform,target_kind,lane,max_targets,max_works_per_target,allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at) VALUES ($1,'xhs','creator','deep_archive',10,20,ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],20,'content reobservation proof','person',scope_001_now()+interval '1 day')").bind(authorization_ref).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_request (request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) VALUES ($1,$2,$3,'primary','deep_archive','content reobservation proof','person')").bind(request_ref).bind(target_ref).bind(domain).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO collection_admission_decision (decision_ref,request_ref,outcome,reason_code,authorization_ref) VALUES ($1,$2,'admitted','fixture',$3)").bind(decision_ref).bind(request_ref).bind(authorization_ref).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO execution_station (station_ref,display_name,daily_work_quota) VALUES ($1,'复观测夹具工位',200)").bind(station_ref).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO plugin_installation (installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version,capabilities) VALUES ($1,$2,$3,'person',scope_001_now(),'0.8.57','[\"content_detail\",\"comments\",\"replies\",\"media_slots\",\"media_bytes\"]'::jsonb)").bind(installation_ref).bind(&install_key).bind(station_ref).execute(pool).await.unwrap();
    set_station_accepting(db, station_ref, true, "person")
        .await
        .unwrap();
    let credential = rotate_installation_credential(db, installation_ref)
        .await
        .unwrap();
    let raw = credential.raw_credential.expose_once().to_owned();
    activate_installation_credential(db, installation_ref, credential.credential_ref, &raw)
        .await
        .unwrap();
    let account = report_account_eligibility(
        db,
        installation_ref,
        &raw,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "xhs-account-evidence-fixture",
        },
        Some(b"evidence-fixture-digest-key-at-least-32"),
    )
    .await
    .unwrap();
    let account_ref = account.account_ref.unwrap();
    bind_observation_account(db, account_ref, installation_ref, "person")
        .await
        .unwrap();
    sqlx::query("UPDATE collection_admission_decision SET target_ref=$2,station_ref=$3,installation_ref=$4,account_ref=$5 WHERE decision_ref=$1").bind(decision_ref).bind(target_ref).bind(station_ref).bind(installation_ref).bind(account_ref).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,dispatch_lane,queue_state,scheduled_for,station_ref,installation_ref,account_ref) VALUES ($1,$2,$3,'deep_archive',20,'[\"maximum_quota\",\"time_budget\"]'::jsonb,'batch','queued',scope_001_now()+interval '1 day',$4,$5,$6)").bind(work_order_ref).bind(decision_ref).bind(target_ref).bind(station_ref).bind(installation_ref).bind(account_ref).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_domain_usage (work_order_ref,request_ref,domain_ref,role,basis_kind) VALUES ($1,$2,$3,'primary','admitted')").bind(work_order_ref).bind(request_ref).bind(domain).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_material_target (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media,allow_ocr,allow_asr) VALUES ($1,$2,1,30,2,false,false,false)").bind(work_order_ref).bind(content_public_ref).execute(pool).await.unwrap();
}

/// 研究底稿的「按当前模型预览完整请求」要有模型配置；连接指向本机回环端口，不会有任何外发调用。
async fn seed_model_config(db: &Database, domain: Uuid) {
    let (connection, version, model, config) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let pool = db.pool();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Synthetic preview only','openai-completions','http://127.0.0.1:1',true,$3)").bind(version).bind(connection).bind(Uuid::new_v4()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-preview','manual')").bind(model).bind(version).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,8192,1000,30,2)").bind(config).bind(model).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,config_ref,daily_token_limit) VALUES($1,'xhs',$2,100000)").bind(domain).bind(config).execute(pool).await.unwrap();
}
