-- B3-PROJECTION-FOUNDATION-001: Content-only dark projection foundation.
-- Pure expand: no caller, backfill, traffic switch, or legacy-row rewrite.

ALTER TABLE "CanonicalObservation"
  ADD CONSTRAINT "CanonicalObservation_workspace_snapshot_record_key"
  UNIQUE ("workspaceId", "rawSnapshotId", "id", "rawRecordId");

ALTER TABLE "ContractEvaluation"
  ADD CONSTRAINT "ContractEvaluation_workspace_id_key" UNIQUE ("workspaceId", "id");

CREATE TABLE "ContentObservation" (
  "id" TEXT NOT NULL,
  "workspaceId" TEXT NOT NULL,
  "contentAssetId" TEXT NOT NULL,
  "canonicalObservationId" TEXT NOT NULL,
  "rawSnapshotId" TEXT NOT NULL,
  "rawRecordId" TEXT NOT NULL,
  "observedAt" TIMESTAMP(3) NOT NULL,
  "contentType" TEXT,
  "title" TEXT,
  "bodyText" TEXT,
  "publishedAt" TIMESTAMP(3),
  "authorId" TEXT,
  "originalUrl" TEXT,
  "fieldPresence" JSONB,
  "qualityStatus" TEXT NOT NULL,
  "adapterVersion" TEXT NOT NULL,
  "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT "ContentObservation_pkey" PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "ContentObservation_workspace_id_key"
  ON "ContentObservation"("workspaceId", "id");
CREATE UNIQUE INDEX "ContentObservation_asset_id_key"
  ON "ContentObservation"("workspaceId", "contentAssetId", "id");
CREATE UNIQUE INDEX "ContentObservation_asset_canonical_key"
  ON "ContentObservation"("workspaceId", "contentAssetId", "canonicalObservationId");
ALTER TABLE "ContentObservation" ADD CONSTRAINT "ContentObservation_contentAsset_fkey"
  FOREIGN KEY ("workspaceId", "contentAssetId")
  REFERENCES "ContentAsset"("workspaceId", "id") ON DELETE RESTRICT ON UPDATE CASCADE;
ALTER TABLE "ContentObservation" ADD CONSTRAINT "ContentObservation_canonical_4col_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "canonicalObservationId", "rawRecordId")
  REFERENCES "CanonicalObservation"("workspaceId", "rawSnapshotId", "id", "rawRecordId")
  ON DELETE RESTRICT ON UPDATE CASCADE;

CREATE OR REPLACE FUNCTION "validate_content_observation_insert"() RETURNS trigger AS $$
DECLARE
  canonical_row "CanonicalObservation"%ROWTYPE;
  source_payload JSONB;
  run_adapter_version TEXT;
  asset_platform TEXT;
  asset_platform_content_id TEXT;
BEGIN
  SELECT * INTO canonical_row
  FROM "CanonicalObservation"
  WHERE "workspaceId" = NEW."workspaceId"
    AND "rawSnapshotId" = NEW."rawSnapshotId"
    AND "id" = NEW."canonicalObservationId"
    AND "rawRecordId" = NEW."rawRecordId";
  IF NOT FOUND THEN
    -- The composite FK supplies SQLSTATE 23503 for a missing provenance edge.
    RETURN NEW;
  END IF;

  SELECT "adapterVersion" INTO run_adapter_version
  FROM "NormalizationRun"
  WHERE "workspaceId" = canonical_row."workspaceId"
    AND "rawSnapshotId" = canonical_row."rawSnapshotId"
    AND "id" = canonical_row."normalizationRunId";
  SELECT "platform", "platformContentId"
    INTO asset_platform, asset_platform_content_id
  FROM "ContentAsset"
  WHERE "workspaceId" = NEW."workspaceId" AND "id" = NEW."contentAssetId";

  source_payload := canonical_row."payload"->'sourcePayload';
  IF canonical_row."observationKind" <> 'note'
     OR canonical_row."payload"->>'platform' <> 'xhs'
     OR canonical_row."payload"->>'recordKind' <> 'note'
     OR jsonb_typeof(source_payload) <> 'object'
     OR source_payload->>'noteId' IS NULL
     OR source_payload->>'noteId' IS DISTINCT FROM source_payload->>'platformContentId'
     OR canonical_row."subjectKey" IS DISTINCT FROM canonical_row."payload"->>'subjectKey'
     OR asset_platform IS DISTINCT FROM 'xhs'
     OR asset_platform_content_id IS DISTINCT FROM source_payload->>'platformContentId'
     OR NEW."observedAt" IS DISTINCT FROM canonical_row."observedAt"
     OR NEW."contentType" IS DISTINCT FROM source_payload->>'type'
     OR NEW."title" IS DISTINCT FROM source_payload->>'title'
     OR NEW."bodyText" IS DISTINCT FROM source_payload->>'content'
     OR NEW."publishedAt" IS NOT NULL
     OR NEW."authorId" IS NOT NULL
     OR NEW."originalUrl" IS DISTINCT FROM source_payload->>'url'
     OR NEW."fieldPresence" IS DISTINCT FROM canonical_row."fieldPresence"
     OR NEW."qualityStatus" IS DISTINCT FROM canonical_row."qualityStatus"
     OR NEW."adapterVersion" IS DISTINCT FROM run_adapter_version THEN
    RAISE EXCEPTION 'ContentObservation must be an exact materialization of its canonical input'
      USING ERRCODE = '55000';
  END IF;
  IF NEW."contentType" NOT IN ('normal', 'video') THEN
    RAISE EXCEPTION 'ContentObservation content type is not accepted' USING ERRCODE = '55000';
  END IF;
  IF NEW."originalUrl" IS NOT NULL
     AND (NEW."originalUrl" !~* '^https://([a-z0-9-]+\.)*(xiaohongshu\.com|xhslink\.com)(:[0-9]+)?([/?#]|$)'
          OR NEW."originalUrl" ~* '^https://[^/?#]*@') THEN
    RAISE EXCEPTION 'ContentObservation original URL is not an accepted XHS share link'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER "ContentObservation_canonical_guard"
  BEFORE INSERT ON "ContentObservation"
  FOR EACH ROW EXECUTE FUNCTION "validate_content_observation_insert"();

CREATE OR REPLACE FUNCTION "reject_content_observation_mutation"() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'ContentObservation is append-only: % is not allowed', TG_OP
    USING ERRCODE = '55000';
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER "ContentObservation_append_only"
  BEFORE UPDATE OR DELETE ON "ContentObservation"
  FOR EACH ROW EXECUTE FUNCTION "reject_content_observation_mutation"();

CREATE TABLE "ContentCurrentProjection" (
  "contentAssetId" TEXT NOT NULL,
  "workspaceId" TEXT NOT NULL,
  "currentObservationId" TEXT NOT NULL,
  "contractEvaluationId" TEXT NOT NULL,
  "title" TEXT,
  "bodyText" TEXT,
  "contentType" TEXT,
  "publishedAt" TIMESTAMP(3),
  "authorId" TEXT,
  "originalUrl" TEXT,
  "lastObservedAt" TIMESTAMP(3) NOT NULL,
  "projectionVersion" INTEGER NOT NULL DEFAULT 1,
  "lifecycleState" TEXT,
  "visibilityState" TEXT NOT NULL DEFAULT 'visible',
  "updatedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT "ContentCurrentProjection_pkey" PRIMARY KEY ("contentAssetId"),
  CONSTRAINT "CCP_version_check" CHECK ("projectionVersion" >= 1),
  CONSTRAINT "CCP_visibility_check" CHECK ("visibilityState" IN ('visible', 'quarantined')),
  CONSTRAINT "CCP_lifecycle_unknown_check" CHECK ("lifecycleState" IS NULL)
);
CREATE UNIQUE INDEX "CCP_workspace_asset_key"
  ON "ContentCurrentProjection"("workspaceId", "contentAssetId");
CREATE UNIQUE INDEX "CCP_workspace_asset_observation_key"
  ON "ContentCurrentProjection"("workspaceId", "contentAssetId", "currentObservationId");
CREATE INDEX "CCP_workspaceId_idx" ON "ContentCurrentProjection"("workspaceId");
ALTER TABLE "ContentCurrentProjection" ADD CONSTRAINT "CCP_contentAsset_fkey"
  FOREIGN KEY ("workspaceId", "contentAssetId")
  REFERENCES "ContentAsset"("workspaceId", "id") ON DELETE RESTRICT ON UPDATE CASCADE;
ALTER TABLE "ContentCurrentProjection" ADD CONSTRAINT "CCP_observation_fkey"
  FOREIGN KEY ("workspaceId", "contentAssetId", "currentObservationId")
  REFERENCES "ContentObservation"("workspaceId", "contentAssetId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;
ALTER TABLE "ContentCurrentProjection" ADD CONSTRAINT "CCP_evaluation_fkey"
  FOREIGN KEY ("workspaceId", "contractEvaluationId")
  REFERENCES "ContractEvaluation"("workspaceId", "id") ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "ContentMediaUsage" ADD COLUMN "contractEvaluationId" TEXT;
ALTER TABLE "ContentMediaUsage" ADD COLUMN "canonicalObservationId" TEXT;
ALTER TABLE "ContentMediaUsage" ADD COLUMN "canonicalSlotId" TEXT;
ALTER TABLE "ContentMediaUsage" ADD COLUMN "mediaOriginId" TEXT;
ALTER TABLE "ContentMediaUsage" ADD COLUMN "originGeneration" INTEGER;
ALTER TABLE "ContentMediaUsage" ADD COLUMN "mediaProcessingEventId" TEXT;
ALTER TABLE "ContentMediaUsage" ADD CONSTRAINT "CMU_provenance_check" CHECK (
  ("contractEvaluationId" IS NULL AND "canonicalObservationId" IS NULL AND
   "canonicalSlotId" IS NULL AND "mediaOriginId" IS NULL AND
   "originGeneration" IS NULL AND "mediaProcessingEventId" IS NULL)
  OR
  ("contractEvaluationId" IS NOT NULL AND "canonicalObservationId" IS NOT NULL AND
   "canonicalSlotId" IS NOT NULL AND "mediaOriginId" IS NOT NULL AND
   "originGeneration" IS NOT NULL AND "mediaProcessingEventId" IS NOT NULL)
);
ALTER TABLE "ContentMediaUsage" ADD CONSTRAINT "CMU_originGeneration_check"
  CHECK ("originGeneration" IS NULL OR "originGeneration" >= 1);
ALTER TABLE "ContentMediaUsage" ADD CONSTRAINT "CMU_evaluation_fkey"
  FOREIGN KEY ("workspaceId", "contractEvaluationId")
  REFERENCES "ContractEvaluation"("workspaceId", "id") ON DELETE RESTRICT ON UPDATE CASCADE;
ALTER TABLE "ContentMediaUsage" ADD CONSTRAINT "CMU_slot_fkey"
  FOREIGN KEY ("workspaceId", "canonicalObservationId", "canonicalSlotId")
  REFERENCES "CanonicalMediaSlot"("workspaceId", "canonicalObservationId", "slotId")
  ON DELETE RESTRICT ON UPDATE CASCADE;
ALTER TABLE "ContentMediaUsage" ADD CONSTRAINT "CMU_origin_fkey"
  FOREIGN KEY ("workspaceId", "mediaOriginId", "mediaItemId")
  REFERENCES "MediaOrigin"("workspaceId", "id", "mediaItemId")
  ON DELETE RESTRICT ON UPDATE CASCADE;
ALTER TABLE "ContentMediaUsage" ADD CONSTRAINT "CMU_processing_event_fkey"
  FOREIGN KEY ("mediaProcessingEventId") REFERENCES "OutboxEvent"("id")
  ON DELETE RESTRICT ON UPDATE CASCADE;
CREATE INDEX "CMU_canonicalObservationId_idx"
  ON "ContentMediaUsage"("canonicalObservationId");

CREATE OR REPLACE FUNCTION "validate_v2_content_media_usage"() RETURNS trigger AS $$
DECLARE
  event_row "OutboxEvent"%ROWTYPE;
  origin_generation INTEGER;
  slot_kind TEXT;
  slot_status TEXT;
  event_role TEXT;
  event_ordinal INTEGER;
BEGIN
  IF TG_OP = 'DELETE' THEN
    IF OLD."contractEvaluationId" IS NOT NULL THEN
      RAISE EXCEPTION 'V2 ContentMediaUsage is append-only' USING ERRCODE = '55000';
    END IF;
    RETURN OLD;
  END IF;

  IF OLD."contractEvaluationId" IS NOT NULL THEN
    IF NEW."id" IS DISTINCT FROM OLD."id"
       OR NEW."workspaceId" IS DISTINCT FROM OLD."workspaceId"
       OR NEW."contentAssetId" IS DISTINCT FROM OLD."contentAssetId"
       OR NEW."mediaItemId" IS DISTINCT FROM OLD."mediaItemId"
       OR NEW."purpose" IS DISTINCT FROM OLD."purpose"
       OR NEW."ordinal" IS DISTINCT FROM OLD."ordinal"
       OR NEW."contractEvaluationId" IS DISTINCT FROM OLD."contractEvaluationId"
       OR NEW."canonicalObservationId" IS DISTINCT FROM OLD."canonicalObservationId"
       OR NEW."canonicalSlotId" IS DISTINCT FROM OLD."canonicalSlotId"
       OR NEW."mediaOriginId" IS DISTINCT FROM OLD."mediaOriginId"
       OR NEW."originGeneration" IS DISTINCT FROM OLD."originGeneration"
       OR NEW."mediaProcessingEventId" IS DISTINCT FROM OLD."mediaProcessingEventId"
       OR NEW."sourceRevision" IS DISTINCT FROM OLD."sourceRevision"
       OR NEW."observedAt" IS DISTINCT FROM OLD."observedAt"
       OR NEW."validFrom" IS DISTINCT FROM OLD."validFrom"
       OR NEW."createdAt" IS DISTINCT FROM OLD."createdAt"
       OR (OLD."validTo" IS NOT NULL AND NEW."validTo" IS DISTINCT FROM OLD."validTo")
       OR (OLD."validTo" IS NULL AND NEW."validTo" IS NULL) THEN
      RAISE EXCEPTION 'V2 ContentMediaUsage mutation is not allowed' USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
  END IF;

  IF NEW."contractEvaluationId" IS NULL THEN
    RETURN NEW;
  END IF;
  IF TG_OP <> 'INSERT' OR NEW."validTo" IS NOT NULL THEN
    RAISE EXCEPTION 'V2 ContentMediaUsage can only be inserted active' USING ERRCODE = '55000';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM "ContractEvaluation" ce
    JOIN "ContractEvaluationInput" cei
      ON cei."workspaceId" = ce."workspaceId"
     AND cei."rawSnapshotId" = ce."rawSnapshotId"
     AND cei."contractEvaluationId" = ce."id"
    JOIN "ContentCurrentProjection" ccp
      ON ccp."workspaceId" = NEW."workspaceId"
     AND ccp."contentAssetId" = NEW."contentAssetId"
     AND ccp."contractEvaluationId" = ce."id"
     AND ccp."currentObservationId" IN (
       SELECT co."id" FROM "ContentObservation" co
       WHERE co."workspaceId" = NEW."workspaceId"
         AND co."contentAssetId" = NEW."contentAssetId"
         AND co."canonicalObservationId" = NEW."canonicalObservationId"
     )
    WHERE ce."workspaceId" = NEW."workspaceId"
      AND ce."id" = NEW."contractEvaluationId"
      AND ce."decision" = 'accepted'
      AND ce."completeness" IN ('full', 'partial')
      AND cei."canonicalObservationId" = NEW."canonicalObservationId"
      AND ccp."visibilityState" = 'visible'
  ) THEN
    RAISE EXCEPTION 'V2 media usage is not bound to the current accepted projection'
      USING ERRCODE = '55000';
  END IF;

  SELECT cms."status", cms."kind" INTO slot_status, slot_kind
  FROM "CanonicalMediaSlot" cms
  WHERE cms."workspaceId" = NEW."workspaceId"
    AND cms."canonicalObservationId" = NEW."canonicalObservationId"
    AND cms."slotId" = NEW."canonicalSlotId";
  IF NOT FOUND OR slot_status <> 'observed' THEN
    RAISE EXCEPTION 'V2 media usage requires an observed canonical slot'
      USING ERRCODE = '55000';
  END IF;

  SELECT mo."generation" INTO origin_generation
  FROM "MediaOrigin" mo
  WHERE mo."workspaceId" = NEW."workspaceId"
    AND mo."id" = NEW."mediaOriginId"
    AND mo."mediaItemId" = NEW."mediaItemId";
  IF NOT FOUND OR origin_generation <> NEW."originGeneration" THEN
    RAISE EXCEPTION 'V2 media usage origin generation mismatch'
      USING ERRCODE = '55000';
  END IF;

  SELECT * INTO event_row FROM "OutboxEvent" WHERE "id" = NEW."mediaProcessingEventId";
  IF NOT FOUND
     OR event_row."workspaceId" IS DISTINCT FROM NEW."workspaceId"
     OR event_row."eventType" <> 'media.processing_requested'
     OR event_row."aggregateType" <> 'MediaCandidate'
     OR event_row."payload"->>'workspaceId' IS DISTINCT FROM NEW."workspaceId"
     OR event_row."payload"->>'kind' IS DISTINCT FROM slot_kind
     OR event_row."payload"#>>'{ledgerOrigin,canonicalObservationId}' IS DISTINCT FROM NEW."canonicalObservationId"
     OR event_row."payload"#>>'{ledgerOrigin,slotId}' IS DISTINCT FROM NEW."canonicalSlotId"
     OR event_row."payload"#>>'{ledgerOrigin,originId}' IS DISTINCT FROM NEW."mediaOriginId"
     OR event_row."payload"#>>'{ledgerOrigin,mediaItemId}' IS DISTINCT FROM NEW."mediaItemId"
     OR event_row."payload"#>>'{ledgerOrigin,generation}' IS DISTINCT FROM NEW."originGeneration"::TEXT THEN
    RAISE EXCEPTION 'V2 media usage processing event proof mismatch'
      USING ERRCODE = '55000';
  END IF;

  event_role := event_row."payload"->>'role';
  IF event_role LIKE 'image:%' THEN
    event_ordinal := substring(event_role FROM 7)::INTEGER;
  ELSE
    event_ordinal := NULL;
  END IF;
  IF NOT (
    (event_role = 'cover' AND NEW."purpose" = 'source_cover' AND NEW."ordinal" IS NULL)
    OR (event_role = 'video' AND NEW."purpose" = 'source_video' AND NEW."ordinal" IS NULL)
    OR (event_role LIKE 'image:%' AND NEW."purpose" = 'source_image' AND NEW."ordinal" = event_ordinal)
  ) THEN
    RAISE EXCEPTION 'V2 media usage business slot does not match processing event'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER "ContentMediaUsage_v2_guard"
  BEFORE INSERT OR UPDATE OR DELETE ON "ContentMediaUsage"
  FOR EACH ROW EXECUTE FUNCTION "validate_v2_content_media_usage"();

CREATE OR REPLACE FUNCTION "protect_v2_media_processing_event_proof"() RETURNS trigger AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    IF EXISTS (
      SELECT 1 FROM "ContentMediaUsage"
      WHERE "mediaProcessingEventId" = OLD."id" AND "contractEvaluationId" IS NOT NULL
    ) THEN
      RAISE EXCEPTION 'Referenced V2 media processing proof is immutable'
        USING ERRCODE = '55000';
    END IF;
    RETURN OLD;
  END IF;
  IF EXISTS (
    SELECT 1 FROM "ContentMediaUsage"
    WHERE "mediaProcessingEventId" = OLD."id" AND "contractEvaluationId" IS NOT NULL
  ) THEN
    IF NEW."id" IS DISTINCT FROM OLD."id"
       OR NEW."workspaceId" IS DISTINCT FROM OLD."workspaceId"
       OR NEW."eventType" IS DISTINCT FROM OLD."eventType"
       OR NEW."aggregateType" IS DISTINCT FROM OLD."aggregateType"
       OR NEW."aggregateId" IS DISTINCT FROM OLD."aggregateId"
       OR NEW."payload" IS DISTINCT FROM OLD."payload" THEN
      RAISE EXCEPTION 'Referenced V2 media processing proof is immutable'
        USING ERRCODE = '55000';
    END IF;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER "OutboxEvent_v2_media_proof_guard"
  BEFORE UPDATE OR DELETE ON "OutboxEvent"
  FOR EACH ROW EXECUTE FUNCTION "protect_v2_media_processing_event_proof"();

-- Coordination-only row fence. Updating this row after the advisory lock turns
-- a stale SERIALIZABLE snapshot into a real write/write serialization failure.
CREATE TABLE "ContentProjectionSubjectFence" (
  "workspaceId" TEXT NOT NULL,
  "platformContentId" TEXT NOT NULL,
  "generation" BIGINT NOT NULL DEFAULT 0,
  "updatedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT "ContentProjectionSubjectFence_pkey"
    PRIMARY KEY ("workspaceId", "platformContentId")
);

CREATE OR REPLACE FUNCTION "lock_xhs_content_subject"(
  p_workspace_id TEXT,
  p_platform_content_id TEXT
) RETURNS INTEGER AS $$
BEGIN
  IF p_workspace_id IS NULL OR btrim(p_workspace_id) = ''
     OR p_platform_content_id IS NULL OR btrim(p_platform_content_id) = '' THEN
    RAISE EXCEPTION 'XHS content subject lock requires exact workspace and platform content identity'
      USING ERRCODE = '55000';
  END IF;
  PERFORM pg_advisory_xact_lock(hashtextextended(
    jsonb_build_array('b3-content', p_workspace_id, 'xhs', p_platform_content_id)::TEXT, 0));
  INSERT INTO "ContentProjectionSubjectFence" (
    "workspaceId", "platformContentId", "generation", "updatedAt"
  ) VALUES (p_workspace_id, p_platform_content_id, 1, CURRENT_TIMESTAMP)
  ON CONFLICT ("workspaceId", "platformContentId") DO UPDATE
    SET "generation" = "ContentProjectionSubjectFence"."generation" + 1,
        "updatedAt" = CURRENT_TIMESTAMP;
  RETURN 1;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "assert_visible_content_media_completeness"(
  p_workspace_id TEXT,
  p_content_asset_id TEXT
) RETURNS INTEGER AS $$
DECLARE
  projection RECORD;
  observed_slot_count INTEGER := 0;
  active_usage_count INTEGER := 0;
  matched_usage_count INTEGER := 0;
BEGIN
  SELECT ccp."contractEvaluationId", co."canonicalObservationId"
    INTO projection
  FROM "ContentCurrentProjection" ccp
  JOIN "ContentObservation" co
    ON co."workspaceId" = ccp."workspaceId"
   AND co."contentAssetId" = ccp."contentAssetId"
   AND co."id" = ccp."currentObservationId"
  WHERE ccp."workspaceId" = p_workspace_id
    AND ccp."contentAssetId" = p_content_asset_id
    AND ccp."visibilityState" = 'visible';
  IF NOT FOUND THEN RETURN 1; END IF;

  SELECT count(*) INTO observed_slot_count
  FROM "CanonicalMediaSlot" cms
  WHERE cms."workspaceId" = p_workspace_id
    AND cms."canonicalObservationId" = projection."canonicalObservationId"
    AND cms."status" = 'observed';

  SELECT count(*) INTO active_usage_count
  FROM "ContentMediaUsage" cmu
  WHERE cmu."workspaceId" = p_workspace_id
    AND cmu."contentAssetId" = p_content_asset_id
    AND cmu."contractEvaluationId" IS NOT NULL
    AND cmu."validTo" IS NULL;

  SELECT count(*) INTO matched_usage_count
  FROM "ContentMediaUsage" cmu
  JOIN "CanonicalMediaSlot" cms
    ON cms."workspaceId" = cmu."workspaceId"
   AND cms."canonicalObservationId" = cmu."canonicalObservationId"
   AND cms."slotId" = cmu."canonicalSlotId"
   AND cms."status" = 'observed'
  WHERE cmu."workspaceId" = p_workspace_id
    AND cmu."contentAssetId" = p_content_asset_id
    AND cmu."contractEvaluationId" = projection."contractEvaluationId"
    AND cmu."canonicalObservationId" = projection."canonicalObservationId"
    AND cmu."validTo" IS NULL;

  IF active_usage_count <> observed_slot_count
     OR matched_usage_count <> observed_slot_count
     OR EXISTS (
       SELECT 1
       FROM "ContentMediaUsage" cmu
       WHERE cmu."workspaceId" = p_workspace_id
         AND cmu."contentAssetId" = p_content_asset_id
         AND cmu."contractEvaluationId" = projection."contractEvaluationId"
         AND cmu."canonicalObservationId" = projection."canonicalObservationId"
         AND cmu."validTo" IS NULL
       GROUP BY cmu."canonicalSlotId"
       HAVING count(*) <> 1
     ) THEN
    RAISE EXCEPTION 'Visible projection media completeness invariant failed'
      USING ERRCODE = '55000';
  END IF;
  RETURN 1;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "enforce_visible_content_media_completeness"()
RETURNS trigger AS $$
DECLARE
  affected_workspace_id TEXT;
  affected_content_asset_id TEXT;
BEGIN
  affected_workspace_id := CASE WHEN TG_OP = 'DELETE' THEN OLD."workspaceId" ELSE NEW."workspaceId" END;
  affected_content_asset_id := CASE WHEN TG_OP = 'DELETE' THEN OLD."contentAssetId" ELSE NEW."contentAssetId" END;
  PERFORM "assert_visible_content_media_completeness"(
    affected_workspace_id,
    affected_content_asset_id
  );
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER "ContentCurrentProjection_media_completeness_guard"
  AFTER INSERT OR UPDATE ON "ContentCurrentProjection"
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION "enforce_visible_content_media_completeness"();
CREATE CONSTRAINT TRIGGER "ContentMediaUsage_projection_completeness_guard"
  AFTER INSERT OR UPDATE OR DELETE ON "ContentMediaUsage"
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION "enforce_visible_content_media_completeness"();

-- A late CanonicalMediaSlot is a new fact about the same XHS content subject.
-- Derive that subject only from the bound CanonicalObservation; callers cannot
-- choose the fence identity. Deferral permits Slot + Domain proof + Usage +
-- Current in one transaction, but rejects a visible Current with a new gap.
CREATE OR REPLACE FUNCTION "enforce_canonical_media_slot_projection_completeness"()
RETURNS trigger AS $$
DECLARE
  platform_content_id TEXT;
  affected_asset RECORD;
BEGIN
  SELECT co."payload"#>>'{sourcePayload,platformContentId}'
    INTO platform_content_id
  FROM "CanonicalObservation" co
  WHERE co."workspaceId" = NEW."workspaceId"
    AND co."id" = NEW."canonicalObservationId"
    AND co."observationKind" = 'note'
    AND co."payload"->>'platform' = 'xhs'
    AND co."payload"->>'recordKind' = 'note'
    AND co."payload"#>>'{sourcePayload,noteId}' = co."payload"#>>'{sourcePayload,platformContentId}'
    AND btrim(co."payload"#>>'{sourcePayload,platformContentId}') <> '';
  IF NOT FOUND THEN
    RAISE EXCEPTION 'CanonicalMediaSlot requires a database-proven XHS note subject'
      USING ERRCODE = '55000';
  END IF;

  PERFORM "lock_xhs_content_subject"(NEW."workspaceId", platform_content_id);
  FOR affected_asset IN
    SELECT ca."id"
    FROM "ContentAsset" ca
    WHERE ca."workspaceId" = NEW."workspaceId"
      AND ca."platform" = 'xhs'
      AND ca."platformContentId" = platform_content_id
  LOOP
    PERFORM "assert_visible_content_media_completeness"(
      NEW."workspaceId",
      affected_asset."id"
    );
  END LOOP;
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER "CanonicalMediaSlot_projection_completeness_guard"
  AFTER INSERT ON "CanonicalMediaSlot"
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION "enforce_canonical_media_slot_projection_completeness"();

CREATE OR REPLACE FUNCTION "validate_content_current_projection"() RETURNS trigger AS $$
DECLARE
  mode TEXT := current_setting('content_workbench.b3_projection_mode', true);
  obs "ContentObservation"%ROWTYPE;
BEGIN
  IF TG_OP = 'DELETE' THEN
    RAISE EXCEPTION 'ContentCurrentProjection DELETE is not allowed'
      USING ERRCODE = '55000';
  END IF;
  IF mode NOT IN ('advance', 'revoke') THEN
    RAISE EXCEPTION 'ContentCurrentProjection requires a controlled database function'
      USING ERRCODE = '55000';
  END IF;
  IF mode = 'revoke' THEN
    IF TG_OP <> 'UPDATE'
       OR OLD."visibilityState" <> 'visible'
       OR NEW."visibilityState" <> 'quarantined'
       OR NEW."contentAssetId" IS DISTINCT FROM OLD."contentAssetId"
       OR NEW."workspaceId" IS DISTINCT FROM OLD."workspaceId"
       OR NEW."currentObservationId" IS DISTINCT FROM OLD."currentObservationId"
       OR NEW."contractEvaluationId" IS DISTINCT FROM OLD."contractEvaluationId"
       OR NEW."title" IS DISTINCT FROM OLD."title"
       OR NEW."bodyText" IS DISTINCT FROM OLD."bodyText"
       OR NEW."contentType" IS DISTINCT FROM OLD."contentType"
       OR NEW."publishedAt" IS DISTINCT FROM OLD."publishedAt"
       OR NEW."authorId" IS DISTINCT FROM OLD."authorId"
       OR NEW."originalUrl" IS DISTINCT FROM OLD."originalUrl"
       OR NEW."lastObservedAt" IS DISTINCT FROM OLD."lastObservedAt"
       OR NEW."projectionVersion" IS DISTINCT FROM OLD."projectionVersion"
       OR NEW."lifecycleState" IS DISTINCT FROM OLD."lifecycleState" THEN
      RAISE EXCEPTION 'Invalid projection quarantine transition' USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
  END IF;

  SELECT * INTO obs FROM "ContentObservation"
  WHERE "workspaceId" = NEW."workspaceId"
    AND "contentAssetId" = NEW."contentAssetId"
    AND "id" = NEW."currentObservationId";
  IF NOT FOUND THEN
    RAISE EXCEPTION 'Projection observation is not bound to its content asset'
      USING ERRCODE = '55000';
  END IF;
  IF NEW."title" IS DISTINCT FROM obs."title"
     OR NEW."bodyText" IS DISTINCT FROM obs."bodyText"
     OR NEW."contentType" IS DISTINCT FROM obs."contentType"
     OR NEW."publishedAt" IS DISTINCT FROM obs."publishedAt"
     OR NEW."authorId" IS DISTINCT FROM obs."authorId"
     OR NEW."originalUrl" IS DISTINCT FROM obs."originalUrl"
     OR NEW."lastObservedAt" IS DISTINCT FROM obs."observedAt"
     OR NEW."lifecycleState" IS NOT NULL
     OR NEW."visibilityState" <> 'visible' THEN
    RAISE EXCEPTION 'Projection materialized fields must be copied from ContentObservation'
      USING ERRCODE = '55000';
  END IF;
  IF NOT EXISTS (
    SELECT 1
    FROM "ContractEvaluation" ce
    JOIN "ContractEvaluationInput" cei
      ON cei."workspaceId" = ce."workspaceId"
     AND cei."rawSnapshotId" = ce."rawSnapshotId"
     AND cei."contractEvaluationId" = ce."id"
    JOIN "ContractEvaluationCurrent" cec
      ON cec."workspaceId" = ce."workspaceId"
     AND cec."rawSnapshotId" = ce."rawSnapshotId"
     AND cec."contractId" = ce."contractId"
     AND cec."contractVersion" = ce."contractVersion"
     AND cec."contractEvaluationId" = ce."id"
    WHERE ce."workspaceId" = NEW."workspaceId"
      AND ce."id" = NEW."contractEvaluationId"
      AND ce."decision" = 'accepted'
      AND ce."completeness" IN ('full', 'partial')
      AND cei."canonicalObservationId" = obs."canonicalObservationId"
      AND 1 = (
        SELECT count(*)
        FROM "ContractEvaluationInput" note_cei
        JOIN "CanonicalObservation" note_co
          ON note_co."workspaceId" = note_cei."workspaceId"
         AND note_co."rawSnapshotId" = note_cei."rawSnapshotId"
         AND note_co."id" = note_cei."canonicalObservationId"
        WHERE note_cei."workspaceId" = ce."workspaceId"
          AND note_cei."rawSnapshotId" = ce."rawSnapshotId"
          AND note_cei."contractEvaluationId" = ce."id"
          AND note_co."observationKind" = 'note'
      )
  ) THEN
    RAISE EXCEPTION 'Projection evaluation is not the current accepted input for its observation'
      USING ERRCODE = '55000';
  END IF;

  IF TG_OP = 'INSERT' THEN
    IF NEW."projectionVersion" <> 1 THEN
      RAISE EXCEPTION 'Initial projection version must be 1' USING ERRCODE = '55000';
    END IF;
  ELSE
    IF OLD."visibilityState" = 'quarantined'
       AND NEW."contractEvaluationId" = OLD."contractEvaluationId" THEN
      RAISE EXCEPTION 'A quarantined projection requires a newer accepted evaluation'
        USING ERRCODE = '55000';
    END IF;
    IF NEW."currentObservationId" = OLD."currentObservationId" THEN
      IF NEW."projectionVersion" <> OLD."projectionVersion" THEN
        RAISE EXCEPTION 'Re-accepting the same observation must not increment version'
          USING ERRCODE = '55000';
      END IF;
    ELSE
      IF NEW."lastObservedAt" <= OLD."lastObservedAt"
         OR NEW."projectionVersion" <> OLD."projectionVersion" + 1 THEN
        RAISE EXCEPTION 'Projection observations must advance by strictly increasing observedAt'
          USING ERRCODE = '55000';
      END IF;
    END IF;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER "ContentCurrentProjection_controlled_writer"
  BEFORE INSERT OR UPDATE OR DELETE ON "ContentCurrentProjection"
  FOR EACH ROW EXECUTE FUNCTION "validate_content_current_projection"();

CREATE OR REPLACE FUNCTION "advance_content_current_projection"(
  p_workspace_id TEXT,
  p_content_asset_id TEXT,
  p_observation_id TEXT,
  p_evaluation_id TEXT
) RETURNS TABLE ("projectionVersion" INTEGER, "observationId" TEXT, "replayed" BOOLEAN) AS $$
DECLARE
  cur "ContentCurrentProjection"%ROWTYPE;
  has_current BOOLEAN;
  platform_content_id TEXT;
BEGIN
  SELECT "platformContentId" INTO platform_content_id
  FROM "ContentAsset"
  WHERE "workspaceId" = p_workspace_id
    AND "id" = p_content_asset_id
    AND "platform" = 'xhs';
  IF NOT FOUND THEN
    RAISE EXCEPTION 'Controlled projection advance requires an XHS ContentAsset'
      USING ERRCODE = '55000';
  END IF;
  PERFORM "lock_xhs_content_subject"(p_workspace_id, platform_content_id);
  SELECT * INTO cur FROM "ContentCurrentProjection"
  WHERE "workspaceId" = p_workspace_id AND "contentAssetId" = p_content_asset_id
  FOR UPDATE;
  has_current := FOUND;
  IF has_current AND cur."currentObservationId" = p_observation_id
           AND cur."contractEvaluationId" = p_evaluation_id
           AND cur."visibilityState" = 'visible' THEN
    RETURN QUERY SELECT cur."projectionVersion", cur."currentObservationId", TRUE;
    RETURN;
  END IF;

  PERFORM set_config('content_workbench.b3_projection_mode', 'advance', true);
  IF NOT has_current THEN
    INSERT INTO "ContentCurrentProjection" (
      "contentAssetId", "workspaceId", "currentObservationId", "contractEvaluationId",
      "title", "bodyText", "contentType", "publishedAt", "authorId", "originalUrl",
      "lastObservedAt", "projectionVersion", "lifecycleState", "visibilityState", "updatedAt"
    ) SELECT co."contentAssetId", co."workspaceId", co."id", p_evaluation_id,
             co."title", co."bodyText", co."contentType", co."publishedAt", co."authorId",
             co."originalUrl", co."observedAt", 1, NULL, 'visible', CURRENT_TIMESTAMP
      FROM "ContentObservation" co
      WHERE co."workspaceId" = p_workspace_id
        AND co."contentAssetId" = p_content_asset_id AND co."id" = p_observation_id;
    IF NOT FOUND THEN
      RAISE EXCEPTION 'ContentObservation not found for controlled projection advance'
        USING ERRCODE = '55000';
    END IF;
  ELSE
    UPDATE "ContentCurrentProjection" ccp SET
      "currentObservationId" = co."id",
      "contractEvaluationId" = p_evaluation_id,
      "title" = co."title", "bodyText" = co."bodyText", "contentType" = co."contentType",
      "publishedAt" = co."publishedAt", "authorId" = co."authorId",
      "originalUrl" = co."originalUrl", "lastObservedAt" = co."observedAt",
      "projectionVersion" = CASE WHEN cur."currentObservationId" = co."id"
                                 THEN cur."projectionVersion" ELSE cur."projectionVersion" + 1 END,
      "lifecycleState" = NULL, "visibilityState" = 'visible', "updatedAt" = CURRENT_TIMESTAMP
    FROM "ContentObservation" co
    WHERE ccp."workspaceId" = p_workspace_id
      AND ccp."contentAssetId" = p_content_asset_id
      AND co."workspaceId" = p_workspace_id
      AND co."contentAssetId" = p_content_asset_id
      AND co."id" = p_observation_id;
    IF NOT FOUND THEN
      RAISE EXCEPTION 'ContentObservation not found for controlled projection advance'
        USING ERRCODE = '55000';
    END IF;
  END IF;
  PERFORM set_config('content_workbench.b3_projection_mode', '', true);
  RETURN QUERY SELECT ccp."projectionVersion", ccp."currentObservationId", FALSE
    FROM "ContentCurrentProjection" ccp
    WHERE ccp."workspaceId" = p_workspace_id AND ccp."contentAssetId" = p_content_asset_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "revoke_content_projection_for_evaluation"(
  p_workspace_id TEXT,
  p_previous_evaluation_id TEXT,
  p_new_evaluation_id TEXT
) RETURNS INTEGER AS $$
DECLARE
  rec RECORD;
  new_decision TEXT;
  new_note_count INTEGER := 0;
  changed INTEGER := 0;
  row_count_changed INTEGER := 0;
BEGIN
  SELECT ce."decision",
         count(co."id") FILTER (WHERE co."observationKind" = 'note')
    INTO new_decision, new_note_count
  FROM "ContractEvaluation" ce
  LEFT JOIN "ContractEvaluationInput" cei
    ON cei."workspaceId" = ce."workspaceId"
   AND cei."rawSnapshotId" = ce."rawSnapshotId"
   AND cei."contractEvaluationId" = ce."id"
  LEFT JOIN "CanonicalObservation" co
    ON co."workspaceId" = cei."workspaceId"
   AND co."rawSnapshotId" = cei."rawSnapshotId"
   AND co."id" = cei."canonicalObservationId"
  WHERE ce."workspaceId" = p_workspace_id
    AND ce."id" = p_new_evaluation_id
  GROUP BY ce."decision";
  IF NOT FOUND THEN
    RAISE EXCEPTION 'Projection revocation requires an existing new evaluation'
      USING ERRCODE = '55000';
  END IF;

  IF new_decision = 'accepted' THEN
    -- Fence every accepted note subject even when Domain mutation is ineligible.
    FOR rec IN
      SELECT DISTINCT co."payload"#>>'{sourcePayload,platformContentId}' AS platform_content_id
      FROM "ContractEvaluationInput" cei
      JOIN "CanonicalObservation" co
        ON co."workspaceId" = cei."workspaceId"
       AND co."rawSnapshotId" = cei."rawSnapshotId"
       AND co."id" = cei."canonicalObservationId"
      WHERE cei."workspaceId" = p_workspace_id
        AND cei."contractEvaluationId" = p_new_evaluation_id
        AND co."observationKind" = 'note'
        AND co."payload"->>'platform' = 'xhs'
        AND co."payload"#>>'{sourcePayload,noteId}' = co."payload"#>>'{sourcePayload,platformContentId}'
    LOOP
      PERFORM "lock_xhs_content_subject"(p_workspace_id, rec.platform_content_id);
    END LOOP;
    IF new_note_count <> 1 THEN RETURN 0; END IF;

    FOR rec IN
      SELECT co."payload"#>>'{sourcePayload,platformContentId}' AS platform_content_id,
             co."id" AS new_canonical_observation_id,
             co."observedAt" AS new_observed_at,
             ca."id" AS content_asset_id,
             ccp."contractEvaluationId" AS current_evaluation_id,
             current_obs."canonicalObservationId" AS current_canonical_observation_id,
             current_obs."observedAt" AS current_observed_at
      FROM "ContractEvaluationInput" cei
      JOIN "CanonicalObservation" co
        ON co."workspaceId" = cei."workspaceId"
       AND co."rawSnapshotId" = cei."rawSnapshotId"
       AND co."id" = cei."canonicalObservationId"
      LEFT JOIN "ContentAsset" ca
        ON ca."workspaceId" = p_workspace_id
       AND ca."platform" = 'xhs'
       AND ca."platformContentId" = co."payload"#>>'{sourcePayload,platformContentId}'
      LEFT JOIN "ContentCurrentProjection" ccp
        ON ccp."workspaceId" = p_workspace_id
       AND ccp."contentAssetId" = ca."id"
       AND ccp."visibilityState" = 'visible'
      LEFT JOIN "ContentObservation" current_obs
        ON current_obs."workspaceId" = ccp."workspaceId"
       AND current_obs."contentAssetId" = ccp."contentAssetId"
       AND current_obs."id" = ccp."currentObservationId"
      WHERE cei."workspaceId" = p_workspace_id
        AND cei."contractEvaluationId" = p_new_evaluation_id
        AND co."observationKind" = 'note'
        AND co."payload"->>'platform' = 'xhs'
        AND co."payload"#>>'{sourcePayload,noteId}' = co."payload"#>>'{sourcePayload,platformContentId}'
    LOOP
      IF rec.content_asset_id IS NOT NULL
         AND rec.current_evaluation_id IS NOT NULL
         AND rec.current_evaluation_id IS DISTINCT FROM p_new_evaluation_id
         AND (rec.new_canonical_observation_id = rec.current_canonical_observation_id
              OR rec.new_observed_at > rec.current_observed_at) THEN
        PERFORM set_config('content_workbench.b3_projection_mode', 'revoke', true);
        UPDATE "ContentCurrentProjection"
          SET "visibilityState" = 'quarantined', "updatedAt" = CURRENT_TIMESTAMP
          WHERE "workspaceId" = p_workspace_id
            AND "contentAssetId" = rec.content_asset_id
            AND "contractEvaluationId" = rec.current_evaluation_id
            AND "visibilityState" = 'visible';
        GET DIAGNOSTICS row_count_changed = ROW_COUNT;
        changed := changed + row_count_changed;
        PERFORM set_config('content_workbench.b3_projection_mode', '', true);
        UPDATE "ContentMediaUsage"
          SET "validTo" = CURRENT_TIMESTAMP
          WHERE "workspaceId" = p_workspace_id
            AND "contentAssetId" = rec.content_asset_id
            AND "contractEvaluationId" = rec.current_evaluation_id
            AND "validTo" IS NULL;
      END IF;
    END LOOP;
  ELSIF new_decision = 'rejected' AND p_previous_evaluation_id IS NOT NULL THEN
    FOR rec IN
      SELECT DISTINCT co."payload"#>>'{sourcePayload,platformContentId}' AS platform_content_id,
             ca."id" AS content_asset_id,
             ccp."contractEvaluationId" AS current_evaluation_id
      FROM "ContractEvaluationInput" cei
      JOIN "CanonicalObservation" co
        ON co."workspaceId" = cei."workspaceId"
       AND co."rawSnapshotId" = cei."rawSnapshotId"
       AND co."id" = cei."canonicalObservationId"
      LEFT JOIN "ContentAsset" ca
        ON ca."workspaceId" = p_workspace_id
       AND ca."platform" = 'xhs'
       AND ca."platformContentId" = co."payload"#>>'{sourcePayload,platformContentId}'
      LEFT JOIN "ContentCurrentProjection" ccp
        ON ccp."workspaceId" = p_workspace_id
       AND ccp."contentAssetId" = ca."id"
       AND ccp."visibilityState" = 'visible'
      WHERE cei."workspaceId" = p_workspace_id
        AND cei."contractEvaluationId" = p_previous_evaluation_id
        AND co."observationKind" = 'note'
        AND co."payload"->>'platform' = 'xhs'
        AND co."payload"#>>'{sourcePayload,noteId}' = co."payload"#>>'{sourcePayload,platformContentId}'
    LOOP
      PERFORM "lock_xhs_content_subject"(p_workspace_id, rec.platform_content_id);
      IF rec.content_asset_id IS NOT NULL
         AND rec.current_evaluation_id = p_previous_evaluation_id THEN
      PERFORM set_config('content_workbench.b3_projection_mode', 'revoke', true);
      UPDATE "ContentCurrentProjection"
        SET "visibilityState" = 'quarantined', "updatedAt" = CURRENT_TIMESTAMP
        WHERE "workspaceId" = p_workspace_id
          AND "contentAssetId" = rec.content_asset_id
          AND "contractEvaluationId" = rec.current_evaluation_id
          AND "visibilityState" = 'visible';
      GET DIAGNOSTICS row_count_changed = ROW_COUNT;
      changed := changed + row_count_changed;
      PERFORM set_config('content_workbench.b3_projection_mode', '', true);
      UPDATE "ContentMediaUsage"
        SET "validTo" = CURRENT_TIMESTAMP
        WHERE "workspaceId" = p_workspace_id
          AND "contentAssetId" = rec.content_asset_id
          AND "contractEvaluationId" = rec.current_evaluation_id
          AND "validTo" IS NULL;
      END IF;
    END LOOP;
  END IF;
  RETURN changed;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION "enforce_cec_projection_revocation"() RETURNS trigger AS $$
DECLARE
  rec RECORD;
  new_decision TEXT;
  new_note_count INTEGER := 0;
  previous_evaluation_id TEXT := NULL;
BEGIN
  IF TG_OP = 'UPDATE' THEN previous_evaluation_id := OLD."contractEvaluationId"; END IF;
  SELECT ce."decision",
         count(co."id") FILTER (WHERE co."observationKind" = 'note')
    INTO new_decision, new_note_count
  FROM "ContractEvaluation" ce
  LEFT JOIN "ContractEvaluationInput" cei
    ON cei."workspaceId" = ce."workspaceId"
   AND cei."rawSnapshotId" = ce."rawSnapshotId"
   AND cei."contractEvaluationId" = ce."id"
  LEFT JOIN "CanonicalObservation" co
    ON co."workspaceId" = cei."workspaceId"
   AND co."rawSnapshotId" = cei."rawSnapshotId"
   AND co."id" = cei."canonicalObservationId"
  WHERE ce."workspaceId" = NEW."workspaceId"
    AND ce."id" = NEW."contractEvaluationId"
  GROUP BY ce."decision";

  IF new_decision = 'accepted' AND new_note_count = 1 THEN
    FOR rec IN
      SELECT ca."id" AS content_asset_id,
             ccp."contractEvaluationId" AS current_evaluation_id,
             current_obs."canonicalObservationId" AS current_canonical_observation_id,
             current_obs."observedAt" AS current_observed_at,
             co."id" AS new_canonical_observation_id,
             co."observedAt" AS new_observed_at
      FROM "ContractEvaluationInput" cei
      JOIN "CanonicalObservation" co
        ON co."workspaceId" = cei."workspaceId"
       AND co."rawSnapshotId" = cei."rawSnapshotId"
       AND co."id" = cei."canonicalObservationId"
      JOIN "ContentAsset" ca
        ON ca."workspaceId" = NEW."workspaceId"
       AND ca."platform" = 'xhs'
       AND ca."platformContentId" = co."payload"#>>'{sourcePayload,platformContentId}'
      JOIN "ContentCurrentProjection" ccp
        ON ccp."workspaceId" = NEW."workspaceId"
       AND ccp."contentAssetId" = ca."id"
      JOIN "ContentObservation" current_obs
        ON current_obs."workspaceId" = ccp."workspaceId"
       AND current_obs."contentAssetId" = ccp."contentAssetId"
       AND current_obs."id" = ccp."currentObservationId"
      WHERE cei."workspaceId" = NEW."workspaceId"
        AND cei."contractEvaluationId" = NEW."contractEvaluationId"
        AND co."observationKind" = 'note'
        AND co."payload"->>'platform' = 'xhs'
        AND co."payload"#>>'{sourcePayload,noteId}' = co."payload"#>>'{sourcePayload,platformContentId}'
    LOOP
      IF rec.current_evaluation_id IS DISTINCT FROM NEW."contractEvaluationId"
         AND (rec.new_canonical_observation_id = rec.current_canonical_observation_id
              OR rec.new_observed_at > rec.current_observed_at)
         AND (EXISTS (
           SELECT 1 FROM "ContentCurrentProjection" ccp
           WHERE ccp."workspaceId" = NEW."workspaceId"
             AND ccp."contentAssetId" = rec.content_asset_id
             AND ccp."contractEvaluationId" = rec.current_evaluation_id
             AND ccp."visibilityState" = 'visible'
         ) OR EXISTS (
           SELECT 1 FROM "ContentMediaUsage" cmu
           WHERE cmu."workspaceId" = NEW."workspaceId"
             AND cmu."contentAssetId" = rec.content_asset_id
             AND cmu."contractEvaluationId" = rec.current_evaluation_id
             AND cmu."validTo" IS NULL
         )) THEN
        RAISE EXCEPTION 'CEC advance omitted atomic Projection/Usage revocation'
          USING ERRCODE = '55000';
      END IF;
    END LOOP;
  ELSIF new_decision = 'rejected' AND previous_evaluation_id IS NOT NULL THEN
    IF EXISTS (
      SELECT 1
      FROM "ContentCurrentProjection" ccp
      WHERE ccp."workspaceId" = NEW."workspaceId"
        AND ccp."contractEvaluationId" = previous_evaluation_id
        AND (ccp."visibilityState" = 'visible' OR EXISTS (
          SELECT 1 FROM "ContentMediaUsage" cmu
          WHERE cmu."workspaceId" = ccp."workspaceId"
            AND cmu."contentAssetId" = ccp."contentAssetId"
            AND cmu."contractEvaluationId" = previous_evaluation_id
            AND cmu."validTo" IS NULL
        ))
    ) THEN
      RAISE EXCEPTION 'CEC advance omitted atomic Projection/Usage revocation'
        USING ERRCODE = '55000';
    END IF;
  END IF;
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;
CREATE CONSTRAINT TRIGGER "CEC_projection_revocation_guard"
  AFTER INSERT OR UPDATE OF "contractEvaluationId" ON "ContractEvaluationCurrent"
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION "enforce_cec_projection_revocation"();

CREATE OR REPLACE FUNCTION "enforce_quarantined_projection_consistency"() RETURNS trigger AS $$
DECLARE
  current_obs "ContentObservation"%ROWTYPE;
  platform_content_id TEXT;
  has_valid_supersede BOOLEAN := FALSE;
BEGIN
  IF NEW."visibilityState" <> 'quarantined' THEN RETURN NULL; END IF;
  IF EXISTS (
    SELECT 1 FROM "ContentMediaUsage"
    WHERE "workspaceId" = NEW."workspaceId"
      AND "contentAssetId" = NEW."contentAssetId"
      AND "contractEvaluationId" = NEW."contractEvaluationId"
      AND "validTo" IS NULL
  ) THEN
    RAISE EXCEPTION 'Quarantined Projection cannot retain active V2 usages'
      USING ERRCODE = '55000';
  END IF;

  SELECT * INTO current_obs FROM "ContentObservation"
  WHERE "workspaceId" = NEW."workspaceId"
    AND "contentAssetId" = NEW."contentAssetId"
    AND "id" = NEW."currentObservationId";
  SELECT "platformContentId" INTO platform_content_id FROM "ContentAsset"
  WHERE "workspaceId" = NEW."workspaceId"
    AND "id" = NEW."contentAssetId"
    AND "platform" = 'xhs';

  has_valid_supersede := EXISTS (
    SELECT 1
    FROM "ContractEvaluationCurrent" cec
    JOIN "ContractEvaluation" ce
      ON ce."workspaceId" = cec."workspaceId"
     AND ce."id" = cec."contractEvaluationId"
     AND ce."decision" = 'accepted'
     AND ce."completeness" IN ('full', 'partial')
    JOIN "ContractEvaluationInput" cei
      ON cei."workspaceId" = ce."workspaceId"
     AND cei."rawSnapshotId" = ce."rawSnapshotId"
     AND cei."contractEvaluationId" = ce."id"
    JOIN "CanonicalObservation" co
      ON co."workspaceId" = cei."workspaceId"
     AND co."rawSnapshotId" = cei."rawSnapshotId"
     AND co."id" = cei."canonicalObservationId"
    WHERE cec."workspaceId" = NEW."workspaceId"
      AND ce."id" IS DISTINCT FROM NEW."contractEvaluationId"
      AND co."observationKind" = 'note'
      AND co."payload"#>>'{sourcePayload,platformContentId}' = platform_content_id
      AND 1 = (
        SELECT count(*)
        FROM "ContractEvaluationInput" note_cei
        JOIN "CanonicalObservation" note_co
          ON note_co."workspaceId" = note_cei."workspaceId"
         AND note_co."rawSnapshotId" = note_cei."rawSnapshotId"
         AND note_co."id" = note_cei."canonicalObservationId"
        WHERE note_cei."workspaceId" = ce."workspaceId"
          AND note_cei."rawSnapshotId" = ce."rawSnapshotId"
          AND note_cei."contractEvaluationId" = ce."id"
          AND note_co."observationKind" = 'note'
      )
      AND (co."id" = current_obs."canonicalObservationId"
           OR co."observedAt" > current_obs."observedAt")
  ) OR EXISTS (
    SELECT 1
    FROM "ContractEvaluation" old_ce
    JOIN "ContractEvaluationCurrent" cec
      ON cec."workspaceId" = old_ce."workspaceId"
     AND cec."rawSnapshotId" = old_ce."rawSnapshotId"
     AND cec."contractId" = old_ce."contractId"
     AND cec."contractVersion" = old_ce."contractVersion"
    JOIN "ContractEvaluation" replacement
      ON replacement."workspaceId" = cec."workspaceId"
     AND replacement."id" = cec."contractEvaluationId"
     AND replacement."decision" = 'rejected'
    WHERE old_ce."workspaceId" = NEW."workspaceId"
      AND old_ce."id" = NEW."contractEvaluationId"
      AND cec."contractEvaluationId" IS DISTINCT FROM old_ce."id"
  );
  IF NOT has_valid_supersede THEN
    RAISE EXCEPTION 'Projection quarantine has no committed CEC supersede fact'
      USING ERRCODE = '55000';
  END IF;
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;
CREATE CONSTRAINT TRIGGER "ContentCurrentProjection_quarantine_guard"
  AFTER UPDATE OF "visibilityState" ON "ContentCurrentProjection"
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION "enforce_quarantined_projection_consistency"();
