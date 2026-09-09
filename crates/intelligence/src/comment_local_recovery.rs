//! Re-accept a retained complete provider response through the current strict parser, without
//! another provider call. Original failed requests and their known/unknown charges stay intact.
use crate::{
    comment_packet::build_packet_with_rule, comment_research::comment_source_hash,
    comment_semantic_reservation::frozen_rule, model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
const PARSER: &str = "comment-normalization.v2";
pub(crate) async fn recover(db: &Database) -> Result<usize, ModelError> {
    let packets=sqlx::query(r#"SELECT p.packet_ref,p.batch_ref,p.invocation_ref,p.source_refs,t.output_content,t.input_hash,t.context_guard,b.context_policy,b.config_ref,b.request
      FROM linggan_comment_daily_packet p JOIN linggan_comment_request_trace t USING(invocation_ref)
      JOIN linggan_comment_daily_batch b USING(batch_ref)
      WHERE p.purpose='extraction' AND p.state IN('failed','partial')
        AND t.expires_at>scope_001_now() AND t.purged_at IS NULL AND t.output_content IS NOT NULL
        AND linggan_ci_analysis_context_readable(t.context_guard)
        AND EXISTS(SELECT 1 FROM jsonb_array_elements(t.events) e WHERE e->>'kind'='response_received' AND COALESCE(e->>'displayTruncated','false')='false')
        AND NOT EXISTS(SELECT 1 FROM linggan_comment_local_recovery r WHERE r.invocation_ref=p.invocation_ref AND r.parser_version=$1)
        AND EXISTS(SELECT 1 FROM linggan_comment_daily_item i WHERE i.batch_ref=p.batch_ref AND i.source_ref=ANY(p.source_refs) AND i.state='failed'
          AND i.failure_code IN('json_invalid','schema_invalid','invalid_output','item_schema_invalid','json_fence_invalid'))
      ORDER BY t.expires_at,p.packet_ref LIMIT 10"#).bind(PARSER).fetch_all(db.pool()).await?;
    let mut count = 0;
    for row in packets {
        let refs: Vec<Uuid> = row.get("source_refs");
        let batch: Uuid = row.get("batch_ref");
        let invocation: Uuid = row.get("invocation_ref");
        let policy = crate::comment_runtime::ContextPolicy::parse(row.get("context_policy"))?;
        let rule = frozen_rule(db, &row.get::<Value, _>("request")).await?;
        let config: Uuid = row.get("config_ref");
        let research = match build_packet_with_rule(
            db,
            &refs,
            &crate::comment_daily::version(config),
            &policy,
            &rule,
        )
        .await
        {
            Ok(p) => p,
            Err(ModelError::Source | ModelError::NotFound) => continue,
            Err(e) => return Err(e),
        };
        if comment_source_hash(&research.prompt) != row.get::<String, _>("input_hash") {
            continue;
        }
        let output: String = row.get("output_content");
        let receipt_ref = Uuid::new_v4();
        let mut analyses = Vec::new();
        let mut recovered = Vec::new();
        let parsed = research.parse_for_rule(&output);
        let mut tx = db.pool().begin().await?;
        sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
            .fetch_one(&mut *tx)
            .await?;
        let already:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_local_recovery WHERE invocation_ref=$1 AND parser_version=$2)").bind(invocation).bind(PARSER).fetch_one(&mut *tx).await?;
        if already {
            continue;
        }
        let trace_valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_request_trace WHERE invocation_ref=$1 AND expires_at>scope_001_now() AND purged_at IS NULL AND input_hash=$2 AND output_content=$3 AND linggan_ci_analysis_context_readable(context_guard))")
            .bind(invocation).bind(row.get::<String,_>("input_hash")).bind(&output).fetch_one(&mut *tx).await?;
        if !trace_valid {
            continue;
        }
        if let Ok(outcomes) = parsed {
            for (input, outcome) in research.inputs.iter().zip(outcomes) {
                let Ok(mut result) = outcome else { continue };
                let readable:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_research_readable source WHERE source.material_ref=$1 AND encode(sha256(convert_to(source.body_text,'UTF8')),'hex')=$2) AND linggan_ci_analysis_context_readable($3)")
                    .bind(input.source_ref).bind(&input.source_sha256).bind(&result).fetch_one(&mut *tx).await?;
                if !readable {
                    continue;
                }
                let semantic:Option<(Uuid,String)>=sqlx::query_as("SELECT sw.semantic_ref,sw.fingerprint FROM linggan_comment_daily_item i JOIN linggan_comment_semantic_work sw USING(semantic_ref) WHERE i.batch_ref=$1 AND i.source_ref=$2 AND i.state='failed' AND sw.state='failed' AND sw.invocation_ref=$3 FOR UPDATE OF sw")
                    .bind(batch).bind(input.source_ref).bind(invocation).fetch_optional(&mut *tx).await?;
                let Some((semantic, fingerprint)) = semantic else {
                    continue;
                };
                let analysis = Uuid::new_v4();
                let state = if result["semantic"]["outcome"] == "no_signal" {
                    "no_signal"
                } else {
                    "succeeded"
                };
                let version = format!(
                    "{}:{semantic}:local:{receipt_ref}",
                    crate::comment_daily::version(config)
                );
                result["modelVersion"] = json!(version);
                result["semanticFingerprint"] = json!(fingerprint);
                result["localRecoveryRef"] = json!(receipt_ref);
                result["ruleRevisionRef"] = json!(rule.rule_revision_ref);
                result["ruleHash"] = json!(rule.canonical_hash);
                sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result,attempts) VALUES($1,$2,$3,$4,$5,$6,0)")
                    .bind(analysis).bind(input.source_ref).bind(rule.rule_version.as_str()).bind(version).bind(state).bind(result).execute(&mut *tx).await?;
                sqlx::query("UPDATE linggan_comment_semantic_work SET state=$2,analysis_ref=$3,failure_code=NULL,updated_at=scope_001_now() WHERE semantic_ref=$1").bind(semantic).bind(state).bind(analysis).execute(&mut *tx).await?;
                sqlx::query("UPDATE linggan_comment_daily_item SET state=$2,analysis_ref=$3,failure_code=NULL WHERE semantic_ref=$1 AND state='failed'").bind(semantic).bind(state).bind(analysis).execute(&mut *tx).await?;
                recovered.push(input.source_ref);
                analyses.push(analysis);
            }
        }
        let receipt = json!({"method":"retained_response_strict_revalidation","parserVersion":PARSER,"accepted":recovered.len(),"newProviderCalls":0,"originalRequestAndChargePreserved":true,"normalization":research.normalization_kind_for_rule(&output)});
        sqlx::query("INSERT INTO linggan_comment_local_recovery(invocation_ref,parser_version,receipt_ref,packet_ref,source_refs,analysis_refs,input_hash,output_hash,receipt) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
          .bind(invocation).bind(PARSER).bind(receipt_ref).bind(row.get::<Uuid,_>("packet_ref")).bind(&recovered).bind(analyses).bind(row.get::<String,_>("input_hash")).bind(comment_source_hash(&output)).bind(receipt).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_model_invocation SET result=COALESCE(result,'{}'::jsonb)||jsonb_build_object('localRecoveryRef',$2::uuid,'locallyRecoveredComments',$3::integer) WHERE invocation_ref=$1")
          .bind(invocation).bind(receipt_ref).bind(recovered.len() as i32).execute(&mut *tx).await?;
        tx.commit().await?;
        count += recovered.len();
    }
    Ok(count)
}
