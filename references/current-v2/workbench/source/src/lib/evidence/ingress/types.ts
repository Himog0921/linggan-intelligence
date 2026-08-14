/**
 * CaptureSubmissionV2 precise types per 07-evidence-ingress-release-b-contract.md §3.
 * Every type is transcribed verbatim from the contract — no invented fields.
 */

import type { JsonValue } from "./canonical-json";

// ── §3.1 Source-controllable header ───────────────────────────────────────

export type CaptureTargetV2 = {
  expectedTargetKey: string;
  observedTargetKey: string | null;
};

export type CaptureTerminalStateV2 =
  | "completed"
  | "blocked"
  | "cancelled"
  | "error";

export type CaptureTerminalReasonV2 =
  | "source_exhausted"
  | "limit_reached"
  | "target_missing"
  | "login_required"
  | "platform_blocked"
  | "parser_failed"
  | "network_failed"
  | "user_cancelled";

export type CaptureTerminalV2 = {
  state: CaptureTerminalStateV2;
  reason: CaptureTerminalReasonV2;
  retryable: boolean;
};

export type CaptureSlotStatusV2 =
  | "observed"
  | "absent"
  | "unavailable"
  | "not_applicable"
  | "invalid";

export type CaptureSlotV2 = {
  slotId: string;
  status: CaptureSlotStatusV2;
  reason: string | null;
};

export type CaptureCountersV2 = {
  requested: number;
  discovered: number;
  emitted: number;
  deduplicated: number;
  failed: number;
};

export type CaptureReportV2 = {
  startedAt: string;
  completedAt: string;
  terminal: CaptureTerminalV2;
  slots: CaptureSlotV2[];
  counters: CaptureCountersV2;
  diagnostics: { [key: string]: JsonValue };
};

export type EvidencePlatformV2 = "xhs";

export type IngressKindV2 = "execution" | "manual_import" | "recovery" | "migration";

export type CaptureHeaderCommonV2 = {
  protocolVersion: "capture-submission/v2";
  captureId: string;
  platform: EvidencePlatformV2;
  target: CaptureTargetV2;
  observedAt: string;
  collectorVersion: string;
  contractId: string;
  contractVersion: number;
  contractHash: string;
  report: CaptureReportV2;
};

export type CaptureHeaderV2 = CaptureHeaderCommonV2 &
  (
    | {
        ingressKind: "execution";
        jobId: string;
        attemptId: string;
        leaseEpoch: number;
        executionPlanVersion: string;
      }
    | {
        ingressKind: "manual_import";
        sourceSummary: string;
      }
    | {
        ingressKind: "recovery";
        recoveryCaptureId: string;
      }
    | {
        ingressKind: "migration";
        sourceSummary: string;
      }
  );

// ── §3.2 RawRecord and Artifact ───────────────────────────────────────────

export type RecordKindV2 = "note" | "comment" | "author" | "metric";

export type RawRecordSubmissionV2 = {
  idempotencyKey: string;
  recordKind: RecordKindV2;
  platform: EvidencePlatformV2;
  targetKey: string | null;
  externalRecordId: string | null;
  sequence: number;
  payload: JsonValue;
  observedAt: string;
};

export type ArtifactKindV2 =
  | "platform_response"
  | "page_snapshot"
  | "dom_fragment"
  | "media_inventory"
  | "context";

export type CaptureArtifactSubmissionV2 = {
  kind: ArtifactKindV2;
  encoding: "base64";
  artifactPayload: string;
  artifactChecksum: string;
  contentLength: number;
  restricted: boolean;
};

// ── §3.3 Decoded CapturePackage payload ────────────────────────────────────

export type CapturePackagePayloadV2 = {
  schemaVersion: "capture-package/v2";
  header: CaptureHeaderV2;
  records: RawRecordSubmissionV2[];
  artifacts: CaptureArtifactSubmissionV2[];
};

export type CapturePackageSubmissionV2 = {
  encoding: "base64";
  packagePayload: string;
  checksumAlgorithm: "sha256";
  checksumValue: string;
  contentLength: number;
  restricted: boolean;
};

export type CaptureSubmissionBodyV2 = {
  header: CaptureHeaderV2;
  capturePackage: CapturePackageSubmissionV2;
};

// ── §3.4 Server authority context ──────────────────────────────────────────

export type EvidenceIngressAuthorityV2 =
  | {
      ingressKind: "execution";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      stationId: string;
      leaseToken: string;
    }
  | {
      ingressKind: "manual_import";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      importerIdentity: string;
    }
  | {
      ingressKind: "recovery";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      recoveryAuthorizedBy: string;
    }
  | {
      ingressKind: "migration";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      migrationAuthorization: string;
    };

export type CaptureSubmissionV2 = {
  body: CaptureSubmissionBodyV2;
  authority: EvidenceIngressAuthorityV2;
};

// ── §4 CollectionContract ──────────────────────────────────────────────────

export type CollectionContractDefinitionV2 = {
  id: string;
  version: number;
  sourceContracts?: {
    recordPayload: { schemaVersion: string; contractHash: string };
    mediaInventory: { schemaVersion: string; contractHash: string };
  };
  platforms: EvidencePlatformV2[];
  recordKinds: RecordKindV2[];
  slots: Array<{
    slotId: string;
    requirement: "required" | "optional" | "conditional";
  }>;
  terminalPolicy: {
    allowedStates: CaptureTerminalStateV2[];
    allowEmptyRecords: boolean;
  };
  mediaPolicy: "not_required" | "metadata_only" | "source_required";
};

// ── §5.3 Result contract ───────────────────────────────────────────────────

export type EvidenceIngressResult =
  | {
      status: "committed";
      rawSnapshotId: string;
      receiptId: string;
      integrityStatus: "verified";
      checksumValue: string;
    }
  | {
      status: "conflict";
      rawSnapshotId: string;
      receiptId: string;
      integrityStatus: "capture_identity_conflict";
      checksumValue: string;
    }
  | {
      status: "replay";
      rawSnapshotId: string;
      receiptId: string;
      integrityStatus: "verified" | "capture_identity_conflict";
      checksumValue: string;
    }
  | {
      status: "rejected";
      reason: EvidenceIngressRejectReason;
      retryable: boolean;
    };

export type EvidenceIngressRejectReason =
  | "invalid_submission"
  | "unsupported_platform"
  | "invalid_base64"
  | "package_not_canonical"
  | "package_checksum_mismatch"
  | "package_length_mismatch"
  | "invalid_artifact_base64"
  | "artifact_checksum_mismatch"
  | "artifact_length_mismatch"
  | "header_package_mismatch"
  | "target_identity_mismatch"
  | "contract_not_registered"
  | "contract_hash_mismatch"
  | "contract_shape_mismatch"
  | "execution_authority_invalid"
  | "manual_import_authority_invalid"
  | "recovery_authority_invalid"
  | "migration_authority_invalid"
  | "concurrency_exhausted";
