-- T0.5: opportunity problem registry, taxonomy versions, lineage, and resolve_problem_id.
-- Source of truth remains 0177; this migration only creates linggan_opportunity_* objects.

CREATE TABLE linggan_opportunity_taxonomy_version (
 domain_ref uuid PRIMARY KEY REFERENCES observation_domain,
 version bigint NOT NULL CHECK (version > 0),
 updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE linggan_opportunity_problem (
 problem_ref uuid PRIMARY KEY,
 source_ref uuid NOT NULL UNIQUE,
 source_definition_ref uuid NOT NULL,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 label text NOT NULL CHECK (length(btrim(label)) BETWEEN 1 AND 120),
 level integer NOT NULL DEFAULT 1 CHECK (level >= 1),
 parent_id uuid REFERENCES linggan_opportunity_problem(problem_ref),
 def_version integer NOT NULL DEFAULT 1 CHECK (def_version > 0),
 created_at timestamptz NOT NULL DEFAULT now(),
 updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX linggan_opportunity_problem_domain ON linggan_opportunity_problem(domain_ref, problem_ref);
CREATE INDEX linggan_opportunity_problem_parent ON linggan_opportunity_problem(parent_id);

CREATE TABLE linggan_opportunity_problem_lineage (
 lineage_ref uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 operation_ref uuid NOT NULL,
 from_problem_ref uuid NOT NULL REFERENCES linggan_opportunity_problem(problem_ref),
 to_problem_ref uuid NOT NULL REFERENCES linggan_opportunity_problem(problem_ref),
 relation text NOT NULL CHECK (relation IN ('merge', 'rename', 'split')),
 created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE (operation_ref, from_problem_ref, to_problem_ref, relation),
 CHECK (from_problem_ref <> to_problem_ref)
);

CREATE INDEX linggan_opportunity_problem_lineage_from
 ON linggan_opportunity_problem_lineage(from_problem_ref, relation);

CREATE FUNCTION linggan_opportunity_problem_lineage_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE source_domain uuid;
DECLARE target_domain uuid;
BEGIN
 SELECT problem.domain_ref INTO source_domain
 FROM linggan_opportunity_problem problem
 WHERE problem.problem_ref = NEW.from_problem_ref;
 SELECT problem.domain_ref INTO target_domain
 FROM linggan_opportunity_problem problem
 WHERE problem.problem_ref = NEW.to_problem_ref;
 IF source_domain IS NULL OR target_domain IS NULL THEN
  RAISE EXCEPTION 'problem_lineage_endpoint_missing' USING ERRCODE = '23503';
 END IF;
 IF source_domain <> target_domain THEN
  RAISE EXCEPTION 'problem_lineage_cross_domain' USING ERRCODE = '23514';
 END IF;
 IF NEW.relation NOT IN ('merge', 'split') THEN
  RETURN NEW;
 END IF;
 IF EXISTS (
  SELECT 1 FROM linggan_opportunity_problem_lineage lineage
  WHERE lineage.operation_ref = NEW.operation_ref
    AND lineage.relation IN ('merge', 'split')
 ) THEN
  RETURN NEW;
 END IF;
 INSERT INTO linggan_opportunity_taxonomy_version(domain_ref, version)
 VALUES (source_domain, 2)
 ON CONFLICT (domain_ref) DO UPDATE
 SET version = linggan_opportunity_taxonomy_version.version + 1, updated_at = now();
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_problem_lineage_guard
BEFORE INSERT ON linggan_opportunity_problem_lineage
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_problem_lineage_guard();

CREATE FUNCTION linggan_opportunity_sync_problems()
RETURNS TABLE(synced bigint)
LANGUAGE plpgsql AS $$
DECLARE touched bigint;
BEGIN
 INSERT INTO linggan_opportunity_taxonomy_version(domain_ref, version)
 SELECT DISTINCT definition.domain_ref, 1
 FROM linggan_comment_problem_definition definition
 ON CONFLICT (domain_ref) DO NOTHING;

 INSERT INTO linggan_opportunity_problem(
  problem_ref, source_ref, source_definition_ref, domain_ref, label, level, parent_id, def_version
 )
 SELECT definition.problem_ref,
        definition.problem_ref,
        definition.definition_ref,
        definition.domain_ref,
        definition.label,
        1,
        NULL,
        1
 FROM linggan_comment_problem_definition definition
 ON CONFLICT (source_ref) DO UPDATE
 SET source_definition_ref = EXCLUDED.source_definition_ref,
     label = EXCLUDED.label,
     def_version = linggan_opportunity_problem.def_version + 1,
     updated_at = now()
 WHERE linggan_opportunity_problem.source_definition_ref
       IS DISTINCT FROM EXCLUDED.source_definition_ref;
 GET DIAGNOSTICS touched = ROW_COUNT;
 RETURN QUERY SELECT touched;
END $$;

SELECT linggan_opportunity_sync_problems();

CREATE FUNCTION linggan_opportunity_resolve_problem_id(p_problem_ref uuid)
RETURNS TABLE(problem_ref uuid, path uuid[])
LANGUAGE plpgsql STABLE AS $$
DECLARE current_ref uuid := p_problem_ref;
DECLARE current_path uuid[] := ARRAY[p_problem_ref];
DECLARE next_ref uuid;
DECLARE redirect_count integer;
DECLARE split_count integer;
BEGIN
 IF NOT EXISTS (
  SELECT 1 FROM linggan_opportunity_problem problem
  WHERE problem.problem_ref = p_problem_ref
 ) THEN
  RAISE EXCEPTION 'problem_not_found: %', p_problem_ref USING ERRCODE = '22023';
 END IF;

 LOOP
  SELECT count(*) INTO split_count
  FROM linggan_opportunity_problem_lineage lineage
  WHERE lineage.from_problem_ref = current_ref AND lineage.relation = 'split';

  IF split_count > 0 THEN
   IF EXISTS (
    SELECT 1 FROM linggan_opportunity_problem_lineage lineage
    WHERE lineage.from_problem_ref = current_ref
      AND lineage.relation IN ('merge', 'rename')
   ) THEN
    RAISE EXCEPTION 'problem_lineage_ambiguous: %', current_ref USING ERRCODE = '22023';
   END IF;
   RETURN QUERY
   SELECT lineage.to_problem_ref, current_path || lineage.to_problem_ref
   FROM linggan_opportunity_problem_lineage lineage
   WHERE lineage.from_problem_ref = current_ref AND lineage.relation = 'split'
   ORDER BY lineage.created_at, lineage.lineage_ref;
   RETURN;
  END IF;

  SELECT count(*) INTO redirect_count
  FROM linggan_opportunity_problem_lineage lineage
  WHERE lineage.from_problem_ref = current_ref
    AND lineage.relation IN ('merge', 'rename');

  IF redirect_count = 0 THEN
   RETURN QUERY SELECT current_ref, current_path;
   RETURN;
  END IF;
  IF redirect_count > 1 THEN
   RAISE EXCEPTION 'problem_lineage_ambiguous: %', current_ref USING ERRCODE = '22023';
  END IF;

  SELECT lineage.to_problem_ref INTO next_ref
  FROM linggan_opportunity_problem_lineage lineage
  WHERE lineage.from_problem_ref = current_ref
    AND lineage.relation IN ('merge', 'rename')
  LIMIT 1;

  IF next_ref = ANY(current_path) THEN
   RAISE EXCEPTION 'problem_lineage_cycle: %', current_ref USING ERRCODE = '22023';
  END IF;

  current_path := current_path || next_ref;
  current_ref := next_ref;
 END LOOP;
END $$;
