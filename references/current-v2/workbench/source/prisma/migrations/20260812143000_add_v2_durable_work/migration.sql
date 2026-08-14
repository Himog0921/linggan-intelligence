-- V2 Release-B: Durable downstream work (pure expand).
CREATE TABLE "V2DurableWork" (
    "id"              TEXT NOT NULL DEFAULT gen_random_uuid()::text,
    "workspaceId"     TEXT NOT NULL,
    "rawSnapshotId"   TEXT NOT NULL,
    "receiptId"       TEXT NOT NULL,
    "status"          TEXT NOT NULL DEFAULT 'pending',
    "attemptCount"    INTEGER NOT NULL DEFAULT 0,
    "nextAttemptAt"   TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "lockedBy"        TEXT,
    "lockedAt"        TIMESTAMP(3),
    "leaseExpiresAt"  TIMESTAMP(3),
    "lastError"       TEXT,
    "deadReason"      TEXT,
    "b2Result"        JSONB,
    "b3Result"        JSONB,
    "createdAt"       TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt"       TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "V2DurableWork_pkey" PRIMARY KEY ("id"),
    CONSTRAINT "V2DurableWork_snapshot_fkey"
      FOREIGN KEY ("workspaceId", "rawSnapshotId")
      REFERENCES "RawSnapshot"("workspaceId", "id")
      ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "V2DurableWork_receipt_fkey"
      FOREIGN KEY ("workspaceId", "receiptId")
      REFERENCES "EvidenceIngressReceipt"("workspaceId", "id")
      ON DELETE RESTRICT ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "V2DurableWork_workspaceId_rawSnapshotId_key"
  ON "V2DurableWork"("workspaceId", "rawSnapshotId");
CREATE INDEX "V2DurableWork_status_nextAttemptAt_idx"
  ON "V2DurableWork"("status", "nextAttemptAt");
CREATE INDEX "V2DurableWork_workspaceId_status_idx"
  ON "V2DurableWork"("workspaceId", "status");
