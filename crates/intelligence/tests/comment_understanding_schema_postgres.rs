//! Isolated schema proof for Comment Understanding V2 (migration 0230): research unit
//! integrity, merge redirects and the derived Problem state.
//!
//! Rows are synthetic. Foreign keys and append-only triggers are bypassed with
//! `session_replication_role = replica` inside one transaction that is always rolled back; only
//! the validation triggers under test are switched back on with `ENABLE ALWAYS`.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier_fixture;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

type Tx = Transaction<'static, Postgres>;

async fn begin(schema: &str) -> Tx {
    let db = dossier_fixture::fixture::proof_database(schema).await;
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::raw_sql(
        "SET LOCAL session_replication_role = replica;
         ALTER TABLE linggan_comment_research_unit ENABLE ALWAYS TRIGGER linggan_comment_research_unit_validate;
         ALTER TABLE linggan_comment_research_unit_target ENABLE ALWAYS TRIGGER linggan_comment_research_unit_target_validate;
         ALTER TABLE linggan_comment_problem_redirect ENABLE ALWAYS TRIGGER linggan_comment_problem_redirect_validate;",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    tx
}

async fn content(tx: &mut Tx, platform: &str, external: &str) -> Uuid {
    let work = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_material_content(platform,content_external_id,public_ref,first_package_ref) VALUES($1,$2,$3,$4)")
        .bind(platform).bind(external).bind(work).bind(Uuid::new_v4())
        .execute(&mut **tx).await.unwrap();
    work
}

async fn comment(tx: &mut Tx, work: Uuid, id: &str, author: Option<&str>) -> Uuid {
    let material = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_material_comment(material_ref,content_public_ref,package_ref,record_ordinal,comment_external_id,root_comment_external_id,is_reply,body_text,body_state,author_external_id,observed_at) VALUES($1,$2,$3,0,$4,$4,false,'合成评论','KNOWN',$5,'2026-11-01T00:00:00Z')")
        .bind(material).bind(work).bind(Uuid::new_v4()).bind(id).bind(author)
        .execute(&mut **tx).await.unwrap();
    material
}

/// A research request with a stored result (`ok`) or a failure, created at `at`.
async fn request(
    tx: &mut Tx,
    domain: Uuid,
    work: Uuid,
    source: Uuid,
    method: &str,
    at: &str,
    ok: bool,
) -> Uuid {
    let request = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version,created_at) VALUES($1,gen_random_uuid(),$2,$3,$4,'primary',$5::timestamptz,repeat('a',64),repeat('b',64),gen_random_uuid(),gen_random_uuid(),$6,$5::timestamptz)")
        .bind(request).bind(domain).bind(work).bind(source).bind(at).bind(method)
        .execute(&mut **tx).await.unwrap();
    let (result, failure): (Option<Value>, Option<&str>) = if ok {
        (Some(json!({"outcome":"signals","signals":[]})), None)
    } else {
        (None, Some("target_invalid"))
    };
    sqlx::query("INSERT INTO linggan_comment_research_result(request_ref,invocation_ref,result,failure_code,created_at) VALUES($1,gen_random_uuid(),$2,$3,$4::timestamptz)")
        .bind(request).bind(result).bind(failure).bind(at)
        .execute(&mut **tx).await.unwrap();
    request
}

async fn problem(tx: &mut Tx, domain: Uuid, label: &str) -> (Uuid, Uuid) {
    let (problem, definition) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO linggan_comment_problem_definition(definition_ref,problem_ref,domain_ref,label,goal,barrier,source_request_ref,source_signal_ordinal,origin_invocation_ref) VALUES($1,$2,$3,$4,'目标','障碍',gen_random_uuid(),0,gen_random_uuid())")
        .bind(definition).bind(problem).bind(domain).bind(label)
        .execute(&mut **tx).await.unwrap();
    (problem, definition)
}

async fn support(tx: &mut Tx, request: Uuid, ordinal: i32, definition: Uuid) {
    sqlx::query("INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,origin_invocation_ref) VALUES($1,$2,$3,gen_random_uuid())")
        .bind(request).bind(ordinal).bind(definition)
        .execute(&mut **tx).await.unwrap();
}

async fn restrict(tx: &mut Tx, work: Uuid, id: &str) {
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,$2,'synthetic')")
        .bind(work).bind(id).execute(&mut **tx).await.unwrap();
}

/// state, signals, comments, authors, works — or None when the Problem is not live.
async fn state(tx: &mut Tx, problem: Uuid) -> Option<(String, i64, i64, i64, i64)> {
    sqlx::query_as("SELECT state,signal_count,comment_count,author_count,work_count FROM linggan_comment_problem_current WHERE problem_ref=$1")
        .bind(problem).fetch_optional(&mut **tx).await.unwrap()
}

fn expect(actual: Option<(String, i64, i64, i64, i64)>, state: &str, counts: (i64, i64, i64, i64)) {
    assert_eq!(
        actual,
        Some((state.to_owned(), counts.0, counts.1, counts.2, counts.3)),
        "state, signals, comments, authors, works"
    );
}

async fn redirect(tx: &mut Tx, from: Uuid, to: Uuid) -> Result<Uuid, sqlx::Error> {
    let redirect = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_problem_redirect(redirect_ref,from_problem_ref,to_problem_ref,decision_invocation_ref) VALUES($1,$2,$3,gen_random_uuid())")
        .bind(redirect).bind(from).bind(to).execute(&mut **tx).await?;
    Ok(redirect)
}

async fn revoke(tx: &mut Tx, redirect: Uuid) {
    sqlx::query("INSERT INTO linggan_comment_problem_redirect_revocation(redirect_ref,command_ref) VALUES($1,gen_random_uuid())")
        .bind(redirect).execute(&mut **tx).await.unwrap();
}

/// Runs a write that the database must refuse, then restores the transaction. A macro, because
/// the attempt borrows the same transaction that the savepoints run on.
macro_rules! refused {
    ($tx:expr, $attempt:expr) => {{
        sqlx::query("SAVEPOINT attempt")
            .execute(&mut *$tx)
            .await
            .unwrap();
        let outcome = $attempt.await;
        sqlx::query("ROLLBACK TO SAVEPOINT attempt")
            .execute(&mut *$tx)
            .await
            .unwrap();
        match outcome {
            Ok(_) => panic!("the database accepted a write it must refuse"),
            Err(error) => error.to_string(),
        }
    }};
}

async fn resolves_to(tx: &mut Tx, problem: Uuid) -> Uuid {
    let rows: Vec<Uuid> = sqlx::query_scalar(
        "SELECT current_ref FROM linggan_comment_problem_resolution WHERE problem_ref=$1",
    )
    .bind(problem)
    .fetch_all(&mut **tx)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "every Problem resolves to exactly one live Problem"
    );
    rows[0]
}

#[tokio::test]
#[ignore = "isolated PostgreSQL only"]
async fn problem_state_is_derived_from_the_current_distinct_authors() {
    let mut tx = begin("comment_understanding_state").await;
    let domain = Uuid::new_v4();
    let (w1, w2) = (
        content(&mut tx, "xhs", "w1").await,
        content(&mut tx, "douyin", "w2").await,
    );
    let (p1, d1) = problem(&mut tx, domain, "孩子写作业启动困难").await;
    let (p2, d2) = problem(&mut tx, domain, "孩子写作业耗时过长").await;
    let v2 = "comment-understanding.v1";

    // Two comments by one author support the Problem twice but add one author.
    let (a1, a2) = (
        comment(&mut tx, w1, "a1", Some("A")).await,
        comment(&mut tx, w1, "a2", Some("A")).await,
    );
    let r_a1 = request(&mut tx, domain, w1, a1, v2, "2026-11-01T00:00:00Z", true).await;
    let r_a2 = request(&mut tx, domain, w1, a2, v2, "2026-11-01T00:00:00Z", true).await;
    support(&mut tx, r_a1, 0, d1).await;
    support(&mut tx, r_a2, 0, d1).await;
    expect(state(&mut tx, p1).await, "candidate", (2, 2, 1, 1));

    // A second distinguishable author makes it formal, with the same Problem identity.
    let b1 = comment(&mut tx, w1, "b1", Some("B")).await;
    let r_b1 = request(&mut tx, domain, w1, b1, v2, "2026-11-01T00:00:00Z", true).await;
    support(&mut tx, r_b1, 0, d1).await;
    expect(state(&mut tx, p1).await, "formal", (3, 3, 2, 1));

    // An unknown author supports the Problem but never adds an author.
    let n1 = comment(&mut tx, w1, "n1", None).await;
    let r_n1 = request(&mut tx, domain, w1, n1, v2, "2026-11-01T00:00:00Z", true).await;
    support(&mut tx, r_n1, 0, d2).await;
    expect(state(&mut tx, p2).await, "candidate", (1, 1, 0, 1));

    // A restricted comment stops counting at once.
    let r1 = comment(&mut tx, w1, "r1", Some("R")).await;
    let r_r1 = request(&mut tx, domain, w1, r1, v2, "2026-11-01T00:00:00Z", true).await;
    support(&mut tx, r_r1, 0, d1).await;
    expect(state(&mut tx, p1).await, "formal", (4, 4, 3, 1));
    restrict(&mut tx, w1, "r1").await;
    expect(state(&mut tx, p1).await, "formal", (3, 3, 2, 1));

    // The same author id on another platform is another author.
    let x1 = comment(&mut tx, w2, "x1", Some("A")).await;
    let r_x1 = request(&mut tx, domain, w2, x1, v2, "2026-11-01T00:00:00Z", true).await;
    support(&mut tx, r_x1, 0, d1).await;
    expect(state(&mut tx, p1).await, "formal", (4, 4, 3, 2));

    // Researching b1 again replaces its support: it leaves P1 and joins P2.
    let r_b1_again = request(&mut tx, domain, w1, b1, v2, "2026-11-02T00:00:00Z", true).await;
    support(&mut tx, r_b1_again, 0, d2).await;
    expect(state(&mut tx, p1).await, "formal", (3, 3, 2, 2));
    expect(state(&mut tx, p2).await, "candidate", (2, 2, 1, 1));

    // Losing the second author takes the Problem back to candidate; the identity is unchanged.
    restrict(&mut tx, w2, "x1").await;
    expect(state(&mut tx, p1).await, "candidate", (2, 2, 1, 1));

    // A newer failed request keeps the older successful research; V1 research is not counted.
    request(&mut tx, domain, w1, a1, v2, "2026-11-03T00:00:00Z", false).await;
    expect(state(&mut tx, p1).await, "candidate", (2, 2, 1, 1));
    let c1 = comment(&mut tx, w1, "c1", Some("C")).await;
    let r_c1 = request(
        &mut tx,
        domain,
        w1,
        c1,
        "comment-research-semantic.v1",
        "2026-11-03T00:00:00Z",
        true,
    )
    .await;
    support(&mut tx, r_c1, 0, d2).await;
    expect(state(&mut tx, p2).await, "candidate", (2, 2, 1, 1));

    // A merge resolves support through the redirect and keeps both definitions as history.
    let merge = redirect(&mut tx, p2, p1).await.unwrap();
    assert!(state(&mut tx, p2).await.is_none());
    expect(state(&mut tx, p1).await, "formal", (4, 4, 2, 1));
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_problem_support WHERE problem_ref=$1",
    )
    .bind(p1)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(rows, 4);
    revoke(&mut tx, merge).await;
    expect(state(&mut tx, p2).await, "candidate", (2, 2, 1, 1));
    expect(state(&mut tx, p1).await, "candidate", (2, 2, 1, 1));
    tx.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL only"]
async fn redirects_join_live_problems_of_one_domain_and_never_form_a_cycle() {
    let mut tx = begin("comment_understanding_redirect").await;
    let (domain, other) = (Uuid::new_v4(), Uuid::new_v4());
    let (p1, _) = problem(&mut tx, domain, "P1").await;
    let (p2, _) = problem(&mut tx, domain, "P2").await;
    let (p3, _) = problem(&mut tx, domain, "P3").await;
    let (q1, _) = problem(&mut tx, other, "Q1").await;

    let message = refused!(tx, redirect(&mut tx, p1, q1));
    assert!(message.contains("inside one domain"), "{message}");
    refused!(tx, redirect(&mut tx, p1, p1));

    redirect(&mut tx, p1, p2).await.unwrap();
    // A redirected Problem cannot be redirected again, nor be a target.
    let message = refused!(tx, redirect(&mut tx, p1, p3));
    assert!(message.contains("not already redirected"), "{message}");
    refused!(tx, redirect(&mut tx, p3, p1));

    // Chains resolve to the live Problem at the end.
    let second = redirect(&mut tx, p2, p3).await.unwrap();
    for problem in [p1, p2, p3] {
        assert_eq!(resolves_to(&mut tx, problem).await, p3);
    }
    // Closing the loop is refused: p1 already has an outgoing redirect.
    refused!(tx, redirect(&mut tx, p3, p1));

    // Revoking an edge makes both ends live again, and a different shape is allowed.
    revoke(&mut tx, second).await;
    assert_eq!(resolves_to(&mut tx, p1).await, p2);
    assert_eq!(resolves_to(&mut tx, p2).await, p2);
    redirect(&mut tx, p3, p2).await.unwrap();
    // The two-cycle p2 -> p3 -> p2 is refused.
    refused!(tx, redirect(&mut tx, p2, p3));
    assert_eq!(resolves_to(&mut tx, p3).await, p2);
    tx.rollback().await.unwrap();
}

async fn l1_request(tx: &mut Tx, domain: Uuid, work: Uuid) -> Uuid {
    let request = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_content_understanding_request(request_ref,command_ref,domain_ref,work_ref,material_snapshot_identity,method_version,knowledge_cutoff,input_hash,input_manifest,task_config_ref,batch_ref,item_ref) VALUES($1,gen_random_uuid(),$2,$3,repeat('c',64),'content-understanding.l1.v1',statement_timestamp(),repeat('d',64),'{}',gen_random_uuid(),gen_random_uuid(),gen_random_uuid())")
        .bind(request).bind(domain).bind(work).execute(&mut **tx).await.unwrap();
    request
}

async fn unit(
    tx: &mut Tx,
    domain: Uuid,
    work: Uuid,
    l1: Uuid,
    batch: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let (unit, item) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) VALUES($1,$2,0,$3,'{}')")
        .bind(item).bind(batch).bind(unit.to_string()).execute(&mut **tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_research_unit(unit_ref,item_ref,domain_ref,work_ref,l1_request_ref) VALUES($1,$2,$3,$4,$5)")
        .bind(unit).bind(item).bind(domain).bind(work).bind(l1).execute(&mut **tx).await?;
    Ok(unit)
}

async fn target(tx: &mut Tx, unit: Uuid, request: Uuid, position: i32) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO linggan_comment_research_unit_target(request_ref,unit_ref,position) VALUES($1,$2,$3)")
        .bind(request).bind(unit).bind(position).execute(&mut **tx).await.map(|_| ())
}

/// A V2 request that belongs to `batch`.
async fn batch_request(tx: &mut Tx, domain: Uuid, work: Uuid, source: Uuid, batch: Uuid) -> Uuid {
    let request = request(
        tx,
        domain,
        work,
        source,
        "comment-understanding.v1",
        "2026-11-01T00:00:00Z",
        true,
    )
    .await;
    sqlx::query("UPDATE linggan_comment_research_request SET batch_ref=$2 WHERE request_ref=$1")
        .bind(request)
        .bind(batch)
        .execute(&mut **tx)
        .await
        .unwrap();
    request
}

#[tokio::test]
#[ignore = "isolated PostgreSQL only"]
async fn a_research_unit_never_mixes_works_domains_or_batches() {
    let mut tx = begin("comment_understanding_unit").await;
    let (domain, other_domain) = (Uuid::new_v4(), Uuid::new_v4());
    let (w1, w2) = (
        content(&mut tx, "xhs", "w1").await,
        content(&mut tx, "xhs", "w2").await,
    );
    let (l1_w1, l1_w2) = (
        l1_request(&mut tx, domain, w1).await,
        l1_request(&mut tx, domain, w2).await,
    );
    let l1_other_domain = l1_request(&mut tx, other_domain, w1).await;
    let batch = Uuid::new_v4();

    // The L1 context must belong to the unit's work and domain.
    let message = refused!(tx, unit(&mut tx, domain, w1, l1_w2, batch));
    assert!(message.contains("L1 context must belong"), "{message}");
    refused!(tx, unit(&mut tx, domain, w1, l1_other_domain, batch));
    let unit_ref = unit(&mut tx, domain, w1, l1_w1, batch).await.unwrap();

    // Targets are V2 requests of the unit work, domain and batch.
    let (c1, c2, c3) = (
        comment(&mut tx, w1, "c1", Some("A")).await,
        comment(&mut tx, w2, "c2", Some("B")).await,
        comment(&mut tx, w1, "c3", Some("C")).await,
    );
    let own = batch_request(&mut tx, domain, w1, c1, batch).await;
    target(&mut tx, unit_ref, own, 1).await.unwrap();
    let foreign_work = batch_request(&mut tx, domain, w2, c2, batch).await;
    let message = refused!(tx, target(&mut tx, unit_ref, foreign_work, 2));
    assert!(message.contains("unit work, domain and batch"), "{message}");
    let foreign_batch = batch_request(&mut tx, domain, w1, c3, Uuid::new_v4()).await;
    refused!(tx, target(&mut tx, unit_ref, foreign_batch, 2));
    let foreign_domain = batch_request(&mut tx, other_domain, w1, c3, batch).await;
    refused!(tx, target(&mut tx, unit_ref, foreign_domain, 2));
    let v1 = request(
        &mut tx,
        domain,
        w1,
        c3,
        "comment-research-semantic.v1",
        "2026-11-01T00:00:00Z",
        true,
    )
    .await;
    sqlx::query("UPDATE linggan_comment_research_request SET batch_ref=$2 WHERE request_ref=$1")
        .bind(v1)
        .bind(batch)
        .execute(&mut *tx)
        .await
        .unwrap();
    refused!(tx, target(&mut tx, unit_ref, v1, 2));

    // A request belongs to one unit, and a position to one request.
    refused!(tx, target(&mut tx, unit_ref, own, 2));
    let second = batch_request(&mut tx, domain, w1, c3, batch).await;
    refused!(tx, target(&mut tx, unit_ref, second, 1));
    target(&mut tx, unit_ref, second, 2).await.unwrap();
    tx.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL only"]
async fn one_batch_command_and_invocation_may_serve_many_requests() {
    let mut tx = begin("comment_understanding_shared").await;
    let domain = Uuid::new_v4();
    let work = content(&mut tx, "xhs", "w1").await;
    let (batch, command, invocation) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    for id in ["c1", "c2"] {
        let source = comment(&mut tx, work, id, Some(id)).await;
        let request = request(
            &mut tx,
            domain,
            work,
            source,
            "comment-understanding.v1",
            "2026-11-01T00:00:00Z",
            true,
        )
        .await;
        sqlx::query("UPDATE linggan_comment_research_request SET batch_ref=$2,command_ref=$3 WHERE request_ref=$1")
            .bind(request).bind(batch).bind(command).execute(&mut *tx).await.unwrap();
        sqlx::query(
            "UPDATE linggan_comment_research_result SET invocation_ref=$2 WHERE request_ref=$1",
        )
        .bind(request)
        .bind(invocation)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    let served: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_request WHERE batch_ref=$1 AND command_ref=$2")
        .bind(batch).bind(command).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(served, 2);
    tx.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL only"]
async fn the_new_task_is_registered_disabled_and_unconfigured() {
    let mut tx = begin("comment_understanding_task").await;
    let row: (String, String, bool, String, String) = sqlx::query_as("SELECT t.lifecycle,t.handler_contract,c.enabled,cfg.budget_mode,cfg.execution_contract FROM linggan_ai_task t JOIN linggan_ai_control c ON c.task_key=t.task_key JOIN linggan_ai_task_setting s ON s.task_key=t.task_key JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=s.current_config_ref WHERE t.task_key='comment_understanding'")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(
        row,
        (
            "active".into(),
            "comment-understanding.v1".into(),
            false,
            "unconfigured".into(),
            "comment-understanding.v1".into()
        )
    );
    let codes: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ai_failure_code WHERE code IN('unit_output_invalid','unit_truncated','target_missing','target_invalid','target_all_signals_invalid')")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(codes, 5);
    // The batch and egress constraints know the task, as interactive comment research.
    for (constraint, fragment) in [
        ("ai_batch_task_kind_valid", "comment_understanding"),
        ("ai_egress_material_task_valid", "comment_understanding"),
    ] {
        let definition: String = sqlx::query_scalar(
            "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname=$1",
        )
        .bind(constraint)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert!(definition.contains(fragment), "{constraint}: {definition}");
    }
    tx.rollback().await.unwrap();
}
