//! Snapshot reads and context inspection; no writes or model calls.
use super::observations::add_observations;
use super::query_sql::{READ, SCOPED};
use super::*;
pub async fn read(db: &Database, q: &ResearchScope) -> Result<Value, ModelError> {
    // Daily displays frozen batch membership, independent of the voices date filter.
    let mut daily_scope = q.clone();
    if q.view.as_deref() == Some("daily") {
        daily_scope.from = Some("1970-01-01T00:00:00Z".into());
        daily_scope.to = Some("9999-01-01T00:00:00Z".into());
    }
    execute_scoped(db, &daily_scope, false, None).await
}
/// Explicit diagnostic seam; never exposed by the product HTTP API.
pub async fn explain_scope(db: &Database, q: &ResearchScope) -> Result<Value, ModelError> {
    execute_scoped(db, q, true, None).await
}
async fn execute_scoped(
    db: &Database,
    q: &ResearchScope,
    explain: bool,
    detail_ref: Option<Uuid>,
) -> Result<Value, ModelError> {
    q.validate()?;
    let dates_valid:bool=sqlx::query_scalar("SELECT ($1::text IS NULL OR pg_input_is_valid($1,'timestamp with time zone')) AND ($2::text IS NULL OR pg_input_is_valid($2,'timestamp with time zone'))").bind(&q.from).bind(&q.to).fetch_one(db.pool()).await?;
    if !dates_valid {
        return Err(ModelError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    // Transaction-local per-sort/aggregate memory limit; no server-wide tuning.
    sqlx::query("SET LOCAL work_mem=\'64MB\'")
        .execute(&mut *tx)
        .await?;
    // Scope selectivity varies widely (one work vs. whole domain); do not reuse a generic plan.
    sqlx::query("SET LOCAL plan_cache_mode=force_custom_plan")
        .execute(&mut *tx)
        .await?;
    let sql = if explain {
        format!("EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) {SCOPED}{READ}")
    } else {
        format!("{SCOPED}{READ}")
    };
    let lens: Vec<&str> = q
        .lenses
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect();
    let mut value: Value = sqlx::query_scalar(AssertSqlSafe(sql))
        .bind(q.domain)
        .bind(&q.from)
        .bind(&q.to)
        .bind(q.days.unwrap_or(30))
        .bind(q.time_basis.as_deref().unwrap_or("observed"))
        .bind(q.text.as_deref().unwrap_or(""))
        .bind(q.work_ref)
        .bind(lens)
        .bind(q.problem_ref)
        .bind(q.term.as_deref().unwrap_or(""))
        .bind(q.bookmarked_only.unwrap_or(false))
        .bind(q.sort.as_deref().unwrap_or("observed"))
        .bind(q.offset.unwrap_or(0))
        .bind(q.limit.unwrap_or(20))
        .bind(q.result_revision.as_deref().unwrap_or(""))
        .bind(q.batch_ref)
        .bind(q.refs()?)
        .bind(q.processing_state.as_deref().unwrap_or(""))
        .bind(detail_ref)
        .bind(q.view.as_deref() == Some("voices") && detail_ref.is_none())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ModelError::Invalid)?;
    if explain {
        tx.commit().await?;
        return Ok(value);
    }
    enrich_titles(&mut tx, &mut value).await?;
    tx.commit().await?;
    add_observations(&mut value);
    value["model"] = crate::model_settings_read::current_comment_model_state(db).await?;
    let mut daily = if value["ownDomain"] == true && value["deferredAggregates"] != true {
        crate::comment_daily::overview(db).await?
    } else {
        json!({"schedule":{"enabled":false},"items":[],"timezone":"Asia/Shanghai"})
    };
    let selected = value["dailySelection"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(items) = daily["items"].as_array_mut() {
        items.retain_mut(|item| {
            let Some(scoped) = selected.iter().find(|s| s["batchRef"] == item["batchRef"]) else {
                return false;
            };
            item["batchTotals"] =
                json!({"total":item["total"],"works":item["works"],"counts":item["counts"]});
            for key in ["total", "works", "counts", "cleaning"] {
                item[key] = scoped[key].clone();
            }
            true
        });
    }
    value["daily"] = daily;
    if let Some(obj) = value.as_object_mut() {
        obj.remove("dailySelection");
        obj.remove("ownDomain");
    }
    Ok(value)
}
async fn enrich_titles(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    value: &mut Value,
) -> Result<(), ModelError> {
    // Cross-industry titles belong to their sample view, never to native work UUIDs.
    if value["ownDomain"] != true {
        return Ok(());
    }
    let mut refs = std::collections::BTreeSet::new();
    for items in [
        &value["works"],
        &value["page"]["items"],
        &value["distribution"],
        &value["representatives"],
    ] {
        for item in items.as_array().into_iter().flatten() {
            if let Some(id) = item["workRef"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                refs.insert(id);
            }
        }
    }
    let refs: Vec<_> = refs.into_iter().collect();
    let as_of = value["scope"]["asOf"]
        .as_str()
        .ok_or(ModelError::Invalid)?
        .to_owned();
    for chunk in refs.chunks(250) {
        let works =
            linggan_evidence::comment_research_read::read_comment_work_contexts_in_snapshot(
                tx, chunk, &as_of,
            )
            .await
            .map_err(ModelError::from)?;
        for v in works.as_array().into_iter().flatten() {
            for item in value["page"]["items"].as_array_mut().into_iter().flatten() {
                if item["workRef"] == v["workRef"] && item["workTitle"].is_null() {
                    item["workTitle"] = v["title"].clone();
                    item["creatorDisplayName"] = v["creatorDisplayName"].clone();
                }
            }
            for item in value["representatives"]
                .as_array_mut()
                .into_iter()
                .flatten()
            {
                if item["workRef"] == v["workRef"] {
                    item["creatorDisplayName"] = v["creatorDisplayName"].clone();
                }
            }
            for key in ["works", "distribution"] {
                for item in value[key].as_array_mut().into_iter().flatten() {
                    if item["workRef"] == v["workRef"] && item["title"].is_null() {
                        item["title"] = v["title"].clone();
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn source(db: &Database, domain: Uuid, reference: Uuid) -> Result<Value, ModelError> {
    source_with_scope(
        db,
        &ResearchScope {
            domain: Some(domain),
            from: Some("1970-01-01T00:00:00Z".into()),
            to: Some("9999-01-01T00:00:00Z".into()),
            ..Default::default()
        },
        reference,
    )
    .await
}
pub async fn source_with_scope(
    db: &Database,
    q: &ResearchScope,
    reference: Uuid,
) -> Result<Value, ModelError> {
    q.validate()?;
    let domain = q.domain.ok_or(ModelError::Invalid)?;
    let row=sqlx::query("SELECT source_ref,canonical_ref,work_ref,to_jsonb(s) AS raw FROM linggan_ci_source s WHERE domain_ref=$1 AND(source_ref=$2 OR canonical_ref=$2 OR EXISTS(SELECT 1 FROM linggan_material_comment old WHERE old.material_ref=$2 AND old.content_public_ref=s.work_ref AND old.comment_external_id=s.comment_external_id AND EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=s.domain_ref AND d.is_own_domain)))").bind(domain).bind(reference).fetch_optional(db.pool()).await?.ok_or(ModelError::Source)?;
    let actual: Uuid = row.get("source_ref");
    let work: Uuid = row.get("work_ref");
    let raw: Value = row.get("raw");
    let own: bool =
        sqlx::query_scalar("SELECT is_own_domain FROM observation_domain WHERE domain_ref=$1")
            .bind(domain)
            .fetch_one(db.pool())
            .await?;
    let mut value = if own {
        linggan_evidence::comment_research_read::read_comment_research_context(db, actual)
            .await
            .map_err(ModelError::from)?
    } else {
        let parent:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('sourceRef',source_ref,'body',body,'likes',likes) FROM linggan_ci_source WHERE domain_ref=$1 AND work_ref=$2 AND comment_external_id=$3").bind(domain).bind(work).bind(raw["parent_external_id"].as_str()).fetch_optional(db.pool()).await?;
        json!({"parent":parent,"parentState":if raw["is_reply"]==true{if parent.is_some(){"AVAILABLE"}else{"MISSING_OR_RESTRICTED"}}else{"NOT_APPLICABLE"},"work":{"title":raw["work_title"]},"derivatives":[]})
    };
    let research = crate::comment_intelligence_actions::source_research(db, domain, actual)
        .await
        .map_err(research_read_error)?;
    value["source"] = json!({"sourceRef":actual,"canonicalRef":row.get::<Uuid,_>("canonical_ref"),"workRef":work,"body":raw["body"],"bodyTruncated":false,"likes":raw["likes"],"isReply":raw["is_reply"],"publishedAt":raw["published_at"],"publishedAtText":raw["published_at_text"],"firstObservedAt":raw["first_observed_at"],"lastObservedAt":raw["last_observed_at"],"role":raw["role"],"workTitle":raw["work_title"],"creatorDisplayName":raw["creator_display_name"],"bookmarked":research["bookmarked"],"bookmarkRevision":research["revision"],"researchRevision":research["revision"],"labels":research["labels"]});
    value["research"] = research;
    value["sourceSha256"] = raw["source_sha256"].clone();
    value["annotations"] = if own {
        crate::comment_research_projection::read_comment_research_annotations(db, actual)
            .await
            .map_err(ModelError::from)?
    } else {
        json!({"human":[],"analysis":[]})
    };
    value["processing"] = if own {
        crate::comment_daily::source_states(db, &[actual])
            .await?
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let thread:Vec<Value>=sqlx::query_scalar("WITH RECURSIVE ancestors AS (SELECT s.*,ARRAY[s.comment_external_id] AS path,0 AS depth FROM linggan_ci_source s WHERE s.domain_ref=$1 AND s.work_ref=$2 AND s.source_ref=$3 UNION ALL SELECT p.*,a.path||p.comment_external_id,a.depth+1 FROM linggan_ci_source p JOIN ancestors a ON p.comment_external_id=a.parent_external_id WHERE p.domain_ref=$1 AND p.work_ref=$2 AND a.depth<32 AND NOT p.comment_external_id=ANY(a.path)), root AS (SELECT comment_external_id FROM ancestors ORDER BY depth DESC LIMIT 1), thread AS (SELECT s.*,ARRAY[s.comment_external_id] AS path,0 AS depth FROM linggan_ci_source s JOIN root USING(comment_external_id) WHERE s.domain_ref=$1 AND s.work_ref=$2 UNION ALL SELECT child.*,p.path||child.comment_external_id,p.depth+1 FROM linggan_ci_source child JOIN thread p ON child.parent_external_id=p.comment_external_id WHERE child.domain_ref=$1 AND child.work_ref=$2 AND p.depth<32 AND NOT child.comment_external_id=ANY(p.path)) SELECT jsonb_build_object('sourceRef',source_ref,'body',body,'likes',likes,'isReply',is_reply,'depth',depth) FROM thread ORDER BY first_observed_at,canonical_ref LIMIT 100").bind(domain).bind(work).bind(actual).fetch_all(db.pool()).await?;
    value["thread"] = json!(thread);
    let scope = ResearchScope {
        offset: Some(0),
        ..q.clone()
    };
    let effective = execute_scoped(db, &scope, false, Some(actual)).await?;
    if let Some(item) = effective["page"]["items"]
        .as_array()
        .and_then(|v| v.first())
    {
        for key in [
            "labels",
            "workTitle",
            "creatorDisplayName",
            "cleanState",
            "analysisState",
            "problemRefs",
        ] {
            value["source"][key] = item[key].clone();
        }
        value["research"]["labels"] = item["labels"].clone();
        if own
            && let Some(analysis) = item["analysisRef"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
        {
            let owner: Uuid = sqlx::query_scalar(
                "SELECT source_ref FROM linggan_comment_analysis_work WHERE work_ref=$1",
            )
            .bind(analysis)
            .fetch_one(db.pool())
            .await?;
            if owner != actual {
                let historical =
                    crate::comment_research_projection::read_comment_research_annotations(
                        db, owner,
                    )
                    .await
                    .map_err(ModelError::from)?;
                value["annotations"]["analysis"] = historical["analysis"].clone();
                value["annotations"]["analysisSourceRef"] = json!(owner);
            }
        }
    }
    let bookmarks: Vec<Value> = if own {
        sqlx::query_scalar("SELECT jsonb_build_object('assetRef',a.asset_ref,'sourceRef',a.source_ref,'quote',substring(m.body_text FROM a.start_char+1 FOR a.end_char-a.start_char),'reason',a.reason,'createdAt',a.created_at,'revision',a.revision,'collectionRef',a.collection_ref) FROM linggan_comment_asset_current a JOIN linggan_comment_research_readable m ON m.material_ref=a.source_ref WHERE m.content_public_ref=$1 AND m.comment_external_id=$2 AND NOT a.withdrawn ORDER BY a.created_at DESC LIMIT 100").bind(work).bind(raw["comment_external_id"].as_str()).fetch_all(db.pool()).await?
    } else {
        Vec::new()
    };
    value["bookmarks"] = json!(bookmarks);
    value["problems"] = effective["linkedProblems"].clone();
    value["scope"] = effective["scope"].clone();
    Ok(value)
}

fn research_read_error(error: linggan_storage_postgres::StorageError) -> ModelError {
    match error {
        linggan_storage_postgres::StorageError::Statement(sqlx::Error::Protocol(ref code))
            if code == "CI_SOURCE_UNAVAILABLE" =>
        {
            ModelError::Source
        }
        linggan_storage_postgres::StorageError::Statement(e)
        | linggan_storage_postgres::StorageError::Connect(e) => ModelError::Database(e),
        linggan_storage_postgres::StorageError::UnusableSchemaName(_) => ModelError::Invalid,
    }
}
