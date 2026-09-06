-- COMMENT-RESEARCH-001: research references do not copy source text.
CREATE TABLE linggan_comment_research_restriction (
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    comment_external_id text NOT NULL,
    restricted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    reason text NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 500),
    PRIMARY KEY(content_public_ref, comment_external_id)
);

CREATE TABLE linggan_comment_saved_query (
    query_ref uuid PRIMARY KEY,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    query_text text NOT NULL CHECK (char_length(query_text) <= 200),
    work_public_ref uuid REFERENCES linggan_material_content(public_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_comment_collection (
    collection_ref uuid PRIMARY KEY,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_comment_asset (
    asset_ref uuid PRIMARY KEY,
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    start_char integer NOT NULL CHECK(start_char >= 0),
    end_char integer NOT NULL CHECK(end_char > start_char),
    source_sha256 text NOT NULL CHECK(source_sha256 ~ '^[0-9a-f]{64}$'),
    reason text NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 1000),
    collection_ref uuid REFERENCES linggan_comment_collection(collection_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_comment_annotation (
    annotation_ref uuid PRIMARY KEY,
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    revision integer NOT NULL CHECK(revision > 0),
    facets jsonb NOT NULL CHECK(jsonb_typeof(facets)='array'),
    reason text NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 1000),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(source_ref,revision)
);

CREATE TABLE linggan_comment_analysis_work (
    work_ref uuid PRIMARY KEY,
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    rule_version text NOT NULL,
    model_version text NOT NULL,
    state text NOT NULL CHECK(state IN ('pending','running','succeeded','no_signal','failed')),
    lease_ref uuid,
    lease_until timestamptz,
    attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 3),
    failure_code text CHECK(failure_code IN ('provider_unavailable','provider_timeout','invalid_output','source_unavailable','lease_expired')),
    result jsonb,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(source_ref,rule_version,model_version),
    CHECK ((state='running')=(lease_ref IS NOT NULL AND lease_until IS NOT NULL)),
    CHECK ((state IN ('succeeded','no_signal'))=(result IS NOT NULL))
);
CREATE INDEX linggan_comment_analysis_pending_idx ON linggan_comment_analysis_work(state,created_at);

CREATE FUNCTION linggan_comment_research_current(as_of timestamptz)
RETURNS SETOF linggan_material_comment LANGUAGE sql STABLE AS $$
 SELECT DISTINCT ON (comment.content_public_ref,comment.comment_external_id) comment.*
 FROM linggan_material_comment comment
 JOIN linggan_runtime_capture_package package USING(package_ref)
 WHERE package.accepted_at <= as_of AND comment.created_at <= as_of
 ORDER BY comment.content_public_ref,comment.comment_external_id,
 comment.observed_at::timestamptz DESC,comment.created_at DESC,comment.material_ref DESC
$$;

CREATE VIEW linggan_comment_research_readable AS
SELECT comment.* FROM linggan_material_comment comment
JOIN linggan_runtime_capture_package package USING(package_ref)
WHERE package.accepted_at <= scope_001_now() AND NOT EXISTS (
 SELECT 1 FROM linggan_comment_research_restriction restriction
 WHERE restriction.content_public_ref=comment.content_public_ref
 AND restriction.comment_external_id=comment.comment_external_id
);

CREATE TRIGGER linggan_comment_asset_immutable BEFORE UPDATE OR DELETE ON linggan_comment_asset
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_comment_annotation_immutable BEFORE UPDATE OR DELETE ON linggan_comment_annotation
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- Original creation requests remain immutable; later edits never rewrite source references.
CREATE TABLE linggan_comment_asset_revision (
    revision_ref uuid PRIMARY KEY,
    asset_ref uuid NOT NULL REFERENCES linggan_comment_asset(asset_ref),
    revision integer NOT NULL CHECK(revision > 0),
    reason text CHECK(char_length(reason) BETWEEN 1 AND 1000),
    collection_ref uuid REFERENCES linggan_comment_collection(collection_ref),
    withdrawn boolean NOT NULL,
    change_reason text NOT NULL CHECK(char_length(change_reason) BETWEEN 1 AND 1000),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK(withdrawn OR reason IS NOT NULL),
    UNIQUE(asset_ref,revision)
);
CREATE TABLE linggan_comment_query_revision (
    revision_ref uuid PRIMARY KEY,
    query_ref uuid NOT NULL REFERENCES linggan_comment_saved_query(query_ref),
    revision integer NOT NULL CHECK(revision > 0),
    name text NOT NULL CHECK(char_length(name) BETWEEN 1 AND 100),
    query_text text NOT NULL CHECK(char_length(query_text) <= 200),
    work_public_ref uuid REFERENCES linggan_material_content(public_ref),
    deleted boolean NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(query_ref,revision)
);
CREATE VIEW linggan_comment_asset_current AS
SELECT asset.asset_ref,asset.source_ref,asset.start_char,asset.end_char,asset.source_sha256,asset.created_at,
 CASE WHEN edit.revision IS NULL THEN asset.reason ELSE edit.reason END AS reason,
 CASE WHEN edit.revision IS NULL THEN asset.collection_ref ELSE edit.collection_ref END AS collection_ref,
 COALESCE(edit.revision,0) AS revision,COALESCE(edit.withdrawn,false) AS withdrawn
FROM linggan_comment_asset asset LEFT JOIN LATERAL
 (SELECT * FROM linggan_comment_asset_revision WHERE asset_ref=asset.asset_ref ORDER BY revision DESC LIMIT 1) edit ON true;
CREATE VIEW linggan_comment_query_current AS
SELECT original.query_ref,original.created_at,COALESCE(edit.name,original.name) AS name,
 COALESCE(edit.query_text,original.query_text) AS query_text,
 CASE WHEN edit.revision IS NULL THEN original.work_public_ref ELSE edit.work_public_ref END AS work_public_ref,
 COALESCE(edit.revision,0) AS revision,COALESCE(edit.deleted,false) AS deleted
FROM linggan_comment_saved_query original LEFT JOIN LATERAL
 (SELECT * FROM linggan_comment_query_revision WHERE query_ref=original.query_ref ORDER BY revision DESC LIMIT 1) edit ON true;
CREATE TRIGGER linggan_comment_asset_revision_immutable BEFORE UPDATE OR DELETE ON linggan_comment_asset_revision
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_comment_saved_query_immutable BEFORE UPDATE OR DELETE ON linggan_comment_saved_query
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_comment_query_revision_immutable BEFORE UPDATE OR DELETE ON linggan_comment_query_revision
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
