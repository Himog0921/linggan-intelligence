-- B3-MEDIA-SRC-002: CanonicalMediaSlot expand (immutable append-only)
-- Per design-freeze §5.12. Exactly 7 fields, no defaults beyond @id/@default(cuid()).

CREATE TABLE "CanonicalMediaSlot" (
    "id" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "canonicalObservationId" TEXT NOT NULL,
    "slotId" TEXT NOT NULL,
    "status" TEXT NOT NULL,
    "kind" TEXT NOT NULL,
    "ordinal" INTEGER NOT NULL,

    CONSTRAINT "CanonicalMediaSlot_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "CanonicalMediaSlot_workspace_observation_slot_key"
  ON "CanonicalMediaSlot"("workspaceId", "canonicalObservationId", "slotId");

CREATE UNIQUE INDEX "CanonicalMediaSlot_workspace_id_key"
  ON "CanonicalMediaSlot"("workspaceId", "id");

ALTER TABLE "CanonicalMediaSlot"
  ADD CONSTRAINT "CanonicalMediaSlot_workspace_observation_fkey"
  FOREIGN KEY ("workspaceId", "canonicalObservationId")
  REFERENCES "CanonicalObservation"("workspaceId", "id")
  ON DELETE RESTRICT
  ON UPDATE CASCADE;

CREATE OR REPLACE FUNCTION "reject_v2_canonical_media_mutation"()
RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION 'V2 Canonical media rows are append-only: %.% is not allowed', TG_TABLE_NAME, TG_OP
    USING ERRCODE = '55000';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER "CanonicalMediaSlot_append_only"
  BEFORE UPDATE OR DELETE ON "CanonicalMediaSlot"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_canonical_media_mutation"();
