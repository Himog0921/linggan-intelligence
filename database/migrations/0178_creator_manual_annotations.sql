-- Keep human annotations and display thresholds separate from AI execution state.
CREATE TABLE linggan_creator_work_override (
 domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 manual_overrides jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(manual_overrides)='object'),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(domain_ref,work_public_ref)
);
CREATE TABLE linggan_creator_author_override (
 domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 platform text NOT NULL,author_external_id text NOT NULL,
 manual_overrides jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(manual_overrides)='object'),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(domain_ref,platform,author_external_id)
);
CREATE TABLE linggan_creator_discovery_threshold (
 domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),platform text NOT NULL,
 like_threshold bigint CHECK(like_threshold>0),revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),PRIMARY KEY(domain_ref,platform)
);
INSERT INTO linggan_creator_work_override SELECT domain_ref,work_public_ref,manual_overrides,updated_at FROM linggan_creator_discovery_work_analysis WHERE manual_overrides<>'{}'::jsonb;
INSERT INTO linggan_creator_author_override SELECT domain_ref,platform,author_external_id,manual_overrides,updated_at FROM linggan_creator_discovery_author_analysis WHERE manual_overrides<>'{}'::jsonb;
INSERT INTO linggan_creator_discovery_threshold SELECT domain_ref,platform,like_threshold,revision,updated_at FROM linggan_creator_discovery_policy;
DROP TABLE linggan_creator_discovery_work_analysis RESTRICT;
DROP TABLE linggan_creator_discovery_author_analysis RESTRICT;
DROP TABLE linggan_creator_discovery_policy RESTRICT;
