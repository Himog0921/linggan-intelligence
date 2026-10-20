//! Offline package presets are reference metadata, never current callability or an egress grant.
use super::{provider_store::OutputMode, store::FoundationError, usage::TokenPrices};
use crate::{pi_adapter::PiAdapter, pi_offline::OfflineHelper};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogLimits {
    pub context_tokens: u64,
    pub max_output_tokens: u64,
    pub max_timeout_seconds: Option<u64>,
    pub thinking_levels: BTreeSet<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogPreset {
    pub provider: String,
    pub model_id: String,
    pub name: String,
    pub api: String,
    pub source_version: String,
    pub supported: bool,
    pub reasons: Vec<String>,
    pub limits: CatalogLimits,
    pub compat: Value,
    pub thinking_level_map: Value,
    pub output_mode: OutputMode,
    pub strict_schema: bool,
    pub prices: TokenPrices,
    pub operator_fields: Vec<String>,
    pub unknown_fields: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogPage {
    pub contract: String,
    pub sdk_version: String,
    pub providers: Vec<String>,
    pub total: u32,
    pub offset: u32,
    pub limit: u16,
    pub presets: Vec<CatalogPreset>,
    pub has_more: bool,
}
pub async fn read_catalog(
    adapter: &PiAdapter,
    query: &CatalogQuery,
) -> Result<CatalogPage, FoundationError> {
    if [
        &query.api,
        &query.provider,
        &query.query,
        &query.model_id,
        &query.source_version,
    ]
    .into_iter()
    .flatten()
    .any(|v| v.len() > 200)
        || query.limit.is_some_and(|n| n == 0 || n > 100)
        || query.offset.is_some_and(|n| n > 10000)
        || query.api.as_ref().is_some_and(|a| {
            ![
                "openai-completions",
                "openai-responses",
                "anthropic-messages",
            ]
            .contains(&a.as_str())
        })
    {
        return Err(FoundationError::Invalid);
    }
    let page: CatalogPage = adapter
        .call_offline(OfflineHelper::Catalog, query)
        .await
        .map_err(|_| FoundationError::LoaderUnavailable)?;
    if page.contract != "ai-pi-catalog.v1"
        || page.sdk_version
            != linggan_contracts::ai_protocol::PROTOCOL["sdkVersion"]
                .as_str()
                .unwrap()
        || page.presets.len() > usize::from(query.limit.unwrap_or(50))
        || page.offset != query.offset.unwrap_or(0)
        || page.limit != query.limit.unwrap_or(50)
        || page.total > 10000
    {
        return Err(FoundationError::LoaderUnavailable);
    }
    Ok(page)
}
pub(crate) async fn preset_for_identity(
    conn: &mut sqlx::PgConnection,
    identity: Uuid,
    endpoint: Uuid,
    source: &str,
) -> Result<CatalogPreset, FoundationError> {
    let (model_id,api):(String,String)=sqlx::query_as("SELECT i.model_id,e.api FROM linggan_ai_model_identity i JOIN linggan_ai_endpoint e USING(connection_ref) WHERE i.identity_ref=$1 AND e.endpoint_ref=$2")
        .bind(identity).bind(endpoint).fetch_optional(&mut *conn).await?.ok_or(FoundationError::NotFound)?;
    let page = read_catalog(
        &PiAdapter::configured(),
        &CatalogQuery {
            api: Some(api),
            model_id: Some(model_id),
            source_version: Some(source.to_owned()),
            limit: Some(1),
            ..Default::default()
        },
    )
    .await?;
    let preset = page
        .presets
        .into_iter()
        .next()
        .ok_or(FoundationError::Invalid)?;
    Ok(preset)
}
