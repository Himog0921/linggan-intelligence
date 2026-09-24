#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{proof_database, submit_package};
use linggan_contracts::EvidenceQuery;
use linggan_evidence::read_work_resources;
use serde_json::json;
use uuid::Uuid;

const ADHD_DOMAIN_REF: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0001);
const PEER_DOMAIN_REF: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0002);

#[tokio::test]
#[ignore = "requires the isolated LOCAL-001 PostgreSQL proof harness"]
async fn one_platform_identity_is_visible_in_multiple_domains_without_material_duplication() {
    let database = proof_database("domain_unification_shared_content").await;
    let external_id = "domain-parity-shared-content";
    submit_package(
        &database,
        "content_detail",
        json!({"contentExternalId":external_id}),
        json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
            "payload":{"title":"同一身份，不同研究领域","likes":3,"publicCommentCount":0,"collects":1,"shares":0}
        }),
    )
    .await;

    let (content_ref, package_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT public_ref,first_package_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(external_id)
    .fetch_one(database.pool())
    .await
    .expect("the accepted package creates one canonical Content identity");

    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
            (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
         VALUES($1,'xhs','creator','domain-parity-target','共享领域目标','manual','archived')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the synthetic target is valid");
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
         VALUES($1,$3,'primary'),($2,$3,'reference')",
    )
    .bind(ADHD_DOMAIN_REF)
    .bind(PEER_DOMAIN_REF)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the same Target can have an explicit role in each Domain");

    for domain_ref in [ADHD_DOMAIN_REF, PEER_DOMAIN_REF] {
        sqlx::query(
            "INSERT INTO linggan_material_domain_usage \
                (usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) \
             VALUES($1,$2,$3,$4,'legacy_domain_migration',$5)",
        )
        .bind(Uuid::new_v4())
        .bind(content_ref)
        .bind(domain_ref)
        .bind(if domain_ref == ADHD_DOMAIN_REF {
            "primary"
        } else {
            "reference"
        })
        .bind(package_ref)
        .execute(database.pool())
        .await
        .expect("Domain usage links the canonical identity without copying it");
    }

    let identity_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let usage_count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT domain_ref) FROM linggan_material_domain_usage \
         WHERE content_public_ref=$1",
    )
    .bind(content_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(identity_count, 1, "platform identity is canonical");
    assert_eq!(
        usage_count, 2,
        "each Domain keeps its own usage relationship"
    );

    let read_domain = |domain_ref: Uuid| {
        serde_json::from_value::<EvidenceQuery>(json!({
            "text":null,
            "scope":"all_accepted_material",
            "window":"latest_accepted_discovery",
            "sort":"latest_discovery",
            "domainRef":domain_ref
        }))
        .expect("Domain-scoped Evidence query is valid")
    };
    for domain_ref in [ADHD_DOMAIN_REF, PEER_DOMAIN_REF] {
        let page = read_work_resources(&database, &read_domain(domain_ref))
            .await
            .expect("the canonical material reader is Domain-scoped");
        assert!(
            page.items
                .iter()
                .any(|item| item.identity.public_ref == content_ref),
            "the same Content identity is visible through Domain {domain_ref}"
        );
    }
}
