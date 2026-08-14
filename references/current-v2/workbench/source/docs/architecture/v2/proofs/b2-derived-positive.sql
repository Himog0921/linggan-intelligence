INSERT INTO "RawSnapshot"
  ("id", "workspaceId", "captureId", "platform", "targetKey", "observedAt")
VALUES
  ('snapshot-a', 'workspace-a', 'capture-a', 'xhs', 'xhs:note:note-a', now());

INSERT INTO "RawRecord"
  ("id", "workspaceId", "rawSnapshotId", "recordKind", "platform", "targetKey",
   "externalRecordId", "payload", "payloadHash", "observedAt", "idempotencyKey")
VALUES
  ('record-a', 'workspace-a', 'snapshot-a', 'note', 'xhs', 'xhs:note:note-a',
   'note-a', '{}'::jsonb, 'raw-hash-a', now(), 'record-key-a');

INSERT INTO "NormalizationRun"
  ("id", "workspaceId", "rawSnapshotId", "rawRecordId", "adapterId", "adapterVersion",
   "canonicalSchemaVersion", "attemptNumber", "status", "inputPayloadHash", "outputPayloadHash")
VALUES
  ('run-a', 'workspace-a', 'snapshot-a', 'record-a', 'xhs-note', '1',
   '1', 1, 'normalized', 'raw-hash-a', 'canonical-hash-a');

INSERT INTO "NormalizationRunCurrent"
  ("workspaceId", "rawRecordId", "rawSnapshotId", "normalizationRunId", "adapterId",
   "adapterVersion", "canonicalSchemaVersion")
VALUES
  ('workspace-a', 'record-a', 'snapshot-a', 'run-a', 'xhs-note', '1', '1');

INSERT INTO "CanonicalObservation"
  ("id", "workspaceId", "normalizationRunId", "rawSnapshotId", "rawRecordId",
   "observationKind", "subjectKey", "observedAt", "schemaVersion", "payload",
   "payloadHash", "qualityStatus")
VALUES
  ('observation-a', 'workspace-a', 'run-a', 'snapshot-a', 'record-a',
   'content', 'xhs:note:note-a', now(), '1', '{}'::jsonb,
   'canonical-hash-a', 'complete');

INSERT INTO "ContractEvaluation"
  ("id", "workspaceId", "rawSnapshotId", "contractId", "contractVersion",
   "evaluatorVersion", "canonicalSchemaVersion", "evaluationInputHash", "decision", "completeness")
VALUES
  ('evaluation-a', 'workspace-a', 'snapshot-a', 'xhs.note-detail', 1,
   '1', '1', 'evaluation-hash-a', 'accepted', 'full');

INSERT INTO "ContractEvaluationNormalizationRun"
  ("id", "evaluationId", "normalizationRunId", "workspaceId", "rawSnapshotId",
   "status", "inputPayloadHash", "outputPayloadHash")
VALUES
  ('evaluation-run-a', 'evaluation-a', 'run-a', 'workspace-a', 'snapshot-a',
   'normalized', 'raw-hash-a', 'canonical-hash-a');

INSERT INTO "ContractEvaluationInput"
  ("id", "workspaceId", "rawSnapshotId", "contractEvaluationId",
   "canonicalObservationId", "ordinal", "canonicalOutputHash")
VALUES
  ('evaluation-input-a', 'workspace-a', 'snapshot-a', 'evaluation-a',
   'observation-a', 0, 'canonical-hash-a');

INSERT INTO "ContractEvaluationCurrent"
  ("workspaceId", "rawSnapshotId", "contractId", "contractVersion",
   "contractEvaluationId", "revision")
VALUES
  ('workspace-a', 'snapshot-a', 'xhs.note-detail', 1, 'evaluation-a', 1);

SELECT 'B2_DERIVED_POSITIVE_PASS' AS proof_marker;
