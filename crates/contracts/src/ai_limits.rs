//! Resource ceilings, not provider capability or permission. Node consumes the same JSON source.
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

pub const SOURCE: &str = include_str!("ai_limits.json");
pub static AI_LIMITS: LazyLock<AiLimits> =
    LazyLock::new(|| serde_json::from_str(SOURCE).expect("versioned resource limits parse"));

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiLimits {
    pub contract: String,
    pub max_context_tokens: u64,
    pub max_output_tokens: u64,
    pub max_timeout_seconds: u64,
    pub max_attempts: u32,
    pub default_concurrency: u32,
    pub max_concurrency: u32,
    pub round_size: u32,
    pub max_batch_items: u32,
    pub adapter_deadline_allowance_seconds: u64,
    pub receipt_finalization_allowance_seconds: u64,
    pub max_skill_files: u32,
    pub max_skill_file_bytes: u64,
    pub max_skill_total_bytes: u64,
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub max_text_bytes: u64,
    pub max_stream_bytes: u64,
    pub base_stream_bytes: u64,
    pub stream_bytes_per_output_token: u64,
    pub max_response_schema_bytes: u64,
    pub legacy_input_tokens: u64,
    pub legacy_output_tokens: u64,
    pub legacy_timeout_seconds: u64,
    pub legacy_request_bytes: u64,
    pub legacy_topic_request_bytes: u64,
    pub legacy_comment_schema_bytes: u64,
}
impl AiLimits {
    pub fn drain_grace_seconds(&self) -> u64 {
        self.max_timeout_seconds
            + self.adapter_deadline_allowance_seconds
            + self.receipt_finalization_allowance_seconds
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_bounds_cover_pinned_catalog_without_granting_capacity() {
        assert!(AI_LIMITS.max_context_tokens >= 1_000_000);
        assert!(AI_LIMITS.max_output_tokens >= 384_000);
        assert_eq!(AI_LIMITS.default_concurrency, 4);
        assert_eq!(AI_LIMITS.max_concurrency, 8);
        assert_eq!(AI_LIMITS.round_size, 50);
        assert_eq!(AI_LIMITS.max_batch_items, 10000);
        assert_eq!(AI_LIMITS.drain_grace_seconds(), 3616);
        assert!(AI_LIMITS.max_request_bytes >= AI_LIMITS.max_context_tokens * 6);
        assert!(AI_LIMITS.max_text_bytes >= AI_LIMITS.max_output_tokens * 4);
        assert!(AI_LIMITS.max_response_bytes > AI_LIMITS.max_text_bytes);
        assert!(AI_LIMITS.max_stream_bytes >= AI_LIMITS.max_response_bytes);
    }
}
