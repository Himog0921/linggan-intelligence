-- A source work's existing Topic membership is contextual provenance, not a new Topic
-- classification. Comment grouping never writes the formal definition or its material pack.
CREATE TABLE linggan_ci_comment_topic_association (
 association_hash text PRIMARY KEY CHECK(association_hash ~ '^[0-9a-f]{64}$'),
 group_ref uuid NOT NULL REFERENCES linggan_ci_semantic_group,
 group_revision bigint NOT NULL,
 atom_ref uuid NOT NULL REFERENCES linggan_ci_semantic_atom,
 analysis_ref uuid NOT NULL REFERENCES linggan_comment_analysis_work,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 classification_run_ref uuid NOT NULL REFERENCES linggan_topic_classification_run,
 work_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 relation text NOT NULL CHECK(relation='source_work_member'),
 source_member_role text NOT NULL CHECK(source_member_role IN('support','challenge','boundary')),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(classification_run_ref,work_ref) REFERENCES linggan_topic_material_member(classification_run_ref,work_public_ref),
 UNIQUE(group_ref,group_revision,atom_ref,definition_ref)
);
CREATE INDEX ci_comment_topic_by_group ON linggan_ci_comment_topic_association(group_ref,group_revision);
CREATE TRIGGER ci_comment_topic_association_immutable BEFORE UPDATE OR DELETE ON linggan_ci_comment_topic_association
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- Legacy/manual problems have no semantic group. For a grouped problem, historical
-- memberships must never remain visible as current after a split, merge or withdrawal.
CREATE FUNCTION linggan_ci_group_problem_member_current(problem uuid,canonical uuid,analysis uuid)
RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT NOT EXISTS(SELECT 1 FROM linggan_ci_semantic_group WHERE problem_ref=problem)
 OR EXISTS(SELECT 1 FROM linggan_ci_semantic_group g
   JOIN linggan_ci_atom_membership m USING(group_ref)
   JOIN linggan_ci_semantic_atom_current a USING(atom_ref)
   WHERE g.problem_ref=problem AND g.state='active' AND m.current AND m.relation='same'
     AND a.canonical_ref=canonical AND a.analysis_ref=analysis)
$$;
