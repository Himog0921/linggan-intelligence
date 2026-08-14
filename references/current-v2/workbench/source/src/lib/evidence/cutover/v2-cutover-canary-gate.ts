import { createHash, timingSafeEqual } from "node:crypto";

type Environment = Record<string, string | undefined>;

const TOKEN_HEADER = "x-v2-cutover-token";
const WORKSPACE_HEADER = "x-v2-cutover-workspace-id";
const RECEIPT_HEADER = "x-v2-cutover-request-receipt";

export function isAuthorizedV2CutoverCanaryRequest(
  request: Request,
  env: Environment = process.env,
): boolean {
  const expectedTokenHash = env.V2_CUTOVER_CANARY_TOKEN_SHA256?.trim();
  const expectedStationId = env.V2_CUTOVER_CANARY_STATION_ID?.trim();
  const expectedWorkspaceId = env.V2_CUTOVER_CANARY_WORKSPACE_ID?.trim();
  const expectedMaterialId = env.V2_CUTOVER_CANARY_MATERIAL_ID?.trim();
  const expectedRequestReceipt = env.V2_CUTOVER_CANARY_REQUEST_RECEIPT?.trim();
  if (
    !expectedTokenHash?.match(/^[0-9a-f]{64}$/)
    || !expectedStationId
    || !expectedWorkspaceId
    || !expectedMaterialId
    || !expectedRequestReceipt
  ) return false;

  const token = request.headers.get(TOKEN_HEADER);
  const workspaceId = request.headers.get(WORKSPACE_HEADER)?.trim();
  const requestReceipt = request.headers.get(RECEIPT_HEADER)?.trim();
  if (!token || !/^[A-Za-z0-9_-]{43,128}$/.test(token) || workspaceId !== expectedWorkspaceId || requestReceipt !== expectedRequestReceipt) {
    return false;
  }
  const actualHash = createHash("sha256").update(token, "utf8").digest();
  const expectedHash = Buffer.from(expectedTokenHash, "hex");
  if (actualHash.length !== expectedHash.length || !timingSafeEqual(actualHash, expectedHash)) return false;

  const url = new URL(request.url);
  if (url.search || url.hash) return false;
  if (request.method === "POST" && url.pathname === "/api/v2/evidence/execution") {
    return request.headers.get("x-cw-station-id")?.trim() === expectedStationId;
  }
  return request.method === "GET" && url.pathname === `/materials/${encodeURIComponent(expectedMaterialId)}`;
}

export const V2_CUTOVER_CANARY_HEADERS = {
  token: TOKEN_HEADER,
  workspaceId: WORKSPACE_HEADER,
  requestReceipt: RECEIPT_HEADER,
} as const;
