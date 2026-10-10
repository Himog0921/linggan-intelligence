use super::*;
use crate::model_settings::ModelError;
use sqlx::{Postgres, Row, Transaction};

/// An unresolved provider send owns its actual source ranges across configs,
/// methods and later discussion assignments. It cannot be replayed by comparison.
pub(crate) async fn blocks_comparison(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    comparison: &ResearchInput,
    inputs: &[ResearchInput],
) -> Result<bool, ModelError> {
    let requested = scope(&json!({"workRefs":comparison.coverage["scopeWorkRefs"]}))
        .unwrap_or_else(|| vec![comparison.work.work_ref]);
    let scope_strings: Vec<_> = requested.iter().map(Uuid::to_string).collect();
    let rows = sqlx::query("SELECT input_refs FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND state='unknown_dispatch' AND (work_public_ref=ANY($2) OR COALESCE(input_refs#>'{source,contextWorkRefs}',input_refs->'contextWorkRefs','[]'::jsonb) ?| $3 OR COALESCE(input_refs#>'{source,coverage,scopeWorkRefs}',input_refs#>'{coverage,scopeWorkRefs}','[]'::jsonb) ?| $3)")
        .bind(domain).bind(&requested).bind(&scope_strings).fetch_all(&mut **tx).await?;
    let mut references: Vec<Value> = rows
        .into_iter()
        .flat_map(|row| {
            let manifest: Value = row.get("input_refs");
            manifest.get("source").unwrap_or(&manifest)["fragments"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .collect();
    if references.is_empty() {
        return Ok(false);
    }
    research::attach_legacy_media_aliases(tx, &mut references).await?;
    let Some(primary) = inputs
        .iter()
        .find(|input| input.work.work_ref == comparison.work.work_ref)
    else {
        return Ok(true);
    };
    let full = research::with_comparison_context(primary.clone(), inputs, &requested);
    Ok(research::overlaps_unknown_dispatch(
        comparison,
        &full,
        &references,
    ))
}
