//! Constrain machine hierarchy changes to candidates created in the accepting invocation.
use super::*;

/// A machine hierarchy can only attach a candidate first created in this invocation.
/// Existing candidates and adjudicated definitions retain their current bindings.
pub(crate) async fn bind_new_candidate_parent_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    child: Uuid,
    parent: Uuid,
    parent_definition: Uuid,
    invocation: Uuid,
) -> Result<(), TopicMapError> {
    lock(tx, "topic-map:commands").await?;
    let valid_parent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_definition d JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE d.topic_ref=$1 AND d.definition_ref=$2 AND d.version=(SELECT max(version) FROM linggan_topic_definition WHERE topic_ref=$1) AND d.lifecycle_state<>'retired' AND b.domain_ref=$3 AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=$1))")
        .bind(parent).bind(parent_definition).bind(domain).fetch_one(&mut **tx).await?;
    let cycle: bool = sqlx::query_scalar("WITH RECURSIVE current AS(SELECT DISTINCT ON(topic_ref) topic_ref,parent_topic_ref,domain_ref FROM linggan_topic_map_binding ORDER BY topic_ref,version DESC), ancestors AS(SELECT topic_ref,parent_topic_ref FROM current WHERE topic_ref=$1 AND domain_ref=$3 UNION SELECT c.topic_ref,c.parent_topic_ref FROM current c JOIN ancestors a ON c.topic_ref=a.parent_topic_ref) SELECT $1=$2 OR EXISTS(SELECT 1 FROM ancestors WHERE topic_ref=$2)")
        .bind(parent).bind(child).bind(domain).fetch_one(&mut **tx).await?;
    if !valid_parent || cycle {
        return Err(TopicMapError::Invalid("invalid candidate parent"));
    }
    let newly_created: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_definition d JOIN linggan_topic_map_concept_rule r ON r.definition_ref=d.definition_ref JOIN LATERAL(SELECT * FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE d.topic_ref=$1 AND d.version=1 AND d.lifecycle_state='candidate' AND r.invocation_ref=$2 AND b.domain_ref=$3 AND b.version=1 AND b.parent_topic_ref IS NULL)")
        .bind(child).bind(invocation).bind(domain).fetch_one(&mut **tx).await?;
    if !newly_created {
        return Ok(());
    }
    let key = format!("topic-map-candidate-parent:{}", child.simple());
    let digest = hash(&(domain, child, parent, parent_definition, invocation))?;
    let r = receipt(tx, &key, &digest, "candidate_parent", Some(child), 2).await?;
    sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref)VALUES($1,$2,$3,$4,2,$5)")
        .bind(Uuid::new_v4()).bind(child).bind(domain).bind(parent).bind(r.receipt_ref).execute(&mut **tx).await?;
    Ok(())
}
