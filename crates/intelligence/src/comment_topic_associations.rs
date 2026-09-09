//! Topic links are derived from existing work membership and frozen definition references.
//! They do not claim that the comment itself was classified into a formal Topic.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde_json::Value;
use uuid::Uuid;

pub(crate) async fn refresh(db: &Database) -> Result<u64, ModelError> {
    let result = sqlx::query(r#"
      INSERT INTO linggan_ci_comment_topic_association(association_hash,group_ref,group_revision,
        atom_ref,analysis_ref,topic_ref,definition_ref,classification_run_ref,work_ref,relation,source_member_role)
      SELECT encode(sha256(convert_to(concat_ws(':',g.group_ref,g.revision,a.atom_ref,d.definition_ref),'UTF8')),'hex'),
        g.group_ref,g.revision,a.atom_ref,a.analysis_ref,d.topic_ref,d.definition_ref,r.classification_run_ref,
        s.work_ref,'source_work_member',member.role
      FROM linggan_ci_semantic_group g JOIN linggan_ci_atom_membership m USING(group_ref)
      JOIN linggan_ci_semantic_atom_current a USING(atom_ref)
      JOIN linggan_ci_source s ON s.canonical_ref=a.canonical_ref AND s.domain_ref=a.domain_ref
      JOIN linggan_topic_material_member member ON member.work_public_ref=s.work_ref
      JOIN linggan_topic_classification_run r USING(classification_run_ref)
      JOIN linggan_topic_definition d USING(definition_ref)
      WHERE g.state='active' AND m.current AND m.relation='same'
        AND NOT EXISTS(SELECT 1 FROM linggan_topic_definition newer WHERE newer.topic_ref=d.topic_ref AND newer.version>d.version)
        AND NOT EXISTS(SELECT 1 FROM linggan_ci_comment_topic_association old WHERE old.group_ref=g.group_ref
          AND old.group_revision=g.revision AND old.atom_ref=a.atom_ref AND old.definition_ref=d.definition_ref)
      ORDER BY g.group_ref,a.atom_ref,d.definition_ref LIMIT 500 ON CONFLICT DO NOTHING
    "#).execute(db.pool()).await?;
    Ok(result.rows_affected())
}

pub(crate) async fn read(db: &Database, domain: Uuid, problem: Uuid) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(r#"
      SELECT COALESCE(jsonb_agg(jsonb_build_object('topicRef',topic_ref,'definitionRef',definition_ref,
        'definitionVersion',version,'name',display_name,'canonicalKey',canonical_key,'works',works,
        'comments',comments,'associationRefs',receipts,'relation','source_work_member',
        'meaning','相关原声来自该主题已收录的作品；不表示评论已被划入正式主题。')),'[]') FROM (
        SELECT t.topic_ref,d.definition_ref,d.version,d.display_name,t.canonical_key,
          count(DISTINCT link.work_ref) works,count(DISTINCT atom.canonical_ref) comments,
          (array_agg(DISTINCT link.association_hash))[1:100] receipts
        FROM linggan_ci_comment_topic_association link JOIN linggan_ci_semantic_group g USING(group_ref)
        JOIN linggan_ci_atom_membership m ON m.group_ref=g.group_ref AND m.atom_ref=link.atom_ref
        JOIN linggan_ci_semantic_atom_current atom ON atom.atom_ref=link.atom_ref AND atom.analysis_ref=link.analysis_ref
        JOIN linggan_topic_definition d USING(definition_ref) JOIN linggan_topic_workspace t ON t.topic_ref=d.topic_ref
        WHERE g.problem_ref=$2 AND g.domain_ref=$1 AND g.state='active' AND m.current AND m.relation='same'
          AND g.revision=link.group_revision
          AND NOT EXISTS(SELECT 1 FROM linggan_topic_definition newer WHERE newer.topic_ref=d.topic_ref AND newer.version>d.version)
        GROUP BY t.topic_ref,d.definition_ref,d.version,d.display_name,t.canonical_key ORDER BY works DESC,t.topic_ref LIMIT 20
      ) current_links
    "#).bind(domain).bind(problem).fetch_one(db.pool()).await
}
