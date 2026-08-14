import { createHash, createHmac, timingSafeEqual } from "node:crypto";

import { prisma } from "@/lib/db";
import { isPluginVersionAtLeast } from "@/lib/plugin-version-policy";
import type { PrismaClient } from "@/lib/prisma-client";
import { decryptStationSigningSecret } from "@/lib/services/execution-station-signing-secret";

const SIGNATURE_WINDOW_MS = 5 * 60_000;

export class ExecutionStationSignatureError extends Error {
  constructor(
    public code:
      | "SIGNATURE_MISSING"
      | "SIGNATURE_STATION_MISMATCH"
      | "SIGNATURE_STATION_NOT_FOUND"
      | "SIGNATURE_NOT_CONFIGURED"
      | "SIGNATURE_TIMESTAMP_INVALID"
      | "SIGNATURE_TIMESTAMP_EXPIRED"
      | "SIGNATURE_BODY_HASH_MISMATCH"
      | "SIGNATURE_INVALID"
      | "SIGNATURE_NONCE_REPLAYED"
      | "STATION_TOKEN_INVALID"
      | "PLUGIN_AUTHORIZATION_MISMATCH"
      | "PLUGIN_AUTHORIZATION_INVALID"
      | "PLUGIN_VERSION_INVALID"
      | "PLUGIN_VERSION_MISMATCH"
      | "PLUGIN_VERSION_OUTDATED"
      | "WORKSPACE_BINDING_INVALID",
    message: string
  ) {
    super(message);
  }
}

export type VerifyExecutionStationRequestSignatureInput = {
  stationId: string;
  method: string;
  path: string;
  headers: Headers;
  bodyText: string;
  now?: Date;
};

const VERIFIED_SESSION_TOKEN = Symbol("verified-execution-station-session");
export const V2_EXECUTION_MIN_PLUGIN_VERSION = "2.0.92";
const VERIFIED_SESSIONS = new WeakSet<VerifiedExecutionStationSession>();
const VERIFIED_SESSION_CLAIMS = new WeakMap<
  VerifiedExecutionStationSession,
  VerifiedExecutionStationSessionClaims
>();

export type VerifiedExecutionStationSessionClaims = Readonly<{
  kind: "verified_execution_station_session";
  stationId: string;
  workspaceId: string;
  pluginAuthorizationId: string;
  signingSecretVersion: number;
  requestMethod: string;
  requestPath: string;
  requestBodySha256: string;
  verifiedAt: Date;
}>;

/**
 * Runtime capability produced only after strict verification. A same-shaped
 * object or a directly constructed instance is not registered in the private
 * WeakSet and is rejected by readVerifiedExecutionStationSession.
 */
export class VerifiedExecutionStationSession {
  readonly kind = "verified_execution_station_session" as const;

  constructor(
    token: symbol,
    claims: VerifiedExecutionStationSessionClaims,
  ) {
    if (token !== VERIFIED_SESSION_TOKEN) {
      throw new Error("VerifiedExecutionStationSession cannot be constructed directly.");
    }
    VERIFIED_SESSIONS.add(this);
    VERIFIED_SESSION_CLAIMS.set(this, Object.freeze({
      ...claims,
      verifiedAt: new Date(claims.verifiedAt.getTime()),
    }));
    Object.freeze(this);
  }

  snapshot(): VerifiedExecutionStationSessionClaims {
    const claims = VERIFIED_SESSION_CLAIMS.get(this);
    if (!VERIFIED_SESSIONS.has(this) || !claims) {
      throw new Error("Verified execution station session capability is invalid.");
    }
    return Object.freeze({
      ...claims,
      verifiedAt: new Date(claims.verifiedAt.getTime()),
    });
  }
}

export function readVerifiedExecutionStationSession(
  value: unknown,
): VerifiedExecutionStationSessionClaims {
  if (!(value instanceof VerifiedExecutionStationSession) || !VERIFIED_SESSIONS.has(value)) {
    throw new Error("A genuine verified execution station session is required.");
  }
  return value.snapshot();
}

export type VerifyExecutionStationIngressSessionInput =
  VerifyExecutionStationRequestSignatureInput & {
    stationToken: string;
    pluginAuthorizationId: string;
    pluginAuthorizationToken: string;
  };

export function sha256Hex(value: string) {
  return createHash("sha256").update(value).digest("hex");
}

export function stationRequestSignaturePayload(input: {
  method: string;
  path: string;
  timestamp: string;
  nonce: string;
  bodyHash: string;
}) {
  return [
    input.method.toUpperCase(),
    input.path,
    input.timestamp,
    input.nonce,
    input.bodyHash,
  ].join("\n");
}

export function signStationRequest(input: {
  method: string;
  path: string;
  timestamp: string;
  nonce: string;
  bodyHash: string;
  signingSecret: string;
}) {
  return createHmac("sha256", input.signingSecret)
    .update(stationRequestSignaturePayload(input))
    .digest("hex");
}

function requiredHeader(headers: Headers, name: string) {
  const value = headers.get(name);
  if (!value?.trim()) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_MISSING",
      `Missing ${name} header`
    );
  }
  return value.trim();
}

function parseTimestamp(value: string) {
  const date = new Date(value);
  return Number.isFinite(date.getTime()) ? date : null;
}

function safeEqualText(left: string, right: string) {
  const leftBuffer = Buffer.from(left);
  const rightBuffer = Buffer.from(right);
  if (leftBuffer.length !== rightBuffer.length) return false;
  return timingSafeEqual(leftBuffer, rightBuffer);
}

function isUniqueViolation(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code?: unknown }).code === "P2002"
  );
}

export async function verifyExecutionStationRequestSignature(
  input: VerifyExecutionStationRequestSignatureInput
): Promise<void> {
  // V1.1 签名要求是渐进式推出的：工位配对时生成签名密钥并存到
  // signatureRequiredAt，但在插件端全量支持签名协议之前，服务端不强制验证。
  // 这里只在 signatureRequiredAt 已到期 且 密钥存在时才做签名校验；
  // 否则直接放行（兼容未升级签名的旧插件）。
  const now = input.now ?? new Date();
  const station = await prisma.executionStation.findUnique({
    where: { id: input.stationId },
    select: {
      id: true,
      signingSecretEncrypted: true,
      signatureRequiredAt: true,
    },
  });
  if (!station) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_STATION_NOT_FOUND",
      "Station was not found for signature verification"
    );
  }

  const signingSecret = decryptStationSigningSecret(station.signingSecretEncrypted);
  const signatureIsRequired =
    station.signatureRequiredAt != null &&
    station.signatureRequiredAt.getTime() <= now.getTime();

  if (!signingSecret || !signatureIsRequired) {
    // 密钥不存在 或 签名要求尚未到生效时间：跳过验证，允许通过。
    return;
  }

  await verifyRequiredSignature(input, signingSecret, now);
}

/**
 * Strict V2 trust boundary. Unlike the legacy sync verifier, this function has
 * no rollout grace period: station token, request HMAC, plugin authorization
 * binding, and non-null workspace equality are all mandatory.
 *
 * This is intentionally a dark seam in B1-B-10. Existing routes keep using
 * verifyExecutionStationRequestSignature until the one-time Release-B cutover.
 */
export async function verifyExecutionStationIngressSession(
  input: VerifyExecutionStationIngressSessionInput,
  database: PrismaClient = prisma,
): Promise<VerifiedExecutionStationSession> {
  // Capture the entire untrusted request synchronously. Database reads and
  // nonce persistence introduce await boundaries; no value used for claims or
  // HMAC verification may be re-read from a caller-owned mutable object.
  const request = snapshotIngressSessionInput(input);
  const now = request.now ?? new Date();
  const station = await database.executionStation.findUnique({
    where: { id: request.stationId },
    select: {
      id: true,
      workspaceId: true,
      pluginAuthorizationId: true,
      stationTokenHash: true,
      signingSecretEncrypted: true,
      signingSecretVersion: true,
    },
  });
  if (!station) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_STATION_NOT_FOUND",
      "Station was not found for V2 ingress verification"
    );
  }

  const suppliedTokenHash = request.stationToken.trim()
    ? sha256Hex(request.stationToken)
    : "";
  if (
    !station.stationTokenHash ||
    !suppliedTokenHash ||
    !safeEqualText(station.stationTokenHash, suppliedTokenHash)
  ) {
    throw new ExecutionStationSignatureError(
      "STATION_TOKEN_INVALID",
      "Station token is missing or invalid"
    );
  }

  const pluginAuthorization = await database.pluginAuthorization.findUnique({
    where: { id: request.pluginAuthorizationId },
    select: {
      id: true,
      workspaceId: true,
      authorizationTokenHash: true,
      status: true,
      expiresAt: true,
    },
  });
  const suppliedAuthorizationHash = request.pluginAuthorizationToken.trim()
    ? sha256Hex(request.pluginAuthorizationToken)
    : "";
  if (
    !pluginAuthorization ||
    !pluginAuthorization.authorizationTokenHash ||
    !suppliedAuthorizationHash ||
    !safeEqualText(
      pluginAuthorization.authorizationTokenHash,
      suppliedAuthorizationHash,
    ) ||
    pluginAuthorization.status !== "active" ||
    (pluginAuthorization.expiresAt !== null &&
      pluginAuthorization.expiresAt.getTime() <= now.getTime())
  ) {
    throw new ExecutionStationSignatureError(
      "PLUGIN_AUTHORIZATION_INVALID",
      "Plugin authorization is missing, expired, revoked, or has an invalid token"
    );
  }

  if (station.pluginAuthorizationId !== pluginAuthorization.id) {
    throw new ExecutionStationSignatureError(
      "PLUGIN_AUTHORIZATION_MISMATCH",
      "Authenticated plugin authorization is not bound to this station"
    );
  }

  if (
    !station.workspaceId ||
    !pluginAuthorization.workspaceId ||
    station.workspaceId !== pluginAuthorization.workspaceId
  ) {
    throw new ExecutionStationSignatureError(
      "WORKSPACE_BINDING_INVALID",
      "Station and plugin authorization must have the same non-null workspace"
    );
  }

  const signingSecret = decryptStationSigningSecret(station.signingSecretEncrypted);
  if (!signingSecret) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_NOT_CONFIGURED",
      "Station signing secret is required for V2 ingress"
    );
  }

  await verifyRequiredSignature(
    request,
    signingSecret,
    now,
    database,
    () => assertV2PluginVersionBinding(request),
  );

  return new VerifiedExecutionStationSession(VERIFIED_SESSION_TOKEN, {
    kind: "verified_execution_station_session",
    stationId: station.id,
    workspaceId: station.workspaceId,
    pluginAuthorizationId: station.pluginAuthorizationId,
    signingSecretVersion: station.signingSecretVersion,
    requestMethod: request.method.toUpperCase(),
    requestPath: request.path,
    requestBodySha256: sha256Hex(request.bodyText),
    verifiedAt: new Date(now.getTime()),
  });
}

function snapshotIngressSessionInput(
  input: VerifyExecutionStationIngressSessionInput,
): VerifyExecutionStationIngressSessionInput {
  return {
    stationId: input.stationId,
    stationToken: input.stationToken,
    pluginAuthorizationId: input.pluginAuthorizationId,
    pluginAuthorizationToken: input.pluginAuthorizationToken,
    method: input.method,
    path: input.path,
    headers: new Headers(input.headers),
    bodyText: input.bodyText,
    ...(input.now ? { now: new Date(input.now.getTime()) } : {}),
  };
}

async function verifyRequiredSignature(
  input: VerifyExecutionStationRequestSignatureInput,
  signingSecret: string,
  now: Date,
  database: PrismaClient = prisma,
  beforeNonce?: () => void,
): Promise<void> {
  const headerStationId = requiredHeader(input.headers, "x-cw-station-id");
  const timestampText = requiredHeader(input.headers, "x-cw-timestamp");
  const nonce = requiredHeader(input.headers, "x-cw-nonce");
  const bodyHash = requiredHeader(input.headers, "x-cw-body-sha256");
  const signature = requiredHeader(input.headers, "x-cw-signature");

  if (headerStationId !== input.stationId) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_STATION_MISMATCH",
      "Signature station header does not match request body"
    );
  }

  const timestamp = parseTimestamp(timestampText);
  if (!timestamp) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_TIMESTAMP_INVALID",
      "Signature timestamp is invalid"
    );
  }
  if (Math.abs(now.getTime() - timestamp.getTime()) > SIGNATURE_WINDOW_MS) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_TIMESTAMP_EXPIRED",
      "Signature timestamp is outside the accepted window"
    );
  }

  const actualBodyHash = sha256Hex(input.bodyText);
  if (!safeEqualText(bodyHash, actualBodyHash)) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_BODY_HASH_MISMATCH",
      "Request body hash does not match the signed body hash"
    );
  }

  const expectedSignature = signStationRequest({
    method: input.method,
    path: input.path,
    timestamp: timestampText,
    nonce,
    bodyHash,
    signingSecret,
  });
  if (!safeEqualText(signature, expectedSignature)) {
    throw new ExecutionStationSignatureError(
      "SIGNATURE_INVALID",
      "Request signature is invalid"
    );
  }

  // The body has now been hash- and HMAC-verified. Bind the independently
  // transported version header to the signed collectorVersion before a nonce
  // is consumed or any authorization-dependent work begins.
  beforeNonce?.();

  const expiresAt = new Date(timestamp.getTime() + SIGNATURE_WINDOW_MS);
  try {
    await database.$transaction(async (tx) => {
      await tx.executionStationRequestNonce.deleteMany({
        where: { expiresAt: { lt: now } },
      });
      const inserted = await tx.executionStationRequestNonce.createMany({
        data: {
          stationId: input.stationId,
          nonce,
          issuedAt: timestamp,
          expiresAt,
        },
      });
      if (inserted.count !== 1) {
        throw new Error("Signature nonce insert did not create exactly one row");
      }
    });
  } catch (error) {
    if (isUniqueViolation(error)) {
      throw new ExecutionStationSignatureError(
        "SIGNATURE_NONCE_REPLAYED",
        "Signature nonce has already been used"
      );
    }
    throw error;
  }
}

function assertV2PluginVersionBinding(
  input: VerifyExecutionStationIngressSessionInput,
): void {
  const reportedVersion = requiredHeader(input.headers, "x-cw-plugin-version");
  if (!/^\d+\.\d+\.\d+$/.test(reportedVersion)) {
    throw new ExecutionStationSignatureError(
      "PLUGIN_VERSION_INVALID",
      "V2 plugin version must be an exact numeric semantic version",
    );
  }
  let body: unknown;
  try {
    body = JSON.parse(input.bodyText);
  } catch {
    throw new ExecutionStationSignatureError(
      "PLUGIN_VERSION_INVALID",
      "Signed V2 body is not valid JSON",
    );
  }
  const collectorVersion = readCollectorVersion(body);
  if (!collectorVersion || !/^\d+\.\d+\.\d+$/.test(collectorVersion)) {
    throw new ExecutionStationSignatureError(
      "PLUGIN_VERSION_INVALID",
      "Signed V2 body has no valid header.collectorVersion",
    );
  }
  if (reportedVersion !== collectorVersion) {
    throw new ExecutionStationSignatureError(
      "PLUGIN_VERSION_MISMATCH",
      "Plugin version header does not match signed header.collectorVersion",
    );
  }
  if (!isPluginVersionAtLeast(reportedVersion, V2_EXECUTION_MIN_PLUGIN_VERSION)) {
    throw new ExecutionStationSignatureError(
      "PLUGIN_VERSION_OUTDATED",
      `V2 execution ingress requires plugin ${V2_EXECUTION_MIN_PLUGIN_VERSION} or newer`,
    );
  }
}

function readCollectorVersion(value: unknown): string | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const header = (value as { header?: unknown }).header;
  if (typeof header !== "object" || header === null || Array.isArray(header)) return null;
  const collectorVersion = (header as { collectorVersion?: unknown }).collectorVersion;
  return typeof collectorVersion === "string" ? collectorVersion.trim() : null;
}
