-- V2 Release-B: Evidence security foundation (pure expand).
CREATE TABLE "EvidenceAccessAudit" (
    "id"               TEXT NOT NULL DEFAULT gen_random_uuid()::text,
    "workspaceId"      TEXT NOT NULL,
    "capturePackageId" TEXT NOT NULL,
    "accessedBy"       TEXT NOT NULL,
    "accessReason"     TEXT NOT NULL,
    "requestTraceId"   TEXT,
    "restricted"       BOOLEAN NOT NULL DEFAULT false,
    "accessedAt"       TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "EvidenceAccessAudit_pkey" PRIMARY KEY ("id"),
    CONSTRAINT "EvidenceAccessAudit_workspace_package_fkey"
      FOREIGN KEY ("workspaceId", "capturePackageId")
      REFERENCES "CapturePackage"("workspaceId", "id")
      ON DELETE RESTRICT ON UPDATE CASCADE
);
CREATE INDEX "EvidenceAccessAudit_workspaceId_capturePackageId_idx"
  ON "EvidenceAccessAudit"("workspaceId", "capturePackageId");

CREATE TABLE "EvidenceReaderWorkspaceGrant" (
    "id"          TEXT NOT NULL DEFAULT gen_random_uuid()::text,
    "workspaceId" TEXT NOT NULL,
    "readerRole"  TEXT NOT NULL,
    "grantedAt"   TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "grantedBy"   TEXT NOT NULL,
    "revokedAt"   TIMESTAMP(3),
    CONSTRAINT "EvidenceReaderWorkspaceGrant_pkey" PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "EvidenceReaderWorkspaceGrant_workspaceId_readerRole_key"
  ON "EvidenceReaderWorkspaceGrant"("workspaceId", "readerRole");
