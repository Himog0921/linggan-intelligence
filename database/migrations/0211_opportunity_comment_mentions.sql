-- D-035: retain the existing UUID problem identity and make taxonomy changes time-addressable.
-- The existing per-domain row is the current pointer; this history is updated in the same
-- transaction. Only the current version can be recovered from a pre-existing pointer.
CREATE TABLE linggan_opportunity_taxonomy_version_history (
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 version bigint NOT NULL CHECK (version > 0),
 created_at timestamptz NOT NULL,
 PRIMARY KEY (domain_ref, version)
);

INSERT INTO linggan_opportunity_taxonomy_version_history(domain_ref, version, created_at)
SELECT domain_ref, version, updated_at FROM linggan_opportunity_taxonomy_version;

CREATE FUNCTION linggan_opportunity_record_taxonomy_version()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO linggan_opportunity_taxonomy_version_history(domain_ref, version, created_at)
 VALUES (NEW.domain_ref, NEW.version, NEW.updated_at);
 RETURN NEW;
END $$;

-- The old guard supplies now(), which is the transaction start time and can run backwards
-- when an older transaction acquires the version lock after a newer one has committed.
CREATE FUNCTION linggan_opportunity_timestamp_taxonomy_version()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE changed_at timestamptz := clock_timestamp();
BEGIN
 IF NEW.version <> OLD.version + 1 THEN
  RAISE EXCEPTION 'opportunity_taxonomy_version_invalid' USING ERRCODE = '23514';
 END IF;
 IF changed_at <= OLD.updated_at THEN
  RAISE EXCEPTION 'opportunity_taxonomy_clock_not_monotonic' USING ERRCODE = '22023';
 END IF;
 NEW.updated_at := changed_at;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_timestamp_taxonomy_version
BEFORE UPDATE ON linggan_opportunity_taxonomy_version
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_timestamp_taxonomy_version();

CREATE TRIGGER linggan_opportunity_record_taxonomy_version
AFTER INSERT OR UPDATE ON linggan_opportunity_taxonomy_version
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_record_taxonomy_version();

CREATE TRIGGER linggan_opportunity_taxonomy_version_history_immutable
BEFORE UPDATE OR DELETE ON linggan_opportunity_taxonomy_version_history
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE FUNCTION linggan_opportunity_taxonomy_version_at(p_domain_ref uuid, p_as_of timestamptz)
RETURNS bigint LANGUAGE plpgsql STABLE STRICT AS $$
DECLARE selected_version bigint;
BEGIN
 SELECT version INTO selected_version
 FROM linggan_opportunity_taxonomy_version_history
 WHERE domain_ref = p_domain_ref AND created_at <= p_as_of
 ORDER BY created_at DESC, version DESC
 LIMIT 1;
 IF selected_version IS NULL THEN
  RAISE EXCEPTION 'opportunity_taxonomy_version_unavailable_as_of' USING ERRCODE = '22023';
 END IF;
 RETURN selected_version;
END $$;

-- These rows are derived from 0177, never from a new model call. The source request and
-- ordinal are the actual composite membership key; their text form is a reversible reference.
CREATE TABLE linggan_opportunity_comment_mention (
 content_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 comment_external_id text NOT NULL,
 problem_ref uuid NOT NULL REFERENCES linggan_opportunity_problem(problem_ref),
 taxonomy_version bigint NOT NULL CHECK (taxonomy_version > 0),
 metric_def_version integer NOT NULL REFERENCES linggan_opportunity_metric_definition(version),
 commenter_hash text CHECK (commenter_hash IS NULL OR commenter_hash ~ '^[0-9a-f]{64}$'),
 commenter_digest_version text NOT NULL,
 source_material_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
 source_request_ref uuid NOT NULL,
 source_signal_ordinal integer NOT NULL,
 source_membership_ref text GENERATED ALWAYS AS
  (source_request_ref::text || ':' || source_signal_ordinal::text) STORED,
 assigned_by text NOT NULL CHECK (assigned_by IN ('ai', 'human', 'migrated')),
 prompt_version text NOT NULL CHECK (btrim(prompt_version) <> ''),
 model text NOT NULL CHECK (btrim(model) <> ''),
 content_hash text NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
 excluded_reason text CHECK (excluded_reason IN ('marketing', 'commenter_unknown')),
 created_at timestamptz NOT NULL DEFAULT now(),
 updated_at timestamptz NOT NULL DEFAULT now(),
 hash_changed_at timestamptz,
 PRIMARY KEY (content_ref, comment_external_id, problem_ref, taxonomy_version),
 FOREIGN KEY (source_request_ref, source_signal_ordinal)
  REFERENCES linggan_comment_problem_membership(request_ref, signal_ordinal),
 CHECK ((commenter_hash IS NULL) = (excluded_reason IS NOT DISTINCT FROM 'commenter_unknown'))
);

CREATE INDEX linggan_opportunity_comment_mention_problem
 ON linggan_opportunity_comment_mention(problem_ref, taxonomy_version);
