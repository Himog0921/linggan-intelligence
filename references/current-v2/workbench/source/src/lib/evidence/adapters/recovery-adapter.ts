import type { CaptureSubmissionBodyV2 } from "../ingress/types";
import {
  assertNonExecutionBodyBoundary,
  bindAuthority,
  requiredIdentifier,
  serverReceivedAt,
  type RequestBoundIngressBinding,
} from "./request-bound-authority";

export type RecoveryAuthorityContext = {
  workspaceId: string;
  userId: string;
  role: string;
};

/**
 * Bind a V2 recovery body to an authenticated owner/admin workspace session.
 * Client/export timestamps remain observation facts and cannot become the
 * server-owned receivedAt authority field.
 */
export function buildRecoveryIngressBinding(
  body: CaptureSubmissionBodyV2,
  context: RecoveryAuthorityContext,
): RequestBoundIngressBinding {
  assertNonExecutionBodyBoundary(body, "recovery");

  const workspaceId = requiredIdentifier(context.workspaceId, "workspaceId");
  const userId = requiredIdentifier(context.userId, "userId");
  if (context.role !== "owner" && context.role !== "admin") {
    throw new Error("Recovery ingress requires an authenticated owner or admin session.");
  }
  const principal = `user:${userId}`;

  return bindAuthority(body, {
    ingressKind: "recovery",
    workspaceId,
    receivedAt: serverReceivedAt(),
    sourcePrincipal: principal,
    recoveryAuthorizedBy: principal,
  });
}
