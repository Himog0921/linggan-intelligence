import { randomUUID } from "node:crypto";

import { NextRequest, NextResponse } from "next/server";

import {
  submitExecutionEvidence,
} from "@/lib/evidence/ingress/evidence-ingress-orchestrator";
import { isAuthorizedV2CutoverCanaryRequest } from "@/lib/evidence/cutover/v2-cutover-canary-gate";
import { ExecutionStationSignatureError } from "@/lib/services/execution-station-signature-service";

const CUTOVER_RECEIPT_HEADER = "x-v2-cutover-request-receipt";

function requireBoundCanaryCapture(request: NextRequest, bodyText: string): string | null {
  const requestReceipt = request.headers.get(CUTOVER_RECEIPT_HEADER)?.trim();
  if (!requestReceipt) return null;
  if (!isAuthorizedV2CutoverCanaryRequest(request)) {
    throw new Error("invalid_cutover_canary_binding");
  }
  let captureId: unknown;
  try {
    const body = JSON.parse(bodyText) as { header?: { captureId?: unknown } };
    captureId = body.header?.captureId;
  } catch {
    throw new Error("invalid_cutover_canary_binding");
  }
  if (captureId !== requestReceipt) {
    throw new Error("invalid_cutover_canary_binding");
  }
  return requestReceipt;
}

function requiredHeader(request: NextRequest, name: string): string {
  const value = request.headers.get(name)?.trim();
  if (!value) throw new Error(`missing_${name}`);
  return value;
}

export async function POST(request: NextRequest) {
  const requestId = randomUUID();
  try {
    const bodyText = await request.text();
    const cutoverRequestReceipt = requireBoundCanaryCapture(request, bodyText);
    const stationId = requiredHeader(request, "x-cw-station-id");
    const stationToken = requiredHeader(request, "x-cw-station-token");
    const pluginAuthorizationId = requiredHeader(request, "x-cw-plugin-authorization-id");
    const result = await submitExecutionEvidence({
      stationId,
      stationToken,
      pluginAuthorizationId,
      pluginAuthorizationToken: requiredHeader(request, "authorization").replace(/^Bearer\s+/i, ""),
      method: request.method,
      path: request.nextUrl.pathname,
      headers: request.headers,
      bodyText,
    });

    if (result.status === "evidence_committed_control_pending") {
      return NextResponse.json(
        { ok: false, ...result },
        { status: 503, headers: { "x-request-id": requestId } },
      );
    }
    if (result.status === "evidence_committed_terminal") {
      return NextResponse.json(
        { ok: true, executionSucceeded: false, ...result },
        { status: 200, headers: { "x-request-id": requestId } },
      );
    }
    if (cutoverRequestReceipt && result.status === "replay") {
      return NextResponse.json(
        { ok: false, reason: "cutover_canary_already_consumed", retryable: false },
        { status: 409, headers: { "x-request-id": requestId } },
      );
    }
    if ((result.status === "committed" || result.status === "replay") && result.integrityStatus === "verified") {
      return NextResponse.json(
        { ok: true, ...result },
        { status: 200, headers: { "x-request-id": requestId } },
      );
    }
    const retryable = result.status === "rejected" ? result.retryable : false;
    return NextResponse.json(
      {
        ok: false,
        ...result,
        reason: result.status === "rejected" ? result.reason : "capture_identity_conflict",
        retryable,
      },
      { status: retryable ? 503 : 409, headers: { "x-request-id": requestId } },
    );
  } catch (error) {
    if (error instanceof ExecutionStationSignatureError) {
      return NextResponse.json(
        { ok: false, reason: error.code, retryable: false },
        { status: 401, headers: { "x-request-id": requestId } },
      );
    }
    if (
      error instanceof Error &&
      error.message === "Execution submission is not bound to a verified current execution context."
    ) {
      return NextResponse.json(
        { ok: false, reason: "execution_context_not_current", retryable: false },
        { status: 409, headers: { "x-request-id": requestId } },
      );
    }
    if (error instanceof Error && (error.message.startsWith("missing_") || error.message === "invalid_cutover_canary_binding")) {
      return NextResponse.json(
        { ok: false, reason: error.message, retryable: false },
        { status: 400, headers: { "x-request-id": requestId } },
      );
    }
    throw error;
  }
}
