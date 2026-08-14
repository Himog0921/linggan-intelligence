/**
 * Manual import authority provenance after DEC-B1-022.
 *
 * The dark V2 adapter consumes only the outputs of the authenticated plugin
 * authorization and user-workspace session boundaries. It has no route caller
 * yet and does not change the legacy V1 manual-import flow.
 */

const workspaceId = {
  verdict: "TRACEABLE" as const,
  sources: [
    "src/lib/services/plugin-authorization-service.ts:1222-1253 — bearer token authentication returns the server-side PluginAuthorization row",
    "src/lib/plugin-data-workspace.ts:120-173 — signed token/session membership returns workspaceId and userId",
    "src/lib/evidence/adapters/manual-import-adapter.ts — rejects null or unequal authorization/session workspace bindings",
    "docs/architecture/v2/01-decisions.md DEC-B1-022 — both verified sources must identify the same workspace",
  ],
  note: "The adapter fails closed unless PluginAuthorization.workspaceId is non-null and equals the authenticated session workspaceId.",
};

const sourcePrincipal = {
  verdict: "TRACEABLE" as const,
  sources: [
    "src/lib/plugin-data-workspace.ts:149-158 — verified session exposes stable userId",
    "src/lib/evidence/adapters/manual-import-adapter.ts — canonical sourcePrincipal is user:<userId>",
    "docs/architecture/v2/01-decisions.md DEC-B1-022 — identity format is confirmed",
  ],
};

const importerIdentity = {
  verdict: "TRACEABLE" as const,
  sources: [
    "src/lib/services/plugin-authorization-service.ts:1222-1253 — authenticated authorization has a server-side id",
    "src/lib/evidence/adapters/manual-import-adapter.ts — canonical importerIdentity is plugin-authorization:<authorizationId>",
    "docs/architecture/v2/01-decisions.md DEC-B1-022 — authorization id is used instead of non-global deviceId",
  ],
};

const receivedAt = {
  verdict: "TRACEABLE" as const,
  sources: [
    "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:252 — receivedAt uses only the server clock",
    "src/lib/evidence/adapters/manual-import-adapter.ts — clock is called inside the adapter and is not read from body",
    "src/lib/evidence/adapters/manual-import-adapter.test.ts — fixed server clock and body-smuggling rejection",
  ],
};

const noBodyOverride = {
  verdict: "TRACEABLE" as const,
  sources: [
    "src/lib/evidence/adapters/request-bound-authority.ts — rejects authority fields on body/header and exact-binds the generated authority",
    "src/lib/evidence/adapters/manual-import-adapter.test.ts — caller authority leakage and post-binding mutation are rejected",
  ],
};

const noExecutionFields = {
  verdict: "TRACEABLE" as const,
  sources: [
    "src/lib/evidence/ingress/types.ts:84-105 — manual_import header excludes execution identity",
    "src/lib/evidence/adapters/request-bound-authority.ts — runtime guard rejects job/attempt/station/lease/plan fields",
    "src/lib/evidence/adapters/manual-import-adapter.test.ts — fabricated execution identity is rejected",
    "src/lib/evidence/ingress/evidence-ingress.integration.test.ts — non-execution persistence writes execution columns as null",
  ],
  note: "The V2 adapter is dark and has no legacy service caller; the existing V1 route remains unchanged until the hard cutover task.",
};

export const MANUAL_IMPORT_PROVENANCE = {
  facts: {
    workspaceId,
    sourcePrincipal,
    importerIdentity,
    receivedAt,
    noBodyOverride,
    noExecutionFields,
  },
  canConstructProductionAuthority: true as const,
  blockers: [] as const,
} as const;
