-- TOPIC-MAP-V41-001. References and decisions only; source text stays canonical.
CREATE TABLE linggan_topic_map_receipt (
 receipt_ref uuid PRIMARY KEY,idempotency_key text NOT NULL UNIQUE,request_sha256 text NOT NULL,
 action text NOT NULL,subject_ref uuid,revision integer NOT NULL DEFAULT 1,
 persisted_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_binding (
 binding_ref uuid PRIMARY KEY,topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 domain_ref uuid NOT NULL REFERENCES observation_domain,parent_topic_ref uuid REFERENCES linggan_topic_workspace,
 version integer NOT NULL CHECK(version>0),receipt_ref uuid NOT NULL REFERENCES linggan_topic_map_receipt,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),UNIQUE(topic_ref,version),CHECK(topic_ref IS DISTINCT FROM parent_topic_ref)
);
CREATE TABLE linggan_topic_map_own_creator (
 membership_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 platform text NOT NULL CHECK(platform IN('xhs','douyin')),author_external_id text NOT NULL CHECK(length(btrim(author_external_id)) BETWEEN 1 AND 256),
 active boolean NOT NULL,receipt_ref uuid NOT NULL REFERENCES linggan_topic_map_receipt,
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX topic_map_own_creator_current ON linggan_topic_map_own_creator(domain_ref,platform,author_external_id,created_at DESC);
CREATE TABLE linggan_topic_map_breakout (
 marking_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),marked boolean NOT NULL,
 receipt_ref uuid NOT NULL REFERENCES linggan_topic_map_receipt,created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_performance_rule (
 rule_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 platform text NOT NULL CHECK(platform IN('xhs','douyin')),like_threshold bigint NOT NULL CHECK(like_threshold>0),
 version integer NOT NULL CHECK(version>0),receipt_ref uuid NOT NULL REFERENCES linggan_topic_map_receipt,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),UNIQUE(domain_ref,platform,version)
);
CREATE TABLE linggan_topic_map_alternative (
 kind text NOT NULL DEFAULT 'angle' CHECK(kind IN('angle','product_research')),
 alternative_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 title text NOT NULL CHECK(length(btrim(title)) BETWEEN 1 AND 120),angle text NOT NULL CHECK(length(btrim(angle)) BETWEEN 1 AND 2000),
 rationale text NOT NULL CHECK(length(btrim(rationale)) BETWEEN 1 AND 2000),method_version text NOT NULL,
 research_manifest jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(research_manifest)='object'),
 receipt_ref uuid NOT NULL REFERENCES linggan_topic_map_receipt,created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_alternative_evidence (
 alternative_ref uuid NOT NULL REFERENCES linggan_topic_map_alternative,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),source_manifest jsonb NOT NULL CHECK(jsonb_typeof(source_manifest)='object'),PRIMARY KEY(alternative_ref,work_public_ref)
);
CREATE TABLE linggan_topic_map_work_annotation (
 annotation_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),topic_ref uuid REFERENCES linggan_topic_workspace,
 definition_ref uuid REFERENCES linggan_topic_definition,method_version text NOT NULL,
 main_stage text NOT NULL CHECK(main_stage IN('discover_understand','seek_assessment','choose_support','begin_practice','long_term_manage','cross_stage','general_background','unclear')),
 involved_stages text[] NOT NULL CHECK(involved_stages <@ ARRAY['discover_understand','seek_assessment','choose_support','begin_practice','long_term_manage']::text[]),
 overlays text[] NOT NULL CHECK(overlays <@ ARRAY['obstruction_recurrence','transition_handoff']::text[]),
 path text NOT NULL CHECK(path IN('family','adult','both','unknown')),rationale text NOT NULL,
 evidence_citations jsonb NOT NULL CHECK(jsonb_typeof(evidence_citations)='array' AND jsonb_array_length(evidence_citations)>0),
 receipt_ref uuid NOT NULL REFERENCES linggan_topic_map_receipt,created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK((topic_ref IS NULL)=(definition_ref IS NULL))
);
CREATE INDEX topic_map_annotation_current ON linggan_topic_map_work_annotation(domain_ref,work_public_ref,created_at DESC);
CREATE TABLE linggan_topic_map_viewed (
 view_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 receipt_ref uuid NOT NULL REFERENCES linggan_topic_map_receipt,observed_work_refs uuid[] NOT NULL DEFAULT '{}',created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
DO $$ DECLARE relation text; BEGIN
 FOREACH relation IN ARRAY ARRAY['linggan_topic_map_receipt','linggan_topic_map_binding','linggan_topic_map_own_creator','linggan_topic_map_breakout','linggan_topic_map_performance_rule','linggan_topic_map_alternative','linggan_topic_map_alternative_evidence','linggan_topic_map_work_annotation','linggan_topic_map_viewed'] LOOP
 EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation()',relation || '_append_only',relation);
 END LOOP;
END $$;
-- Machine proposals remain candidates; the human import contract retains its counterweight gate.
ALTER TABLE linggan_topic_definition DROP CONSTRAINT linggan_topic_definition_lifecycle_state_check;
ALTER TABLE linggan_topic_definition ADD CONSTRAINT linggan_topic_definition_lifecycle_state_check CHECK(lifecycle_state IN('provisional','candidate'));
ALTER TABLE linggan_topic_classification_run DROP CONSTRAINT linggan_topic_classification_run_run_kind_check;
ALTER TABLE linggan_topic_classification_run ADD CONSTRAINT linggan_topic_classification_run_run_kind_check CHECK(run_kind IN('human_adjudicated','machine_proposed'));
CREATE TABLE linggan_topic_map_membership (
 membership_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),method_version text NOT NULL,
 evidence_citations jsonb NOT NULL CHECK(jsonb_typeof(evidence_citations)='array' AND jsonb_array_length(evidence_citations)>0),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),UNIQUE(definition_ref,work_public_ref)
);
CREATE TRIGGER topic_map_membership_append_only BEFORE UPDATE OR DELETE ON linggan_topic_map_membership FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
