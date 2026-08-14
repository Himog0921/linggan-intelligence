/**
 * Dark Release-B composition root for the three authorized ingress kinds.
 *
 * This module closes the verified-authority → XHS contract registry →
 * EvidenceIngress chain without wiring any route. Migration intentionally has
 * no composition root (DEC-B1-019).
 */

import type { PrismaClient } from "@/lib/prisma-client";
import {
  verifyExecutionStationIngressSession,
  type VerifyExecutionStationIngressSessionInput,
} from "@/lib/services/execution-station-signature-service";
import {
  getV2DatabaseClient,
  validateV2DatabaseIdentity,
  type V2DatabaseRole,
} from "../security/v2-database-roles";

import {
  buildExecutionIngressBinding,
} from "../adapters/execution-adapter";
import {
  buildManualImportIngressBinding,
  type ManualImportAuthorityContext,
} from "../adapters/manual-import-adapter";
import {
  buildRecoveryIngressBinding,
  type RecoveryAuthorityContext,
} from "../adapters/recovery-adapter";
import { CollectionContractRegistry } from "../contracts/collection-contract-registry";
import { XHS_COLLECTION_CONTRACTS } from "../contracts/xhs-collection-contracts";
import {
  finalizeExecutionEvidenceControl,
  type EvidenceCommittedControlPending,
} from "./execution-control-finalizer";
import { EvidenceIngress } from "./evidence-ingress";
import type {
  CaptureSubmissionBodyV2,
  EvidenceIngressResult,
} from "./types";

const XHS_INGRESS_REGISTRY = new CollectionContractRegistry([
  ...XHS_COLLECTION_CONTRACTS,
]);

type IngressDependencies = {
  /** Explicit test seam. Production composition roots never pass this. */
  db?: PrismaClient;
  registry?: CollectionContractRegistry;
};

async function ingressDb(
  dependencies: IngressDependencies,
  role: V2DatabaseRole,
): Promise<PrismaClient> {
  if (dependencies.db) return dependencies.db;
  const db = getV2DatabaseClient(role);
  await validateV2DatabaseIdentity(db, role);
  return db;
}

export type ExecutionEvidenceIngressResult =
  | EvidenceIngressResult
  | EvidenceCommittedControlPending
  | {
      status: "evidence_committed_terminal";
      terminalState: "blocked" | "cancelled" | "error";
      rawSnapshotId: string;
      receiptId: string;
      checksumValue: string;
      retryable: false;
    };

export async function submitExecutionEvidence(
  request: VerifyExecutionStationIngressSessionInput,
  dependencies: IngressDependencies = {},
): Promise<ExecutionEvidenceIngressResult> {
  // The composition root owns the same immutable request snapshot throughout
  // verification and body parsing. A caller cannot swap body/method/path while
  // the verifier is awaiting its database reads.
  const requestSnapshot: VerifyExecutionStationIngressSessionInput = {
    stationId: request.stationId,
    stationToken: request.stationToken,
    pluginAuthorizationId: request.pluginAuthorizationId,
    pluginAuthorizationToken: request.pluginAuthorizationToken,
    method: request.method,
    path: request.path,
    headers: new Headers(request.headers),
    bodyText: request.bodyText,
    ...(request.now ? { now: new Date(request.now.getTime()) } : {}),
  };
  const [adapterDb, evidenceDb, controlDb] = await Promise.all([
    ingressDb(dependencies, "adapter_reader"),
    ingressDb(dependencies, "evidence_writer"),
    ingressDb(dependencies, "default_app"),
  ]);
  const registry = dependencies.registry ?? XHS_INGRESS_REGISTRY;
  const clock = requestSnapshot.now
    ? () => new Date(requestSnapshot.now!.getTime())
    : () => new Date();

  const session = await verifyExecutionStationIngressSession(requestSnapshot, adapterDb);
  const binding = await buildExecutionIngressBinding(
    requestSnapshot.bodyText,
    session,
    adapterDb,
    clock,
  );
  const evidenceResult = await new EvidenceIngress(
    evidenceDb,
    registry,
    binding.authorityValidator,
  ).submit(binding.submission);
  const controlResult = await finalizeExecutionEvidenceControl(
    { submission: binding.submission, evidenceResult },
    controlDb,
    clock,
  );

  if (controlResult.status === "evidence_committed_control_pending") return controlResult;
  if (
    (controlResult.status === "advanced" || controlResult.status === "already_advanced")
    && controlResult.terminalState !== "completed"
    && (evidenceResult.status === "committed" || evidenceResult.status === "replay")
  ) {
    return {
      status: "evidence_committed_terminal",
      terminalState: controlResult.terminalState,
      rawSnapshotId: evidenceResult.rawSnapshotId,
      receiptId: evidenceResult.receiptId,
      checksumValue: evidenceResult.checksumValue,
      retryable: false,
    };
  }
  return evidenceResult;
}

export async function submitManualImportEvidence(
  input: {
    body: CaptureSubmissionBodyV2;
    context: ManualImportAuthorityContext;
  },
  dependencies: IngressDependencies = {},
): Promise<EvidenceIngressResult> {
  const db = await ingressDb(dependencies, "evidence_writer");
  const registry = dependencies.registry ?? XHS_INGRESS_REGISTRY;
  const binding = buildManualImportIngressBinding(input.body, input.context);
  return new EvidenceIngress(
    db,
    registry,
    binding.authorityValidator,
  ).submit(binding.submission);
}

export async function submitRecoveryEvidence(
  input: {
    body: CaptureSubmissionBodyV2;
    context: RecoveryAuthorityContext;
  },
  dependencies: IngressDependencies = {},
): Promise<EvidenceIngressResult> {
  const db = await ingressDb(dependencies, "evidence_writer");
  const registry = dependencies.registry ?? XHS_INGRESS_REGISTRY;
  const binding = buildRecoveryIngressBinding(input.body, input.context);
  return new EvidenceIngress(
    db,
    registry,
    binding.authorityValidator,
  ).submit(binding.submission);
}
