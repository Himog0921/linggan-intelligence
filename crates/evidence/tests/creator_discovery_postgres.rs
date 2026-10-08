#[path = "support/material_fixture.rs"]
mod fixture;
use linggan_contracts::{
    EvidenceQuery,
    creator_discovery::{CreatorScope, creator_key},
};
use linggan_evidence::{
    creator_discovery::{aggregate, load},
    read_work_resources, register_discovered_creator, work_resource_schema_is_ready,
};
use serde_json::json;
use uuid::Uuid;
const D: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
const OTHER: Uuid = Uuid::from_u128(0x00000000000040008000000000000002);

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn handbook_fixture_and_same_work_counterexamples() {
    let db = fixture::proof_database("creator_discovery_fixture").await;
    let fixture = [
        (1, 1600, "related", false),
        (1, 50, "related", false),
        (2, 2000, "related", true),
        (2, 30, "related", true),
        (3, 40, "related", false),
        (3, 9000, "unrelated", true),
        (4, 1500, "related", true),
        (5, 1200, "related", false),
        (6, 8000, "unknown", false),
        (7, 100, "related", true),
        (8, 900, "related", true),
        (0, 6000, "related", false),
    ];
    let mut refs = Vec::new();
    for (i, (author, likes, _, _)) in fixture.iter().enumerate() {
        let id = format!("discovery-w{}", i + 1);
        let mut payload = json!({"title":format!("W{} 实践记录",i+1),"bodyText":format!("这是作品{}的具体实践过程",i+1),"likes":likes,"publicCommentCount":0,"collects":0,"shares":0,"authorName":if *author==2||*author==8{"同名作者".to_string()}else{format!("作者{author}")}});
        if *author != 0 {
            payload["authorId"] = json!(format!("A{author}"));
        }
        let package=fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":payload})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        refs.push(work);
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,like_threshold) VALUES($1,'xhs',1000)").bind(D).execute(db.pool()).await.unwrap();
    let mut q: CreatorScope = serde_json::from_value(json!({"domain":D})).unwrap();
    let data = load(&db, &q).await.unwrap();
    assert_eq!(data.works.len(), 12);
    let before_metric_fingerprint=data.works.iter().find(|w|w.work_ref==refs[10]).unwrap().fingerprint.clone();
    fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"discovery-w11"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"discovery-w11"},"payload":{"title":"W11 实践记录","bodyText":"这是作品11的具体实践过程","authorId":"A8","authorName":"同名作者","likes":901,"publicCommentCount":0,"collects":0,"shares":0}})).await;
    let data=load(&db,&q).await.unwrap();
    let after_metric=data.works.iter().find(|w|w.work_ref==refs[10]).unwrap();
    assert_eq!(after_metric.likes,Some(901));
    assert_eq!(after_metric.fingerprint,before_metric_fingerprint,"a like refresh with unchanged text must not queue new semantic analysis");
    for (i, (_, _, relevance, personal)) in fixture.iter().enumerate() {
        let w = data.works.iter().find(|w| w.work_ref == refs[i]).unwrap();
        let ids: Vec<_> = w.fragments.iter().map(|f| &f.fragment_id).take(1).collect();
        let result = json!({"relevance":{"value":relevance,"reason":"合成依据","evidenceFragmentIds":ids},"traits":{"personal_experience":{"value":if *personal{"yes"}else{"no"},"reason":"合成依据","evidenceFragmentIds":ids}},"topicHints":[]});
        sqlx::query("INSERT INTO linggan_creator_discovery_work_analysis(domain_ref,work_public_ref,requested_fingerprint,result_fingerprint,result_json,job_state) VALUES($1,$2,$3,$3,$4,'idle')").bind(D).bind(w.work_ref).bind(&w.fingerprint).bind(result).execute(db.pool()).await.unwrap();
    }
    for author in [1, 2] {
        let key=format!("A{author}");
        let supporting=data.works.iter().find(|w|w.work_ref==refs[if author==1 {0}else{2}]).unwrap().fragments[0].fragment_id.clone();
        sqlx::query("INSERT INTO linggan_creator_discovery_author_analysis(domain_ref,platform,author_external_id,requested_fingerprint,manual_overrides,job_state) VALUES($1,'xhs',$2,'manual',$3,'idle')").bind(D).bind(key).bind(json!({"focus":{"value":{"value":"vertical_tendency"},"supportFragmentIds":[supporting],"actor":"fixture","at":"2026-10-08"}})).execute(db.pool()).await.unwrap();
    }
    for (author, domain, state) in [(1, D, "archived"), (4, OTHER, "dismissed"), (7, D, "paused")] {
        let target = Uuid::new_v4();
        sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,source,lifecycle_state) VALUES($1,'xhs','creator',$2,'manual',$3)").bind(target).bind(format!("A{author}")).bind(state).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,$3)").bind(domain).bind(target).bind(if author==7 {"reference"}else{"primary"}).execute(db.pool()).await.unwrap();
    }
    let a1_target:Uuid=sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE platform='xhs' AND target_kind='creator' AND identity_key='A1'").fetch_one(db.pool()).await.unwrap();
    let source_package:Uuid=sqlx::query_scalar("SELECT package_ref FROM linggan_material_domain_usage WHERE content_public_ref=$1 AND domain_ref=$2").bind(refs[0]).bind(D).fetch_one(db.pool()).await.unwrap();
    for role in ["primary","reference"] {
        let request=Uuid::new_v4();
        sqlx::query("INSERT INTO collection_acquisition_request(request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) VALUES($1,$2,$3,$4,'deep_archive','duplicate usage fixture','person')").bind(request).bind(a1_target).bind(D).bind(role).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,request_ref,package_ref) VALUES($1,$2,$3,$4,'admission_reuse',$5,$6)").bind(Uuid::new_v4()).bind(refs[0]).bind(D).bind(role).bind(request).bind(source_package).execute(db.pool()).await.unwrap();
    }
    let data = load(&db, &q).await.unwrap();
    let a = aggregate(&data, &q);
    assert_eq!(data.works.len(),12,"repeated primary usage cannot duplicate a canonical work");
    let mut reference_scope=q.base();
    reference_scope.usage_role="reference".into();
    let reference=aggregate(&load(&db,&reference_scope).await.unwrap(),&reference_scope);
    assert_eq!(reference["stats"]["works"],1);
    assert_eq!(reference["stats"]["authors"],1);
    assert_eq!(a["stats"]["authors"], 8);
    assert_eq!(a["stats"]["works"], 12);
    assert_eq!(a["stats"]["unknownAuthorWorks"], 1);
    assert_eq!(a["stats"]["relatedAuthors"], 7);
    assert_eq!(a["stats"]["vertical"], 2);
    assert_eq!(a["stats"]["personal"], 4);
    assert_eq!(a["stats"]["outside"], 6);
    assert_eq!(a["stats"]["outsideViral"], 3);
    assert_eq!(a["stats"]["unknownDateWorks"], 12);
    assert_ne!(a["items"][0]["creatorKey"], a["items"][1]["creatorKey"]);
    let same_name = a["items"].as_array().unwrap().iter().filter(|item| item["displayName"] == "同名作者").count();
    assert_eq!(same_name, 2, "same display name cannot merge stable author IDs");
    let explicit: EvidenceQuery = serde_json::from_value(json!({"domainRef":D,"publicRefs":[refs[2],refs[0],refs[2]],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    let hydrated = read_work_resources(&db, &explicit).await.unwrap();
    assert_eq!(hydrated.items.len(), 2, "duplicate public refs hydrate one canonical work");
    assert!(hydrated.items.iter().any(|item| item.identity.public_ref == refs[2]));
    let unrestricted:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"publicRefs":[refs[2]],"restriction":"UNRESTRICTED","scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    assert!(read_work_resources(&db,&unrestricted).await.unwrap().items.is_empty(),"each explicit ref must pass the current restriction filter");
    let foreign: EvidenceQuery = serde_json::from_value(json!({"domainRef":OTHER,"publicRefs":[refs[2]],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    assert!(read_work_resources(&db, &foreign).await.unwrap().items.is_empty(), "explicit refs must retain the domain gate");
    for extra in [json!({"sort":"relevance"}),json!({"window":"last_30_days"}),json!({"text":"实践"})] {
        let mut value=json!({"domainRef":D,"publicRefs":[refs[2]],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"});
        value.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let unsupported:EvidenceQuery=serde_json::from_value(value).unwrap();
        assert!(read_work_resources(&db,&unsupported).await.is_err(),"batch must not silently ignore filters or sort");
    }
    let too_many:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"publicRefs":vec![refs[0];101],"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    assert!(read_work_resources(&db,&too_many).await.is_err());
    q.observation = Some("outside".into());
    q.viral = true;
    q.traits = Some("personal_experience".into());
    let matched = aggregate(&data, &q);
    assert_eq!(matched["total"], 2);
    q.creator_key = Some(creator_key("xhs", "A2"));
    let query:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"creatorScope":q,"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    let back = read_work_resources(&db, &query).await.unwrap();
    assert_eq!(back.items.len(), 1);
    assert_eq!(back.items[0].identity.public_ref, refs[2]);
    let mut author_search=q.base();
    author_search.query=Some("同名作者".into());
    let search_query:EvidenceQuery=serde_json::from_value(json!({"domainRef":D,"creatorScope":author_search,"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"})).unwrap();
    let search_back=read_work_resources(&db,&search_query).await.unwrap();
    assert_eq!(search_back.items.len(),3,"author search backlink must include only the two stable authors sharing that name");
    assert!(search_back.items.iter().all(|item|[refs[2],refs[3],refs[10]].contains(&item.identity.public_ref)));
    let key = creator_key("xhs", "A2");
    let (one, two) = tokio::join!(
        register_discovered_creator(&db, D, &key,"primary"),
        register_discovered_creator(&db, D, &key,"primary")
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one["targetRef"], two["targetRef"]);
    assert_eq!(one["collectionRequested"], false);
    let paused=register_discovered_creator(&db,D,&creator_key("xhs","A7"),"primary").await.unwrap();
    assert_eq!(paused["lifecycleState"],"paused");
    assert_eq!(paused["created"],false);
    assert_eq!(paused["relationshipAdded"],false);
    let paused_role:String=sqlx::query_scalar("SELECT role FROM observation_domain_target WHERE domain_ref=$1 AND target_ref=$2").bind(D).bind(paused["targetRef"].as_str().unwrap().parse::<Uuid>().unwrap()).fetch_one(db.pool()).await.unwrap();
    assert_eq!(paused_role,"reference","idempotent registration must preserve an existing role");
    q = q.base();
    let after = aggregate(&load(&db, &q).await.unwrap(), &q);
    assert_eq!(after["stats"]["outside"], 5);
    assert_eq!(after["stats"]["outsideViral"], 2);
    assert_eq!(after["stats"]["authors"], 8);
    sqlx::query("UPDATE observation_domain SET status='paused' WHERE domain_ref=$1").bind(OTHER).execute(db.pool()).await.unwrap();
    let a4_before=load(&db,&q).await.unwrap();
    let a4=a4_before.works.iter().find(|w|w.author_external_id.as_deref()==Some("A4")).unwrap();
    assert_eq!(a4.observation["hasOtherPrimaryDomain"],true);
    assert_eq!(a4.observation["lifecycleState"],"dismissed");
    let a4_key=creator_key("xhs","A4");
    let conflict=register_discovered_creator(&db,D,&a4_key,"primary").await.unwrap_err();
    assert_eq!(conflict.to_string(),"active_primary_domain_conflict","a paused other domain still owns the primary relation");
    let reference_receipt=register_discovered_creator(&db,D,&a4_key,"reference").await.unwrap();
    assert_eq!(reference_receipt["usageRole"],"reference");
    assert_eq!(reference_receipt["lifecycleState"],"dismissed");
    assert_eq!(reference_receipt["monitoringEnabled"],false);
    let repeated=register_discovered_creator(&db,D,&a4_key,"primary").await.unwrap();
    assert_eq!(repeated["usageRole"],"reference","replay cannot silently promote an existing relation");
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(jobs, 0);
    sqlx::query(
        "UPDATE linggan_creator_discovery_policy SET like_threshold=NULL WHERE domain_ref=$1",
    )
    .bind(D)
    .execute(db.pool())
    .await
    .unwrap();
    let no_rule = load(&db, &q).await.unwrap();
    assert!(aggregate(&no_rule, &q)["stats"]["outsideViral"].is_null());
    q.high_likes = true;
    q.observation = Some("outside".into());
    let high = aggregate(&no_rule, &q);
    assert_eq!(high["items"][0]["authorExternalId"], "A6");
    assert_eq!(high["items"][0]["candidateHighLikeCount"], 8000);
    // Exact deep links must never silently turn an invalid key into an all-library response.
    q.creator_key = Some("xhs:bad!".into());
    assert!(load(&db, &q).await.is_err());
    sqlx::query("ALTER TABLE collection_observation_target RENAME TO collection_observation_target_unavailable")
        .execute(db.pool()).await.unwrap();
    let partial_scope=q.base();
    let partial=load(&db,&partial_scope).await.unwrap();
    let partial_rows=aggregate(&partial,&partial_scope);
    assert_eq!(partial_rows["observationLookupState"],"unavailable");
    assert!(partial_rows["stats"]["outside"].is_null());
    assert!(partial_rows["stats"]["outsideViral"].is_null());
    assert!(partial_rows["items"][0]["observation"]["inCurrentDomain"].is_null());
    let mut unavailable_filter=partial_scope.clone();
    unavailable_filter.observation=Some("outside".into());
    assert!(load(&db,&unavailable_filter).await.is_err(),"unavailable observation filter cannot produce a false empty result");
    sqlx::query("ALTER TABLE collection_observation_target_unavailable RENAME TO collection_observation_target")
        .execute(db.pool()).await.unwrap();
    sqlx::query("ALTER TABLE collection_monitor_rule_revision RENAME TO collection_monitor_rule_revision_unavailable")
        .execute(db.pool()).await.unwrap();
    let target_count_before:i64=sqlx::query_scalar("SELECT count(*) FROM collection_observation_target").fetch_one(db.pool()).await.unwrap();
    assert!(register_discovered_creator(&db,D,&creator_key("xhs","A5"),"primary").await.is_err());
    let target_count_after:i64=sqlx::query_scalar("SELECT count(*) FROM collection_observation_target").fetch_one(db.pool()).await.unwrap();
    assert_eq!(target_count_before,target_count_after,"unknown observation state must block registration");
    sqlx::query("ALTER TABLE collection_monitor_rule_revision_unavailable RENAME TO collection_monitor_rule_revision")
        .execute(db.pool()).await.unwrap();
    for (id,author,likes,date,epoch) in [("discovery-w1","A1",1600,"2026-01-10T00:00:00Z",1768003200000_i64),("discovery-w2","A1",50,"2026-08-25T00:00:00Z",1787616000000_i64)] {
        fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":format!("{id} 时间反例"),"bodyText":"亲历时间反例","authorId":author,"likes":likes,"publishedAt":epoch,"publishedAtText":date,"publishedAtSourceField":"publishTime","publishedAtSourceKind":"platform_epoch"}})).await;
    }
    sqlx::query("UPDATE linggan_creator_discovery_policy SET like_threshold=1000 WHERE domain_ref=$1").bind(D).execute(db.pool()).await.unwrap();
    let mut recent=q.base();
    recent.published_from=Some("2026-08-24".into());
    recent.published_to=Some("2026-08-27".into());
    recent.viral=true;
    recent.creator_key=Some(creator_key("xhs","A1"));
    let recent_data=load(&db,&recent).await.unwrap();
    assert_eq!(aggregate(&recent_data,&recent)["total"],0,"recent low-like work cannot borrow an older viral work");
    let changed=load(&db,&q.base()).await.unwrap();
    let a1=changed.works.iter().find(|w|w.author_external_id.as_deref()==Some("A1")).unwrap();
    assert_eq!(a1.author_analysis["manual"]["focus"]["value"]["value"],"unknown","changed source text invalidates the old positive support without altering the stored row");
    assert_eq!(a1.author_analysis["manual"]["focus"]["supportUnavailable"],true);
    let stored:serde_json::Value=sqlx::query_scalar("SELECT manual_overrides FROM linggan_creator_discovery_author_analysis WHERE domain_ref=$1 AND author_external_id='A1'").bind(D).fetch_one(db.pool()).await.unwrap();
    assert_eq!(stored["focus"]["value"]["value"],"vertical_tendency","the manual record remains available for review");
    println!(
        "CREATOR-DISCOVERY fixture: 8 authors / 12 works / 1 unknown identity; 2 same-work personal viral; concurrent registration unique; no WorkOrder; null policy verified"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn profile_only_author_uses_the_shared_as_of_owner() {
    let db=fixture::proof_database("creator_discovery_profile_owner").await;
    let id="creator-profile-only-work";
    let package=fixture::submit_package(&db,"profile_discovery",json!({"authorExternalId":"ProfileAuthor"}),json!({"kind":"profile_discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"主页发现的作品"}})).await;
    let work:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1").bind(id).fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    let q:CreatorScope=serde_json::from_value(json!({"domain":D})).unwrap();
    let data=load(&db,&q).await.unwrap();
    let current=data.works.iter().find(|w|w.work_ref==work).unwrap();
    assert_eq!(current.author_external_id.as_deref(),Some("ProfileAuthor"));
    assert_eq!(current.acquisition_kind,"limited_domain_sample", "a profile package without a domain WorkOrder cannot claim broad acquisition");
    let view:(String,String)=sqlx::query_as("SELECT author_external_id,attribution_source FROM linggan_material_content_author WHERE content_public_ref=$1").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(view,("ProfileAuthor".into(),"profile_discovery".into()));
    let direct:(String,String)=sqlx::query_as("SELECT author_external_id,attribution_source FROM linggan_material_content_author_at($1,scope_001_now())").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(direct,view);
    let too_early:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_author_at($1,'2026-01-01'::timestamptz)").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(too_early,0,"the shared owner must not expose future accepted packages in an older read");
    assert!(work_resource_schema_is_ready(&db).await.unwrap());
    sqlx::query("DELETE FROM linggan_local_schema_migration WHERE migration_id='0115_creator_discovery'").execute(db.pool()).await.unwrap();
    assert!(!work_resource_schema_is_ready(&db).await.unwrap(),"shared Work Resource must report unready without its author owner migration");
    sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES('0115_creator_discovery','b935e46868c90819235981749a680ff1a5ae9eb0d5ba69cb9d51e5521c4aed7c')").execute(db.pool()).await.unwrap();
    let detail=fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"详情作者","authorId":"DetailAuthor","authorName":"详情作者"}})).await;
    assert_ne!(detail,package);
    let updated=load(&db,&q).await.unwrap();
    assert_eq!(updated.works.iter().find(|w|w.work_ref==work).unwrap().author_external_id.as_deref(),Some("DetailAuthor"));
    let view:(String,String)=sqlx::query_as("SELECT author_external_id,attribution_source FROM linggan_material_content_author WHERE content_public_ref=$1").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(view,("DetailAuthor".into(),"content_detail".into()));
}
