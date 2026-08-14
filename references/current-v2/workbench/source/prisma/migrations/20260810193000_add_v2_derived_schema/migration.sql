-- V2 B2 / Derived schema expand only.
--
-- Adds the immutable normalization/evaluation records and their mutable
-- current selectors. No caller is connected and no existing row is changed.

CREATE TABLE "NormalizationRun" (
    "id" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "rawRecordId" TEXT NOT NULL,
    "adapterId" TEXT NOT NULL,
    "adapterVersion" TEXT NOT NULL,
    "canonicalSchemaVersion" TEXT NOT NULL,
    "attemptNumber" INTEGER NOT NULL DEFAULT 1,
    "status" TEXT NOT NULL,
    "inputPayloadHash" TEXT NOT NULL,
    "outputPayloadHash" TEXT,
    "missingFields" JSONB,
    "parseErrors" JSONB,
    "startedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "completedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "NormalizationRun_pkey" PRIMARY KEY ("id"),
    CONSTRAINT "NormalizationRun_status_check"
      CHECK ("status" IN ('normalized', 'rejected', 'quarantined'))
);

CREATE TABLE "NormalizationRunCurrent" (
    "workspaceId" TEXT NOT NULL,
    "rawRecordId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "normalizationRunId" TEXT NOT NULL,
    "adapterId" TEXT NOT NULL,
    "adapterVersion" TEXT NOT NULL,
    "canonicalSchemaVersion" TEXT NOT NULL,
    "updatedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "NormalizationRunCurrent_pkey" PRIMARY KEY ("workspaceId", "rawRecordId")
);

CREATE TABLE "CanonicalObservation" (
    "id" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "normalizationRunId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "rawRecordId" TEXT NOT NULL,
    "observationKind" TEXT NOT NULL,
    "subjectKey" TEXT NOT NULL,
    "observedAt" TIMESTAMP(3) NOT NULL,
    "schemaVersion" TEXT NOT NULL,
    "payload" JSONB NOT NULL,
    "payloadHash" TEXT NOT NULL,
    "qualityStatus" TEXT NOT NULL,
    "fieldPresence" JSONB,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "CanonicalObservation_pkey" PRIMARY KEY ("id")
);

CREATE TABLE "ContractEvaluation" (
    "id" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "contractId" TEXT NOT NULL,
    "contractVersion" INTEGER NOT NULL,
    "evaluatorVersion" TEXT NOT NULL,
    "canonicalSchemaVersion" TEXT NOT NULL,
    "evaluationInputHash" TEXT NOT NULL,
    "decision" TEXT NOT NULL,
    "completeness" TEXT NOT NULL,
    "rejectionCode" TEXT,
    "rejectionReason" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "ContractEvaluation_pkey" PRIMARY KEY ("id"),
    CONSTRAINT "ContractEvaluation_decision_completeness_check"
      CHECK (
        ("decision" = 'accepted' AND "completeness" IN ('full', 'partial'))
        OR ("decision" = 'rejected' AND "completeness" = 'not_applicable')
      )
);

CREATE TABLE "ContractEvaluationCurrent" (
    "workspaceId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "contractId" TEXT NOT NULL,
    "contractVersion" INTEGER NOT NULL,
    "contractEvaluationId" TEXT NOT NULL,
    "revision" INTEGER NOT NULL,
    "updatedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "ContractEvaluationCurrent_pkey" PRIMARY KEY ("workspaceId", "rawSnapshotId", "contractId", "contractVersion")
);

CREATE TABLE "ContractEvaluationNormalizationRun" (
    "id" TEXT NOT NULL,
    "evaluationId" TEXT NOT NULL,
    "normalizationRunId" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "status" TEXT NOT NULL,
    "inputPayloadHash" TEXT NOT NULL,
    "outputPayloadHash" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "ContractEvaluationNormalizationRun_pkey" PRIMARY KEY ("id")
);

CREATE TABLE "ContractEvaluationInput" (
    "id" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "contractEvaluationId" TEXT NOT NULL,
    "canonicalObservationId" TEXT NOT NULL,
    "ordinal" INTEGER NOT NULL,
    "canonicalOutputHash" TEXT NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "ContractEvaluationInput_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "NormalizationRun_record_adapter_attempt_key"
  ON "NormalizationRun"("rawRecordId", "adapterId", "adapterVersion", "canonicalSchemaVersion", "attemptNumber");
CREATE UNIQUE INDEX "NormalizationRun_workspace_id_key"
  ON "NormalizationRun"("workspaceId", "id");
CREATE UNIQUE INDEX "NormalizationRun_workspace_snapshot_id_key"
  ON "NormalizationRun"("workspaceId", "rawSnapshotId", "id");

CREATE UNIQUE INDEX "NormalizationRunCurrent_workspace_run_key"
  ON "NormalizationRunCurrent"("workspaceId", "normalizationRunId");
CREATE UNIQUE INDEX "NormalizationRunCurrent_workspace_snapshot_record_key"
  ON "NormalizationRunCurrent"("workspaceId", "rawSnapshotId", "rawRecordId");

CREATE UNIQUE INDEX "CanonicalObservation_workspace_id_key"
  ON "CanonicalObservation"("workspaceId", "id");
CREATE UNIQUE INDEX "CanonicalObservation_workspace_snapshot_id_key"
  ON "CanonicalObservation"("workspaceId", "rawSnapshotId", "id");

CREATE UNIQUE INDEX "ContractEvaluation_identity_key"
  ON "ContractEvaluation"("workspaceId", "rawSnapshotId", "contractId", "contractVersion", "evaluatorVersion", "canonicalSchemaVersion", "evaluationInputHash");
CREATE UNIQUE INDEX "ContractEvaluation_workspace_contract_id_key"
  ON "ContractEvaluation"("workspaceId", "id", "rawSnapshotId", "contractId", "contractVersion");
CREATE UNIQUE INDEX "ContractEvaluation_workspace_snapshot_id_key"
  ON "ContractEvaluation"("workspaceId", "rawSnapshotId", "id");

CREATE UNIQUE INDEX "ContractEvaluationNormalizationRun_evaluation_run_key"
  ON "ContractEvaluationNormalizationRun"("evaluationId", "normalizationRunId");
CREATE UNIQUE INDEX "ContractEvaluationNormalizationRun_workspace_id_key"
  ON "ContractEvaluationNormalizationRun"("workspaceId", "id");

CREATE UNIQUE INDEX "ContractEvaluationInput_evaluation_observation_key"
  ON "ContractEvaluationInput"("contractEvaluationId", "canonicalObservationId");
CREATE UNIQUE INDEX "ContractEvaluationInput_evaluation_ordinal_key"
  ON "ContractEvaluationInput"("contractEvaluationId", "ordinal");
CREATE UNIQUE INDEX "ContractEvaluationInput_workspace_id_key"
  ON "ContractEvaluationInput"("workspaceId", "id");

ALTER TABLE "NormalizationRun"
  ADD CONSTRAINT "NormalizationRun_workspace_snapshot_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId")
  REFERENCES "RawSnapshot"("workspaceId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "NormalizationRun"
  ADD CONSTRAINT "NormalizationRun_workspace_snapshot_record_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "rawRecordId")
  REFERENCES "RawRecord"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "NormalizationRunCurrent"
  ADD CONSTRAINT "NormalizationRunCurrent_workspace_snapshot_run_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "normalizationRunId")
  REFERENCES "NormalizationRun"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "NormalizationRunCurrent"
  ADD CONSTRAINT "NormalizationRunCurrent_workspace_snapshot_record_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "rawRecordId")
  REFERENCES "RawRecord"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "CanonicalObservation"
  ADD CONSTRAINT "CanonicalObservation_workspace_snapshot_run_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "normalizationRunId")
  REFERENCES "NormalizationRun"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "CanonicalObservation"
  ADD CONSTRAINT "CanonicalObservation_workspace_snapshot_record_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "rawRecordId")
  REFERENCES "RawRecord"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "ContractEvaluation"
  ADD CONSTRAINT "ContractEvaluation_workspace_snapshot_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId")
  REFERENCES "RawSnapshot"("workspaceId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "ContractEvaluationCurrent"
  ADD CONSTRAINT "ContractEvaluationCurrent_workspace_evaluation_fkey"
  FOREIGN KEY ("workspaceId", "contractEvaluationId", "rawSnapshotId", "contractId", "contractVersion")
  REFERENCES "ContractEvaluation"("workspaceId", "id", "rawSnapshotId", "contractId", "contractVersion")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "ContractEvaluationNormalizationRun"
  ADD CONSTRAINT "ContractEvaluationNormalizationRun_workspace_evaluation_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "evaluationId")
  REFERENCES "ContractEvaluation"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "ContractEvaluationNormalizationRun"
  ADD CONSTRAINT "ContractEvaluationNormalizationRun_workspace_run_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "normalizationRunId")
  REFERENCES "NormalizationRun"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "ContractEvaluationInput"
  ADD CONSTRAINT "ContractEvaluationInput_workspace_evaluation_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "contractEvaluationId")
  REFERENCES "ContractEvaluation"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "ContractEvaluationInput"
  ADD CONSTRAINT "ContractEvaluationInput_workspace_observation_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "canonicalObservationId")
  REFERENCES "CanonicalObservation"("workspaceId", "rawSnapshotId", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

CREATE OR REPLACE FUNCTION "reject_v2_derived_mutation"()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION 'V2 Derived rows are immutable: %.% is not allowed', TG_TABLE_NAME, TG_OP
    USING ERRCODE = '55000';
END;
$$;

CREATE TRIGGER "NormalizationRun_immutable"
  BEFORE UPDATE OR DELETE ON "NormalizationRun"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_derived_mutation"();
CREATE TRIGGER "CanonicalObservation_immutable"
  BEFORE UPDATE OR DELETE ON "CanonicalObservation"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_derived_mutation"();
CREATE TRIGGER "ContractEvaluation_immutable"
  BEFORE UPDATE OR DELETE ON "ContractEvaluation"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_derived_mutation"();
CREATE TRIGGER "ContractEvaluationNormalizationRun_immutable"
  BEFORE UPDATE OR DELETE ON "ContractEvaluationNormalizationRun"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_derived_mutation"();
CREATE TRIGGER "ContractEvaluationInput_immutable"
  BEFORE UPDATE OR DELETE ON "ContractEvaluationInput"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_derived_mutation"();

CREATE OR REPLACE FUNCTION "reject_v2_derived_current_delete"()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION 'V2 Derived Current rows cannot be deleted: %', TG_TABLE_NAME
    USING ERRCODE = '55000';
END;
$$;

CREATE TRIGGER "NormalizationRunCurrent_no_delete"
  BEFORE DELETE ON "NormalizationRunCurrent"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_derived_current_delete"();
CREATE TRIGGER "ContractEvaluationCurrent_no_delete"
  BEFORE DELETE ON "ContractEvaluationCurrent"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_derived_current_delete"();
