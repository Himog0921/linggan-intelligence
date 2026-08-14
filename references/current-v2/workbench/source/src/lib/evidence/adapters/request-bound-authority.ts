import type {
  CaptureHeaderV2,
  CaptureSubmissionBodyV2,
  CaptureSubmissionV2,
  EvidenceIngressAuthorityV2,
} from "../ingress/types";
import type {
  AuthorityValidationResult,
  EvidenceIngressAuthorityValidator,
} from "../ingress/authority-validator";

const AUTHORITY_FIELDS = [
  "workspaceId",
  "receivedAt",
  "sourcePrincipal",
  "stationId",
  "leaseToken",
  "importerIdentity",
  "recoveryAuthorizedBy",
  "migrationAuthorization",
] as const;

const EXECUTION_IDENTITY_FIELDS = [
  "jobId",
  "attemptId",
  "stationId",
  "leaseToken",
  "leaseEpoch",
  "executionPlanVersion",
] as const;

type AdapterKind = "manual_import" | "recovery";
type BoundAuthority = Extract<
  EvidenceIngressAuthorityV2,
  { ingressKind: AdapterKind }
>;

export type RequestBoundIngressBinding = {
  submission: CaptureSubmissionV2;
  authorityValidator: EvidenceIngressAuthorityValidator;
};

export function requiredIdentifier(value: string | null | undefined, field: string): string {
  const normalized = value?.trim() ?? "";
  if (!normalized) {
    throw new Error(`${field} must be a non-empty verified identifier.`);
  }
  return normalized;
}

export function serverReceivedAt(): Date {
  const value = new Date();
  if (!(value instanceof Date) || !Number.isFinite(value.getTime())) {
    throw new Error("The server clock must return a valid Date.");
  }
  return new Date(value.getTime());
}

export function assertNonExecutionBodyBoundary(
  body: CaptureSubmissionBodyV2,
  expectedKind: AdapterKind,
): void {
  if (!isRecord(body) || !isRecord(body.header) || !isRecord(body.capturePackage)) {
    throw new Error(
      `CaptureSubmissionBodyV2 must contain object header and capturePackage fields for ${expectedKind} ingress.`,
    );
  }

  if (body.header.ingressKind !== expectedKind) {
    throw new Error(
      `CaptureSubmissionBodyV2.header.ingressKind must be ${expectedKind} for ${expectedKind} ingress.`,
    );
  }

  const containers: readonly object[] = [body, body.header];
  const authorityLeaks = AUTHORITY_FIELDS.filter((field) =>
    containers.some((container) => Object.hasOwn(container, field)),
  );
  const executionLeaks = EXECUTION_IDENTITY_FIELDS.filter((field) =>
    containers.some((container) => Object.hasOwn(container, field)),
  );

  if (authorityLeaks.length > 0 || executionLeaks.length > 0) {
    const details = [
      authorityLeaks.length > 0 ? `server-authority fields: ${authorityLeaks.join(", ")}` : null,
      executionLeaks.length > 0 ? `execution identity fields: ${executionLeaks.join(", ")}` : null,
    ].filter((detail): detail is string => detail !== null);
    throw new Error(
      `CaptureSubmissionBodyV2 crosses the ${expectedKind} trust boundary with ${details.join("; ")}.`,
    );
  }
}

export function bindAuthority(
  body: CaptureSubmissionBodyV2,
  authority: BoundAuthority,
): RequestBoundIngressBinding {
  const expected = cloneAuthority(authority);
  return {
    submission: { body, authority: cloneAuthority(expected) },
    authorityValidator: new ExactRequestAuthorityValidator(expected),
  };
}

class ExactRequestAuthorityValidator implements EvidenceIngressAuthorityValidator {
  constructor(private readonly expected: BoundAuthority) {}

  validate(
    authority: EvidenceIngressAuthorityV2,
    header: CaptureHeaderV2,
  ): AuthorityValidationResult {
    if (
      header.ingressKind !== this.expected.ingressKind ||
      !authorityEquals(authority, this.expected)
    ) {
      return {
        valid: false,
        reason: `${this.expected.ingressKind}_authority_invalid`,
      };
    }
    return { valid: true };
  }
}

function cloneAuthority(authority: BoundAuthority): BoundAuthority {
  return {
    ...authority,
    receivedAt: new Date(authority.receivedAt.getTime()),
  };
}

function authorityEquals(
  actual: EvidenceIngressAuthorityV2,
  expected: BoundAuthority,
): boolean {
  if (!isRecord(actual)) return false;
  if (actual.ingressKind !== expected.ingressKind) return false;
  if (!hasExactAuthorityKeys(actual, expected.ingressKind)) return false;
  if (!(actual.receivedAt instanceof Date)) return false;
  if (actual.receivedAt.getTime() !== expected.receivedAt.getTime()) return false;
  if (actual.workspaceId !== expected.workspaceId) return false;
  if (actual.sourcePrincipal !== expected.sourcePrincipal) return false;

  switch (expected.ingressKind) {
    case "manual_import":
      return actual.ingressKind === "manual_import" &&
        actual.importerIdentity === expected.importerIdentity;
    case "recovery":
      return actual.ingressKind === "recovery" &&
        actual.recoveryAuthorizedBy === expected.recoveryAuthorizedBy;
  }
}

function hasExactAuthorityKeys(
  authority: Record<PropertyKey, unknown>,
  ingressKind: AdapterKind,
): boolean {
  const expected = ingressKind === "manual_import"
    ? ["ingressKind", "workspaceId", "receivedAt", "sourcePrincipal", "importerIdentity"]
    : ["ingressKind", "workspaceId", "receivedAt", "sourcePrincipal", "recoveryAuthorizedBy"];
  const actual = Reflect.ownKeys(authority);
  return actual.length === expected.length &&
    expected.every((field) => Object.hasOwn(authority, field));
}

function isRecord(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
