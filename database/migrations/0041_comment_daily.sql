-- COMMENT-DAILY-001: derived cleaning, explicit daily grants and frozen research manifests.
CREATE TABLE linggan_comment_clean (
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    cleaner_version text NOT NULL,
    source_sha256 text NOT NULL,
    state text NOT NULL CHECK(state IN ('direct','context','low_information','anomaly')),
    result jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY(source_ref,cleaner_version)
);
CREATE TRIGGER linggan_comment_clean_immutable BEFORE UPDATE OR DELETE ON linggan_comment_clean
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_comment_daily_schedule (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    revision integer NOT NULL DEFAULT 0,
    enabled boolean NOT NULL DEFAULT false,
    config_ref uuid REFERENCES linggan_model_config,
    source_limit integer NOT NULL DEFAULT 100 CHECK(source_limit BETWEEN 1 AND 1000),
    token_limit bigint NOT NULL DEFAULT 100000 CHECK(token_limit BETWEEN 1024 AND 10000000),
    next_start timestamptz,
    next_end timestamptz,
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO linggan_comment_daily_schedule(singleton) VALUES(true);
CREATE TABLE linggan_comment_daily_batch (
    batch_ref uuid PRIMARY KEY,
    kind text NOT NULL CHECK(kind IN ('daily','selected')),
    config_ref uuid NOT NULL REFERENCES linggan_model_config,
    enabled boolean NOT NULL DEFAULT true,
    window_start timestamptz NOT NULL,
    window_end timestamptz NOT NULL,
    source_limit integer NOT NULL CHECK(source_limit BETWEEN 1 AND 1000),
    token_limit bigint NOT NULL CHECK(token_limit BETWEEN 1024 AND 10000000),
    request jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK(window_start<=window_end)
);
CREATE UNIQUE INDEX linggan_comment_daily_window ON linggan_comment_daily_batch(window_start,window_end) WHERE kind='daily';
CREATE TABLE linggan_comment_daily_item (
    batch_ref uuid NOT NULL REFERENCES linggan_comment_daily_batch,
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    state text NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','running','succeeded','no_signal','failed','low_information','anomaly','context_missing','restricted','source_limit')),
    attempts integer NOT NULL DEFAULT 0,
    failure_code text,
    analysis_ref uuid REFERENCES linggan_comment_analysis_work(work_ref),
    PRIMARY KEY(batch_ref,source_ref)
);
CREATE TABLE linggan_comment_daily_packet (
    packet_ref uuid PRIMARY KEY,
    batch_ref uuid NOT NULL REFERENCES linggan_comment_daily_batch,
    source_refs uuid[] NOT NULL,
    context_refs uuid[] NOT NULL,
    context_hash text NOT NULL,
    invocation_ref uuid NOT NULL UNIQUE REFERENCES linggan_model_invocation,
    lease_until timestamptz NOT NULL,
    state text NOT NULL CHECK(state IN ('running','succeeded','partial','failed')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz
);
CREATE INDEX linggan_comment_daily_pending ON linggan_comment_daily_item(batch_ref,state);
CREATE INDEX linggan_comment_daily_packet_batch ON linggan_comment_daily_packet(batch_ref);
-- Replaced scheduling authority: do not leave legacy automatic raw-comment work enabled.
UPDATE linggan_model_plan SET enabled=false,revision=revision+1 WHERE kind='automatic' AND enabled;
UPDATE linggan_model_workspace SET active_auto_plan_ref=NULL WHERE singleton;
CREATE TABLE linggan_comment_daily_command (
    command_ref uuid PRIMARY KEY,
    batch_ref uuid NOT NULL REFERENCES linggan_comment_daily_batch,
    result jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER linggan_comment_daily_command_immutable BEFORE UPDATE OR DELETE ON linggan_comment_daily_command
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
-- Freeze membership/configuration/provenance while allowing operational progress and pause.
CREATE FUNCTION linggan_comment_daily_freeze_manifest() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'research manifests are retained'; END IF;
    IF TG_TABLE_NAME='linggan_comment_daily_batch' AND (to_jsonb(OLD)-'enabled') IS DISTINCT FROM (to_jsonb(NEW)-'enabled') THEN
        RAISE EXCEPTION 'batch manifest is immutable';
    ELSIF TG_TABLE_NAME='linggan_comment_daily_packet' AND (to_jsonb(OLD)-ARRAY['state','lease_until','finished_at']) IS DISTINCT FROM (to_jsonb(NEW)-ARRAY['state','lease_until','finished_at']) THEN
        RAISE EXCEPTION 'packet manifest is immutable';
    ELSIF TG_TABLE_NAME='linggan_comment_daily_item' AND (to_jsonb(OLD)->'batch_ref',to_jsonb(OLD)->'source_ref') IS DISTINCT FROM (to_jsonb(NEW)->'batch_ref',to_jsonb(NEW)->'source_ref') THEN
        RAISE EXCEPTION 'batch membership is immutable';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER linggan_comment_daily_batch_frozen BEFORE UPDATE OR DELETE ON linggan_comment_daily_batch FOR EACH ROW EXECUTE FUNCTION linggan_comment_daily_freeze_manifest();
CREATE TRIGGER linggan_comment_daily_packet_frozen BEFORE UPDATE OR DELETE ON linggan_comment_daily_packet FOR EACH ROW EXECUTE FUNCTION linggan_comment_daily_freeze_manifest();
CREATE TRIGGER linggan_comment_daily_item_frozen BEFORE UPDATE OR DELETE ON linggan_comment_daily_item FOR EACH ROW EXECUTE FUNCTION linggan_comment_daily_freeze_manifest();
