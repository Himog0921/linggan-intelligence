//! Frozen context choices and source-gated, expiring request diagnostics.
use crate::{
    comment_cleaning::{CleanComment, outbound},
    comment_packet::{ResearchPacket, SYSTEM},
    comment_research::comment_source_hash,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextPolicy {
    pub work_body: bool,
    pub ocr: bool,
    pub asr: bool,
    pub parent: bool,
    pub existing_problems: bool,
    pub recall_limit: usize,
    pub max_comments: usize,
    pub record_content: bool,
}
impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            work_body: true,
            ocr: true,
            asr: true,
            parent: true,
            existing_problems: true,
            recall_limit: 10,
            max_comments: 7,
            record_content: true,
        }
    }
}
impl ContextPolicy {
    pub fn parse(v: Value) -> Result<Self, ModelError> {
        let p: Self = serde_json::from_value(v).map_err(|_| ModelError::Invalid)?;
        if !(1..=10).contains(&p.recall_limit) || !(1..=30).contains(&p.max_comments) {
            return Err(ModelError::Invalid);
        }
        Ok(p)
    }
    pub fn semantic_value(&self) -> Value {
        let mut v = json!(self);
        v.as_object_mut().unwrap().remove("recordContent");
        v
    }
    pub fn apply(&self, context: &mut Value) {
        if !self.work_body {
            context["work"]["body"] = Value::Null;
        }
        if !self.parent {
            context["parent"] = Value::Null;
            context["parentState"] = json!("EXCLUDED_BY_POLICY");
        }
        if let Some(items) = context["derivatives"].as_array_mut() {
            items.retain(|d| match d["kind"].as_str() {
                Some("ocr") | Some("ocr_text") => self.ocr,
                Some("asr") | Some("asr_text") | Some("transcript") | Some("transcription") => {
                    self.asr
                }
                _ => false,
            });
        }
        context["researchPolicy"] = self.semantic_value();
    }
}
pub async fn settings(db: &Database) -> Result<Value, ModelError> {
    let r =
        sqlx::query("SELECT revision,policy FROM linggan_comment_context_settings WHERE singleton")
            .fetch_one(db.pool())
            .await?;
    Ok(
        json!({"revision":r.get::<i64,_>("revision"),"policy":ContextPolicy::parse(r.get("policy"))?}),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveContext {
    pub expected_revision: i64,
    pub policy: ContextPolicy,
}
pub async fn save_settings(db: &Database, r: &SaveContext) -> Result<Value, ModelError> {
    ContextPolicy::parse(json!(r.policy))?;
    let revision:Option<i64>=sqlx::query_scalar("UPDATE linggan_comment_context_settings SET policy=$2,revision=revision+1,updated_at=scope_001_now() WHERE singleton AND revision=$1 RETURNING revision").bind(r.expected_revision).bind(json!(r.policy)).fetch_optional(db.pool()).await?;
    Ok(
        json!({"revision":revision.ok_or(ModelError::Conflict)?,"policy":r.policy,"state":"SAVED_NO_ANALYSIS_STARTED"}),
    )
}

pub async fn begin_trace(
    db: &Database,
    invocation: Uuid,
    packet: Uuid,
    research: &ResearchPacket,
    policy: &ContextPolicy,
) -> Result<(), ModelError> {
    let prompt: Value = serde_json::from_str(&research.prompt).map_err(|_| ModelError::Invalid)?;
    let input = json!({"contract":crate::comment_daily::DAILY_RULE,"system":SYSTEM,"work":prompt["untrustedMaterial"]["work"],"comments":prompt["untrustedMaterial"]["comments"],"existingProblems":prompt["existingProblems"],"policy":policy,"task":prompt["task"],"outputSchema":prompt["outputSchema"]});
    let hashes: Value = research
        .inputs
        .iter()
        .map(|i| (i.source_ref.to_string(), json!(i.source_sha256)))
        .collect::<serde_json::Map<String, Value>>()
        .into();
    let work = research
        .inputs
        .first()
        .and_then(|i| i.context["workUrl"].as_str())
        .and_then(|s| s.rsplit('/').next());
    let jobs: Vec<Value> = research
        .inputs
        .iter()
        .flat_map(|i| i.context["derivatives"].as_array().into_iter().flatten())
        .filter(|v| v["state"] == "ACQUIRED" && v["displayText"].is_string())
        .map(|v| v["jobRef"].clone())
        .collect();
    let guard = json!({"contextRefs":{"researchSourceRefs":research.context_refs,"workRef":work,"mediaJobs":jobs}});
    sqlx::query("INSERT INTO linggan_comment_request_trace(invocation_ref,packet_ref,input_content,input_hash,source_hashes,context_guard,policy,events) VALUES($1,$2,$3,$4,$5,$6,$7,jsonb_build_array(jsonb_build_object('kind','request_started','at',scope_001_now()))) ON CONFLICT DO NOTHING")
 .bind(invocation).bind(packet).bind(if policy.record_content{Some(input)}else{None}).bind(comment_source_hash(&research.prompt)).bind(hashes).bind(guard).bind(json!(policy)).execute(db.pool()).await?;
    Ok(())
}
pub async fn expire_content(db: &Database) -> Result<(), ModelError> {
    // Reads independently enforce expiry and access, even if this worker is stopped.
    sqlx::query("UPDATE linggan_comment_request_trace SET input_content=NULL,output_content=NULL,purged_at=scope_001_now() WHERE purged_at IS NULL AND (expires_at<=scope_001_now() OR NOT linggan_ci_analysis_context_readable(context_guard))").execute(db.pool()).await?;
    Ok(())
}
fn mask_text(text: &str) -> String {
    outbound(CleanComment {
        text: text.into(),
        offsets: vec![],
        state: "direct".into(),
        reasons: vec![],
    })
    .text
}
fn mask_value(value: &mut Value) {
    match value {
        Value::String(s) => *s = mask_text(s),
        Value::Array(a) => a.iter_mut().for_each(mask_value),
        Value::Object(o) => o.values_mut().for_each(mask_value),
        _ => {}
    }
}
fn masked_response(text: &str) -> String {
    let text = if let Ok(mut value) = serde_json::from_str::<Value>(text) {
        mask_value(&mut value);
        value.to_string()
    } else {
        mask_text(text)
    };
    text
}
fn bounded_response(text: &str) -> String {
    text.chars()
        .scan(0usize, |size, c| {
            *size += c.len_utf8();
            (*size <= 65536).then_some(c)
        })
        .collect()
}
pub async fn record_response(
    db: &Database,
    invocation: Uuid,
    response: Option<&crate::pi_adapter::PiResponse>,
    policy: &ContextPolicy,
) -> Result<(), ModelError> {
    let masked = if policy.record_content {
        response
            .and_then(|p| p.text.as_deref())
            .map(masked_response)
    } else {
        None
    };
    let truncated = masked.as_ref().is_some_and(|text| text.len() > 65536);
    let output = masked.as_deref().map(bounded_response);
    sqlx::query("UPDATE linggan_comment_request_trace SET output_content=CASE WHEN expires_at>scope_001_now() AND purged_at IS NULL THEN $2 ELSE NULL END,events=events||jsonb_build_array(jsonb_build_object('kind',$3::text,'at',scope_001_now(),'displayTruncated',$4::boolean)) WHERE invocation_ref=$1")
 .bind(invocation).bind(output).bind(if response.is_some_and(|p|p.ok){"response_received"}else{"request_failed"}).bind(truncated).execute(db.pool()).await?;
    Ok(())
}

pub async fn request_detail(
    db: &Database,
    batch: Uuid,
    invocation: Uuid,
    domain: Uuid,
) -> Result<Value, ModelError> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND is_own_domain)",
    )
    .bind(domain)
    .fetch_one(db.pool())
    .await?;
    if !allowed {
        return Err(ModelError::Source);
    }
    let r=sqlx::query("SELECT t.*,t.expires_at>scope_001_now() AS fresh,p.purpose,p.source_refs,p.context_refs,v.state,v.failure_code,v.created_at,v.finished_at FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) LEFT JOIN linggan_comment_request_trace t USING(invocation_ref) WHERE p.batch_ref=$1 AND p.invocation_ref=$2 AND NOT EXISTS(SELECT 1 FROM unnest(p.source_refs) ref LEFT JOIN linggan_material_comment c ON c.material_ref=ref LEFT JOIN linggan_material_content w ON w.public_ref=c.content_public_ref WHERE w.domain_ref IS DISTINCT FROM $3)").bind(batch).bind(invocation).bind(domain).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    let guard = r.get::<Option<Value>, _>("context_guard");
    let Some(guard) = guard else {
        return Ok(
            json!({"purpose":r.get::<String,_>("purpose"),"availability":"NOT_RECORDED","input":null,"output":null,"validation":[],"events":[]}),
        );
    };
    if !crate::comment_daily_read::context_readable(db, &guard).await? {
        return Ok(
            json!({"purpose":r.get::<String,_>("purpose"),"availability":"RESTRICTED","input":null,"output":null,"validation":[],"events":[]}),
        );
    }
    let fresh = r.get::<Option<bool>, _>("fresh") == Some(true)
        && r.get::<Option<Value>, _>("input_content").is_some();
    let output = r.get::<Option<String>, _>("output_content");
    Ok(
        json!({"purpose":r.get::<String,_>("purpose"),"availability":if fresh{"AVAILABLE"}else if r.get::<Option<bool>,_>("fresh")==Some(false){"EXPIRED"}else{"NOT_RECORDED"},
 "input":if fresh{r.get::<Option<Value>,_>("input_content")}else{None},"output":if fresh{json!({"received":output.is_some(),"redacted":true,"truncated":r.get::<Option<Value>,_>("events").is_some_and(|events|events.as_array().is_some_and(|a|a.iter().any(|e|e["displayTruncated"]==true))),"json":output.as_deref().and_then(|s|serde_json::from_str::<Value>(s).ok()),"text":output})}else{Value::Null},
 "validation":r.get::<Option<Value>,_>("validation"),"events":r.get::<Option<Value>,_>("events"),"policy":r.get::<Option<Value>,_>("policy"),"inputHash":r.get::<Option<String>,_>("input_hash"),"outcomes":r.get::<Option<Value>,_>("outcomes")}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_redaction_preserves_json_and_long_text_without_changing_quotes() {
        let raw=json!({"quote":"&quot;全角Ａ\n保持换行", "contact":"13812345678", "long":"内容".repeat(10000)}).to_string();
        let value: Value = serde_json::from_str(&masked_response(&raw)).unwrap();
        assert_eq!(value["quote"], "&quot;全角Ａ\n保持换行");
        assert_eq!(value["long"].as_str().unwrap().chars().count(), 20000);
        assert!(!value["contact"].as_str().unwrap().contains("13812345678"));
        let large = "中".repeat(30000);
        let clipped = bounded_response(&large);
        assert!(clipped.len() <= 65536);
        assert!(clipped.ends_with('中'));
    }
    #[test]
    fn policies_exclude_only_selected_context_and_diagnostics_do_not_change_semantics() {
        let a = ContextPolicy::default();
        let b = ContextPolicy {
            record_content: false,
            ..a.clone()
        };
        assert_eq!(a.semantic_value(), b.semantic_value());
        let mut context = json!({"work":{"title":{"value":"标题"},"body":{"value":"正文"}},"parent":{"body":"父评论"},"derivatives":[{"kind":"ocr_text"},{"kind":"asr_text"}]});
        let off = ContextPolicy {
            work_body: false,
            parent: false,
            ocr: false,
            ..a
        };
        off.apply(&mut context);
        assert_eq!(context["work"]["title"]["value"], "标题");
        assert!(context["work"]["body"].is_null());
        assert!(context["parent"].is_null());
        assert_eq!(context["derivatives"], json!([{"kind":"asr_text"}]));
        assert!(ContextPolicy::parse(json!({"maxComments":31})).is_err());
        assert!(ContextPolicy::parse(json!({"recallLimit":0})).is_err());
    }
}
