-- T1.4: one versioned read model and a durable, per-problem debounce marker.
CREATE TABLE linggan_opportunity_opportunity_card (
 problem_ref uuid PRIMARY KEY REFERENCES linggan_opportunity_problem(problem_ref),
 payload jsonb NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
 as_of timestamptz NOT NULL,
 built_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 taxonomy_version bigint NOT NULL CHECK (taxonomy_version > 0),
 metric_def_version integer NOT NULL REFERENCES linggan_opportunity_metric_definition(version),
 input_fingerprint text NOT NULL CHECK (input_fingerprint ~ '^[0-9a-f]{64}$')
);

-- Reverse references contain only stable comment identities, never commenter identities or text.
CREATE TABLE linggan_opportunity_card_voice_ref (
 problem_ref uuid NOT NULL REFERENCES linggan_opportunity_opportunity_card(problem_ref) ON DELETE CASCADE,
 content_ref uuid NOT NULL,
 comment_external_id text NOT NULL CHECK (comment_external_id <> ''),
 PRIMARY KEY (problem_ref,content_ref,comment_external_id)
);
CREATE INDEX linggan_opportunity_card_voice_ref_source
 ON linggan_opportunity_card_voice_ref(content_ref,comment_external_id,problem_ref);

CREATE FUNCTION linggan_opportunity_card_index_voices()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 DELETE FROM linggan_opportunity_card_voice_ref WHERE problem_ref=NEW.problem_ref;
 INSERT INTO linggan_opportunity_card_voice_ref(problem_ref,content_ref,comment_external_id)
 SELECT NEW.problem_ref,(voice#>>'{commentRef,contentRef}')::uuid,
        voice#>>'{commentRef,commentExternalId}'
 FROM jsonb_array_elements(COALESCE(NEW.payload#>'{demand,sampleVoices}','[]'::jsonb)) voice;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_index_voices
AFTER INSERT OR UPDATE OF payload ON linggan_opportunity_opportunity_card
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_index_voices();

CREATE TABLE linggan_opportunity_card_change (
 problem_ref uuid PRIMARY KEY REFERENCES linggan_opportunity_problem(problem_ref) ON DELETE CASCADE,
 changed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0)
);

CREATE INDEX linggan_opportunity_card_change_due
 ON linggan_opportunity_card_change(changed_at, problem_ref);

-- Material and profile writers only append to this outbox. They never wait for a card
-- rebuild or read opportunity metrics/mentions while admitting material. Restriction
-- writers additionally remove exposed text synchronously, as required for withdrawal.
-- The existing worker resolves these events to affected problems in a separate transaction.
CREATE TABLE linggan_opportunity_card_source_event (
 event_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 event_kind text NOT NULL CHECK (event_kind IN ('content','domain','creator','target')),
 event_key text NOT NULL CHECK (event_key <> ''),
 changed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 CHECK (event_kind='creator' OR event_key ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
);

-- The environment flag remains authoritative. An enabled, live executor holds this
-- schema-specific session lock. Offline writers take a nonblocking shared transaction
-- lock and skip the outbox; the next executor catches up every problem after acquiring it.
-- Resolve the schema containing these functions, even when a caller puts pg_temp first.
CREATE FUNCTION linggan_opportunity_card_namespace()
RETURNS text LANGUAGE sql STABLE AS $$
 SELECT namespace.nspname::text
 FROM pg_catalog.pg_proc function_def
 JOIN pg_catalog.pg_namespace namespace ON namespace.oid=function_def.pronamespace
 WHERE function_def.oid='linggan_opportunity_card_namespace()'::regprocedure
$$;

CREATE FUNCTION linggan_opportunity_card_event_gate_key()
RETURNS bigint LANGUAGE sql STABLE AS $$
 SELECT hashtextextended('linggan:opportunity-card:source-events:'||linggan_opportunity_card_namespace(),0)
$$;

CREATE FUNCTION linggan_opportunity_card_source_changed(p_kind text, p_key text)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
 IF pg_try_advisory_xact_lock_shared(linggan_opportunity_card_event_gate_key()) THEN
  RETURN;
 END IF;
 INSERT INTO linggan_opportunity_card_source_event(event_kind,event_key) VALUES (p_kind,p_key);
END $$;

CREATE FUNCTION linggan_opportunity_card_mark_changed(p_problem_ref uuid)
RETURNS void LANGUAGE sql AS $$
 INSERT INTO linggan_opportunity_card_change(problem_ref, changed_at)
 VALUES (p_problem_ref, clock_timestamp())
 ON CONFLICT (problem_ref) DO UPDATE
 SET changed_at = EXCLUDED.changed_at,
     revision = linggan_opportunity_card_change.revision + 1
$$;

-- These helpers only mark already-built or pending cards. They do not create collection work,
-- and a disabled installation with no card work has no additional derived writes.
CREATE FUNCTION linggan_opportunity_card_mark_domain(p_domain_ref uuid)
RETURNS void LANGUAGE sql AS $$
 INSERT INTO linggan_opportunity_card_change(problem_ref, changed_at)
 SELECT problem.problem_ref, clock_timestamp() FROM linggan_opportunity_problem problem
 WHERE problem.domain_ref=p_domain_ref AND (
  EXISTS(SELECT 1 FROM linggan_opportunity_opportunity_card card WHERE card.problem_ref=problem.problem_ref)
  OR EXISTS(SELECT 1 FROM linggan_opportunity_card_change pending WHERE pending.problem_ref=problem.problem_ref)
 ) ORDER BY problem.problem_ref
 ON CONFLICT(problem_ref) DO UPDATE SET changed_at=EXCLUDED.changed_at,
  revision=linggan_opportunity_card_change.revision+1
$$;

CREATE FUNCTION linggan_opportunity_card_metric_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP = 'DELETE' THEN
  PERFORM linggan_opportunity_card_mark_changed(OLD.problem_ref);
  RETURN OLD;
 END IF;
 PERFORM linggan_opportunity_card_mark_changed(NEW.problem_ref);
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_period_changed
AFTER INSERT OR DELETE OR UPDATE OF k, n, ci_low, ci_high, baseline_k, baseline_n,
 no_baseline, mentioners_total, evidence_tier, input_fingerprint
ON linggan_opportunity_problem_metric_period
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_metric_changed();

CREATE TRIGGER linggan_opportunity_card_creator_changed
AFTER INSERT OR DELETE OR UPDATE OF k, n, ci_low, ci_high, sufficient, input_fingerprint
ON linggan_opportunity_creator_problem_density
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_metric_changed();

CREATE TRIGGER linggan_opportunity_card_problem_changed
AFTER INSERT OR UPDATE OF label, level, parent_id, def_version
ON linggan_opportunity_problem
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_metric_changed();

CREATE TRIGGER linggan_opportunity_card_mention_changed
AFTER INSERT OR DELETE OR UPDATE OF commenter_hash, excluded_reason, source_material_ref
ON linggan_opportunity_comment_mention
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_metric_changed();

CREATE FUNCTION linggan_opportunity_card_definition_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO linggan_opportunity_card_change(problem_ref,changed_at)
 SELECT problem_ref,clock_timestamp() FROM linggan_opportunity_problem WHERE true ORDER BY problem_ref
 ON CONFLICT(problem_ref) DO UPDATE SET changed_at=EXCLUDED.changed_at,
  revision=linggan_opportunity_card_change.revision+1;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_definition_changed
AFTER INSERT OR UPDATE OF version ON linggan_opportunity_metric_definition_current
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_definition_changed();

CREATE FUNCTION linggan_opportunity_card_taxonomy_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM linggan_opportunity_card_mark_domain(NEW.domain_ref);
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_taxonomy_changed
AFTER INSERT OR UPDATE OF version ON linggan_opportunity_taxonomy_version
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_taxonomy_changed();

CREATE FUNCTION linggan_opportunity_card_comment_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM linggan_opportunity_card_source_changed('content',NEW.content_public_ref::text);
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_comment_changed
AFTER INSERT ON linggan_material_comment
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_comment_changed();

CREATE FUNCTION linggan_opportunity_card_likes_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE source_content uuid;
BEGIN
 SELECT content_public_ref INTO source_content
 FROM linggan_material_comment WHERE material_ref=NEW.material_ref;
 IF source_content IS NOT NULL THEN
  PERFORM linggan_opportunity_card_source_changed('content',source_content::text);
 END IF;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_likes_changed
AFTER INSERT ON linggan_material_comment_engagement
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_likes_changed();

CREATE FUNCTION linggan_opportunity_card_clean_cache_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE source_content uuid;
BEGIN
 SELECT content_public_ref INTO source_content
 FROM linggan_material_comment WHERE material_ref=NEW.source_ref;
 IF source_content IS NOT NULL THEN
  PERFORM linggan_opportunity_card_source_changed('content',source_content::text);
 END IF;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_clean_cache_changed
AFTER INSERT ON linggan_comment_clean_cache
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_clean_cache_changed();

CREATE FUNCTION linggan_opportunity_card_detail_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM linggan_opportunity_card_source_changed('content',NEW.content_public_ref::text);
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_detail_changed
AFTER INSERT ON linggan_material_content_detail
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_detail_changed();

CREATE TRIGGER linggan_opportunity_card_discovery_changed
AFTER INSERT ON linggan_material_discovery_finding
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_detail_changed();

CREATE FUNCTION linggan_opportunity_card_usage_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP <> 'INSERT' THEN PERFORM linggan_opportunity_card_source_changed('domain',OLD.domain_ref::text); END IF;
 IF TG_OP <> 'DELETE' THEN PERFORM linggan_opportunity_card_source_changed('domain',NEW.domain_ref::text); END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_usage_changed
AFTER INSERT OR UPDATE OR DELETE ON linggan_material_domain_usage
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_usage_changed();

CREATE FUNCTION linggan_opportunity_card_profile_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.platform='xhs' THEN
  PERFORM linggan_opportunity_card_source_changed('creator',NEW.author_external_id);
 END IF;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_profile_changed
AFTER INSERT ON linggan_material_author_profile
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_profile_changed();

CREATE FUNCTION linggan_opportunity_card_observation_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP <> 'INSERT' THEN PERFORM linggan_opportunity_card_source_changed('domain',OLD.domain_ref::text); END IF;
 IF TG_OP <> 'DELETE' THEN PERFORM linggan_opportunity_card_source_changed('domain',NEW.domain_ref::text); END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_observation_changed
AFTER INSERT OR UPDATE OR DELETE ON observation_domain_target
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_observation_changed();

CREATE FUNCTION linggan_opportunity_card_target_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM linggan_opportunity_card_source_changed('target',NEW.target_ref::text);
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_target_changed
AFTER UPDATE OF identity_key, target_kind, purpose_kind, lifecycle_state, first_stored_at
ON collection_observation_target
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_target_changed();

-- Serialize only affected domains. The rebuilding session takes the shared lock before
-- its first transaction snapshot; a withdrawal takes the exclusive transaction lock.
-- This preserves withdrawal safety without locking unrelated domains' source writes.
CREATE INDEX linggan_opportunity_card_usage_content
 ON linggan_material_domain_usage(content_public_ref,domain_ref);

CREATE FUNCTION linggan_opportunity_card_restriction_lock_key(p_domain_ref uuid)
RETURNS bigint LANGUAGE sql STABLE STRICT AS $$
 SELECT hashtextextended('linggan:opportunity-card:restriction:'||linggan_opportunity_card_namespace()||':'||p_domain_ref::text,0)
$$;

CREATE FUNCTION linggan_opportunity_card_restriction_gate()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE affected_contents uuid[];
DECLARE affected_domain uuid;
BEGIN
 IF TG_OP='INSERT' THEN affected_contents:=ARRAY[NEW.content_public_ref];
 ELSIF TG_OP='DELETE' THEN affected_contents:=ARRAY[OLD.content_public_ref];
 ELSE affected_contents:=ARRAY[OLD.content_public_ref,NEW.content_public_ref]; END IF;
 FOR affected_domain IN
  SELECT usage.domain_ref FROM linggan_material_domain_usage usage
   WHERE usage.content_public_ref=ANY(affected_contents)
  UNION
  SELECT problem.domain_ref FROM linggan_opportunity_card_voice_ref voice
   JOIN linggan_opportunity_problem problem USING(problem_ref)
   WHERE voice.content_ref=ANY(affected_contents)
  ORDER BY 1
 LOOP
  PERFORM pg_advisory_xact_lock(linggan_opportunity_card_restriction_lock_key(affected_domain));
 END LOOP;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_restriction_gate
BEFORE INSERT OR UPDATE OR DELETE ON linggan_material_comment_restriction
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_restriction_gate();

-- Remove only cards that actually reference the restricted comment. The reverse index
-- keeps source text out of this lookup and is maintained atomically with each payload.
CREATE FUNCTION linggan_opportunity_card_restriction_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE affected_content uuid;
DECLARE affected_comment text;
BEGIN
 IF TG_OP = 'DELETE' THEN
  affected_content := OLD.content_public_ref;
  affected_comment := OLD.comment_external_id;
 ELSE
  affected_content := NEW.content_public_ref;
  affected_comment := NEW.comment_external_id;
 END IF;
 IF TG_OP <> 'DELETE' THEN
  WITH removed AS (
   DELETE FROM linggan_opportunity_opportunity_card card
   USING linggan_opportunity_card_voice_ref voice
   WHERE voice.problem_ref=card.problem_ref AND voice.content_ref=affected_content
     AND voice.comment_external_id=affected_comment
   RETURNING card.problem_ref
  )
  INSERT INTO linggan_opportunity_card_change(problem_ref, changed_at)
  SELECT problem_ref, clock_timestamp() FROM removed WHERE true ORDER BY problem_ref
  ON CONFLICT (problem_ref) DO UPDATE
  SET changed_at = EXCLUDED.changed_at,
      revision = linggan_opportunity_card_change.revision + 1;
 END IF;
 PERFORM linggan_opportunity_card_source_changed('content',affected_content::text);
 IF TG_OP='UPDATE' AND OLD.content_public_ref IS DISTINCT FROM NEW.content_public_ref THEN
  PERFORM linggan_opportunity_card_source_changed('content',OLD.content_public_ref::text);
 END IF;
 IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_card_restriction_changed
AFTER INSERT OR UPDATE OR DELETE ON linggan_material_comment_restriction
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_card_restriction_changed();

INSERT INTO linggan_opportunity_card_change(problem_ref)
SELECT problem_ref FROM linggan_opportunity_problem WHERE true ORDER BY problem_ref ON CONFLICT(problem_ref) DO NOTHING;
