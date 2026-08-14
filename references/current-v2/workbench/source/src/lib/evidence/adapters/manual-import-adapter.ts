import type { CaptureSubmissionBodyV2 } from "../ingress/types";
import {
  assertNonExecutionBodyBoundary,
  bindAuthority,
  requiredIdentifier,
  serverReceivedAt,
  type RequestBoundIngressBinding,
} from "./request-bound-authority";

export type ManualImportAuthorityContext = {
  workspaceId: string;
  userId: string;
  pluginAuthorization: {
    id: string;
    workspaceId: string | null;
  };
};

/**
 * Bind a V2 manual_import body to the already authenticated plugin
 * authorization and user-session context. This is a dark adapter seam: it
 * has no route caller and does not submit or persist Evidence by itself.
 */
export function buildManualImportIngressBinding(
  body: CaptureSubmissionBodyV2,
  context: ManualImportAuthorityContext,
): RequestBoundIngressBinding {
  assertNonExecutionBodyBoundary(body, "manual_import");

  const workspaceId = requiredIdentifier(context.workspaceId, "workspaceId");
  const userId = requiredIdentifier(context.userId, "userId");
  const authorizationId = requiredIdentifier(
    context.pluginAuthorization.id,
    "pluginAuthorization.id",
  );
  const authorizationWorkspaceId = requiredIdentifier(
    context.pluginAuthorization.workspaceId,
    "pluginAuthorization.workspaceId",
  );
  if (authorizationWorkspaceId !== workspaceId) {
    throw new Error(
      "Plugin authorization and user session must be bound to the same workspace.",
    );
  }

  return bindAuthority(body, {
    ingressKind: "manual_import",
    workspaceId,
    receivedAt: serverReceivedAt(),
    sourcePrincipal: `user:${userId}`,
    importerIdentity: `plugin-authorization:${authorizationId}`,
  });
}
