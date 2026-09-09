-- CI-AUTO-004 P4. Derived atoms retain lineage to immutable accepted analysis. No source facts
-- or formal domain topics are replaced by clustering output.
CREATE TABLE linggan_ci_semantic_atom (
 atom_ref uuid PRIMARY KEY,
 analysis_ref uuid NOT NULL REFERENCES linggan_comment_analysis_work(work_ref),
 domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 canonical_ref uuid NOT NULL,
 source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
 kind text NOT NULL CHECK(kind IN('problem','need','solution','stance','story','quote','emotion')),
 ordinal integer NOT NULL CHECK(ordinal BETWEEN 1 AND 100),
 atom_contract_version text NOT NULL CHECK(atom_contract_version IN('comment-atoms.v4-adapter.1','comment-atoms.v5.1')),
 definition_hash text NOT NULL CHECK(definition_hash ~ '^[0-9a-f]{64}$'),
 meaning text NOT NULL CHECK(length(meaning) BETWEEN 1 AND 1000),
 context_text text CHECK(length(context_text)<=1000),
 target text CHECK(length(target) BETWEEN 1 AND 200),
 position text CHECK(position IN('support','oppose','concern','mixed','neutral')),
 CHECK ((kind='stance' AND target IS NOT NULL AND position IS NOT NULL) OR (kind<>'stance' AND target IS NULL AND position IS NULL)),
 evidence jsonb NOT NULL CHECK(jsonb_typeof(evidence)='array' AND jsonb_array_length(evidence)>0),
 context_evidence jsonb NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(context_evidence)='array'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(analysis_ref,kind,ordinal,atom_contract_version)
);
CREATE INDEX ci_semantic_atom_source ON linggan_ci_semantic_atom(canonical_ref,analysis_ref);
CREATE INDEX ci_semantic_atom_type ON linggan_ci_semantic_atom(domain_ref,kind,atom_ref);
CREATE TRIGGER ci_semantic_atom_immutable BEFORE UPDATE OR DELETE ON linggan_ci_semantic_atom
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_ci_atom_projection (
 analysis_ref uuid PRIMARY KEY REFERENCES linggan_comment_analysis_work(work_ref),
 atom_contract_version text NOT NULL,
 atom_count integer NOT NULL CHECK(atom_count>=0),
 projected_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_ci_semantic_space (
 space_ref uuid PRIMARY KEY,
 model_ref uuid NOT NULL REFERENCES linggan_model_entry(model_ref),
 model_id text NOT NULL,
 model_version text NOT NULL,
 provider_api text NOT NULL,
 endpoint text NOT NULL,
 dimensions integer NOT NULL CHECK(dimensions BETWEEN 2 AND 8192),
 normalization text NOT NULL CHECK(normalization='l2'),
 space_hash text NOT NULL UNIQUE CHECK(space_hash ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER ci_semantic_space_immutable BEFORE UPDATE OR DELETE ON linggan_ci_semantic_space
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ci_atom_vector (
 space_ref uuid NOT NULL REFERENCES linggan_ci_semantic_space,
 definition_hash text NOT NULL CHECK(definition_hash ~ '^[0-9a-f]{64}$'),
 vector_bytes bytea NOT NULL,
 dimensions integer NOT NULL CHECK(dimensions BETWEEN 2 AND 8192),
 invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(space_ref,definition_hash),
 CHECK(octet_length(vector_bytes)=dimensions*4)
);

CREATE TABLE linggan_ci_cluster_run (
 run_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 kind text NOT NULL CHECK(kind IN('problem','need','solution','stance','story','quote','emotion')),
 space_ref uuid NOT NULL REFERENCES linggan_ci_semantic_space,
 snapshot_hash text NOT NULL CHECK(snapshot_hash ~ '^[0-9a-f]{64}$'),
 vector_hash text CHECK(vector_hash ~ '^[0-9a-f]{64}$'),
 policy_hash text NOT NULL CHECK(policy_hash ~ '^[0-9a-f]{64}$'),
 state text NOT NULL CHECK(state IN('pending','running','reviewing','accepted','insufficient','failed','superseded')),
 member_count integer NOT NULL CHECK(member_count BETWEEN 1 AND 200000),
 lease_until timestamptz,
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts>=0),
 quality jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(quality)='object'),
 failure_code text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 finished_at timestamptz,
 UNIQUE(domain_ref,kind,space_ref,snapshot_hash,policy_hash)
);
CREATE UNIQUE INDEX ci_cluster_one_running ON linggan_ci_cluster_run(domain_ref,kind) WHERE state='running';
CREATE TABLE linggan_ci_cluster_input (
 run_ref uuid NOT NULL REFERENCES linggan_ci_cluster_run,
 ordinal integer NOT NULL CHECK(ordinal>=0),
 atom_ref uuid NOT NULL REFERENCES linggan_ci_semantic_atom,
 definition_hash text NOT NULL,
 PRIMARY KEY(run_ref,atom_ref), UNIQUE(run_ref,ordinal)
);
CREATE TRIGGER ci_cluster_input_immutable BEFORE UPDATE OR DELETE ON linggan_ci_cluster_input
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ci_cluster_assignment (
 run_ref uuid NOT NULL,
 atom_ref uuid NOT NULL,
 algorithm text NOT NULL CHECK(algorithm IN('hdbscan','leiden')),
 cluster_key text,
 confidence double precision CHECK(confidence>=0 AND confidence<=1),
 PRIMARY KEY(run_ref,atom_ref,algorithm),
 FOREIGN KEY(run_ref,atom_ref) REFERENCES linggan_ci_cluster_input(run_ref,atom_ref)
);
CREATE TABLE linggan_ci_semantic_group (
 group_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 kind text NOT NULL CHECK(kind IN('problem','need','solution','stance','story','quote','emotion')),
 problem_ref uuid REFERENCES linggan_ci_problem,
 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
 definition text NOT NULL CHECK(length(definition) BETWEEN 1 AND 1000),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision>0),
 state text NOT NULL DEFAULT 'active' CHECK(state IN('active','superseded')),
 superseded_by_run uuid REFERENCES linggan_ci_cluster_run,
 superseded_reason text CHECK(superseded_reason IN('reclustered','source_withdrawn')),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK((state='active' AND superseded_by_run IS NULL AND superseded_reason IS NULL)
    OR (state='superseded' AND superseded_by_run IS NOT NULL AND superseded_reason='reclustered')
    OR (state='superseded' AND superseded_by_run IS NULL AND superseded_reason='source_withdrawn'))
);
CREATE INDEX ci_semantic_group_current ON linggan_ci_semantic_group(domain_ref,kind,group_ref) WHERE state='active';
CREATE TABLE linggan_ci_atom_membership (
 group_ref uuid NOT NULL REFERENCES linggan_ci_semantic_group,
 atom_ref uuid NOT NULL REFERENCES linggan_ci_semantic_atom,
 run_ref uuid REFERENCES linggan_ci_cluster_run,
 relation text NOT NULL CHECK(relation IN('same','related')),
 decision_ref uuid NOT NULL,
 current boolean NOT NULL DEFAULT true,
 superseded_by_run uuid REFERENCES linggan_ci_cluster_run,
 superseded_reason text CHECK(superseded_reason IN('reclustered','source_withdrawn')),
 superseded_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(group_ref,atom_ref,decision_ref),
 CHECK((current AND superseded_by_run IS NULL AND superseded_reason IS NULL AND superseded_at IS NULL)
    OR (NOT current AND superseded_by_run IS NOT NULL AND superseded_reason='reclustered' AND superseded_at IS NOT NULL)
    OR (NOT current AND superseded_by_run IS NULL AND superseded_reason='source_withdrawn' AND superseded_at IS NOT NULL))
);
-- An atom can support related expressions, but only one active group can count it
-- as the same expression. Historical memberships remain append-only evidence.
CREATE UNIQUE INDEX ci_atom_current_same_membership ON linggan_ci_atom_membership(atom_ref) WHERE current AND relation='same';
CREATE UNIQUE INDEX ci_group_atom_current_membership ON linggan_ci_atom_membership(group_ref,atom_ref) WHERE current;
CREATE TABLE linggan_ci_semantic_lineage (
 lineage_ref uuid PRIMARY KEY,
 kind text NOT NULL CHECK(kind IN('create','rename','split','merge','membership')),
 from_groups uuid[] NOT NULL,
 to_groups uuid[] NOT NULL,
 reason text NOT NULL CHECK(length(reason) BETWEEN 1 AND 1000),
 run_ref uuid REFERENCES linggan_ci_cluster_run,
 invocation_ref uuid REFERENCES linggan_model_invocation,
 receipt jsonb NOT NULL CHECK(jsonb_typeof(receipt)='object'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER ci_semantic_lineage_immutable BEFORE UPDATE OR DELETE ON linggan_ci_semantic_lineage
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ci_cluster_review (
 review_ref uuid PRIMARY KEY,
 run_ref uuid NOT NULL REFERENCES linggan_ci_cluster_run,
 cluster_key text NOT NULL,
 algorithm text NOT NULL CHECK(algorithm IN('hdbscan','leiden')),
 sample_refs uuid[] NOT NULL CHECK(cardinality(sample_refs) BETWEEN 1 AND 12),
 snapshot_hash text NOT NULL,
 state text NOT NULL CHECK(state IN('pending','running','accepted','independent','failed','insufficient')),
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 2),
 invocation_ref uuid REFERENCES linggan_model_invocation,
 next_attempt_at timestamptz,
 lease_until timestamptz,
 receipt jsonb NOT NULL DEFAULT '{}',
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(run_ref,cluster_key,algorithm)
);

CREATE VIEW linggan_ci_semantic_atom_current AS
 SELECT a.*,s.work_ref,s.likes,e.source_context_revision
 FROM linggan_ci_semantic_atom a
 JOIN linggan_ci_source s ON s.domain_ref=a.domain_ref AND s.canonical_ref=a.canonical_ref
 JOIN linggan_comment_research_eligibility_current e ON e.current_analysis_ref=a.analysis_ref
   AND e.source_sha256=s.source_sha256 AND e.source_ref=s.source_ref
 JOIN linggan_comment_research_rule_active active ON active.singleton
 JOIN linggan_comment_research_rule_revision rule ON rule.rule_revision_ref=active.rule_revision_ref
 WHERE e.rule_hash=rule.canonical_hash AND e.schema_version=rule.schema_version AND e.selector_version=rule.selector_version
   AND e.source_context_revision=linggan_comment_research_context_revision(s.source_ref)
   AND e.result_state IN('studied','no_signal')
   AND EXISTS(SELECT 1 FROM linggan_comment_analysis_work analysis WHERE analysis.work_ref=a.analysis_ref
     AND analysis.state IN('succeeded','no_signal') AND linggan_ci_analysis_context_readable(analysis.result));

CREATE TABLE linggan_ci_atom_embedding_work (
 space_ref uuid NOT NULL REFERENCES linggan_ci_semantic_space,
 definition_hash text NOT NULL,
 atom_ref uuid NOT NULL REFERENCES linggan_ci_semantic_atom,
 state text NOT NULL CHECK(state IN('pending','running','succeeded','failed')),
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 3),
 unknown_retry_attempts integer NOT NULL DEFAULT 0 CHECK(unknown_retry_attempts BETWEEN 0 AND 1),
 invocation_ref uuid REFERENCES linggan_model_invocation,
 lease_until timestamptz,
 failure_code text,
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(space_ref,definition_hash)
);

-- A read adapter preserves the stored V4/V5 contracts. Only accepted atomic kinds map to
-- research lenses; stance targets and positions must already have been explicitly accepted.
CREATE FUNCTION linggan_ci_research_labels(result jsonb) RETURNS jsonb
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
 SELECT COALESCE(jsonb_agg(DISTINCT label),'[]'::jsonb) FROM (
   SELECT item->>'label' AS label
   FROM jsonb_array_elements(COALESCE(result#>'{semantic,labels}','[]')) item
   WHERE result->>'schemaVersion' IS DISTINCT FROM 'comment-extraction.schema.v5'
   UNION ALL
   SELECT item->>'kind' AS label
   FROM jsonb_array_elements(COALESCE(result#>'{semantic,atoms}','[]')) item
   WHERE result->>'schemaVersion'='comment-extraction.schema.v5'
     AND item->>'kind' IN('need','solution','story','quote')
     AND item->>'basis' IN('explicit','context_resolved')
 ) labels WHERE label IS NOT NULL
$$;

CREATE FUNCTION linggan_ci_research_stances(result jsonb) RETURNS jsonb
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
 SELECT CASE WHEN result->>'schemaVersion'='comment-extraction.schema.v5'
 THEN COALESCE((SELECT jsonb_agg(jsonb_build_object(
   'target',item->>'target','position',item->>'position','meaning',item->>'meaning',
   'evidence',item->'evidence','contextEvidence',item->'contextEvidence'))
   FROM jsonb_array_elements(COALESCE(result#>'{semantic,atoms}','[]')) item
   WHERE item->>'kind'='stance' AND item->>'basis' IN('explicit','context_resolved')
     AND length(item->>'target')>0 AND item->>'position' IN('support','oppose','concern','mixed','neutral')),'[]'::jsonb)
 ELSE COALESCE(result#>'{semantic,stances}','[]'::jsonb) END
$$;
