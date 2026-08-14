import { NextRequest, NextResponse } from "next/server";
import { auth } from "@/lib/auth";
import {
  getMediaLibraryMaintenanceState,
  MEDIA_LIBRARY_MANUAL_WRITE_RECOVERY_MODE,
  MEDIA_LIBRARY_READ_ONLY_RECOVERY_MODE,
} from "@/lib/media-library-maintenance";
import { isAuthorizedV2CutoverCanaryRequest } from "@/lib/evidence/cutover/v2-cutover-canary-gate";

const PUBLIC_PATHS = new Set(["/api/auth", "/favicon.ico"]);
const PUBLIC_PREFIXES = ["/api/auth/", "/api/push/", "/_next/"];
const MEDIA_LIBRARY_MAINTENANCE_PAGE = "/media-library-maintenance";
const MANUAL_COVER_UPLOAD_PATH = "/api/media-assets/cover";
const MAINTENANCE_SAFE_AUTH_SESSION_PATHS = new Set([
  "/api/auth/sign-in/email",
  "/api/auth/sign-out",
]);
// HC-7 的业务只读恢复不是“放行所有 GET”。每条路径都须逐条核验其 GET
// 路由只读取工作区数据，不能触发缓存失效、任务入队、同步或媒体处理。
const REVIEWED_BUSINESS_READ_RECOVERY_PATHS = new Set([
  "/api/topics",
  "/api/topics/tag-options",
]);
const MAINTENANCE_AWARE_SCHEDULER_PATHS = new Set([
  "/api/scheduler/tick",
  "/api/scheduler/maintenance",
]);

const PLUGIN_PROTOCOL_PATHS = [
  "/api/collect/status",
  "/api/plugin-authorizations/activate",
  "/api/plugin-data-workspace",
  "/api/media-assets/cover",
  "/api/execution-stations/register",
  "/api/execution-stations/sync",
  "/api/v2/evidence/execution",
  "/api/execution-tasks/manual-import",
  "/api/execution-stations/push-subscription",
];

function hasSessionCookie(request: NextRequest) {
  return Boolean(request.headers.get("cookie")?.trim());
}

function isPluginAuthorizationRequestPath(pathname: string) {
  return (
    pathname === "/api/plugin-authorizations/requests" ||
    /^\/api\/plugin-authorizations\/requests\/[^/]+\/claim$/.test(pathname)
  );
}

function isPublicPath(pathname: string) {
  if (pathname === "/login") return true;
  if (pathname === MEDIA_LIBRARY_MAINTENANCE_PAGE) return true;
  if (pathname === "/forgot-password") return true;
  if (pathname === "/reset-password") return true;
  if (pathname === "/privacy" || pathname.startsWith("/privacy/")) return true;
  if (PUBLIC_PATHS.has(pathname)) return true;
  if (PUBLIC_PREFIXES.some((prefix) => pathname.startsWith(prefix))) return true;
  return /\.[a-zA-Z0-9]+$/.test(pathname);
}

function isPluginProtocolRequest(request: NextRequest) {
  const { pathname } = request.nextUrl;
  if (pathname === "/api/collect/status") return true;
  if (PLUGIN_PROTOCOL_PATHS.includes(pathname)) return true;
  if (isPluginAuthorizationRequestPath(pathname)) return true;
  return false;
}

function isSystemJobRequest(request: NextRequest) {
  return [
    ...MAINTENANCE_AWARE_SCHEDULER_PATHS,
    "/api/scheduler/snapshot",
    "/api/scheduler/audit",
    "/api/scheduler/execution-queue-audit",
    "/api/scheduler/execution-status-audit",
    "/api/scheduler/execution-task-split-audit",
    "/api/feishu/daily-boom-digest",
    "/api/data-foundation/production-validation",
    "/api/data-foundation/production-validation/daily",
    "/api/data-foundation/production-validation/status",
    "/api/data-foundation/quality-issues",
  ].includes(request.nextUrl.pathname);
}

function isMaintenanceAwareSystemRoute(pathname: string) {
  return MAINTENANCE_AWARE_SCHEDULER_PATHS.has(pathname);
}

function isReadOnlyRecoveryRequest(request: NextRequest) {
  if (!["GET", "HEAD"].includes(request.method)) return false;
  const { pathname } = request.nextUrl;
  return (
    !pathname.startsWith("/api/") ||
    isPublicPath(pathname) ||
    pathname.startsWith("/api/local-media/") ||
    REVIEWED_BUSINESS_READ_RECOVERY_PATHS.has(pathname)
  );
}

function isManualCoverUploadRecoveryRequest(request: NextRequest) {
  return request.method === "POST" && request.nextUrl.pathname === MANUAL_COVER_UPLOAD_PATH;
}

function isMaintenanceSafeAuthSessionRequest(request: NextRequest) {
  return request.method === "POST" && MAINTENANCE_SAFE_AUTH_SESSION_PATHS.has(request.nextUrl.pathname);
}

export async function proxy(request: NextRequest) {
  const { pathname, search } = request.nextUrl;
  const isPluginRequest = isPluginProtocolRequest(request);
  const maintenance = getMediaLibraryMaintenanceState();

  if (maintenance.active && pathname !== MEDIA_LIBRARY_MAINTENANCE_PAGE) {
    if (
      maintenance.mode === "hard_cutover"
      && isAuthorizedV2CutoverCanaryRequest(request)
    ) {
      return NextResponse.next();
    }
    if (
      [MEDIA_LIBRARY_READ_ONLY_RECOVERY_MODE, MEDIA_LIBRARY_MANUAL_WRITE_RECOVERY_MODE].includes(maintenance.mode ?? "") &&
      isMaintenanceSafeAuthSessionRequest(request)
    ) {
      return NextResponse.next();
    }
    if (
      [MEDIA_LIBRARY_READ_ONLY_RECOVERY_MODE, MEDIA_LIBRARY_MANUAL_WRITE_RECOVERY_MODE].includes(maintenance.mode ?? "") &&
      isReadOnlyRecoveryRequest(request)
    ) {
      return NextResponse.next();
    }
    if (
      maintenance.mode === MEDIA_LIBRARY_MANUAL_WRITE_RECOVERY_MODE &&
      isManualCoverUploadRecoveryRequest(request)
    ) {
      return NextResponse.next();
    }
    if (pathname.startsWith("/api/")) {
      if (!isMaintenanceAwareSystemRoute(pathname)) {
        return NextResponse.json(
          {
            error: "媒体架构迁移维护中，暂不接收写入。",
            code: maintenance.code,
          },
          { status: 503 },
        );
      }
    } else if (!isPublicPath(pathname)) {
      const url = request.nextUrl.clone();
      url.pathname = MEDIA_LIBRARY_MAINTENANCE_PAGE;
      url.search = "";
      return NextResponse.redirect(url);
    }
  }

  if (
    isPublicPath(pathname) ||
    isPluginRequest ||
    isSystemJobRequest(request)
  ) {
    return NextResponse.next();
  }

  if (pathname.startsWith("/api/") && !hasSessionCookie(request)) {
    return NextResponse.json(
      { error: "请先登录内容工作台。" },
      { status: 401 }
    );
  }

  const session = await auth.api.getSession({
    headers: request.headers,
  });

  if (session) return NextResponse.next();

  if (pathname.startsWith("/api/")) {
    return NextResponse.json(
      { error: "请先登录内容工作台。" },
      { status: 401 }
    );
  }

  const url = request.nextUrl.clone();
  url.pathname = "/login";
  url.search = "";
  url.searchParams.set("next", `${pathname}${search}`);
  return NextResponse.redirect(url);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
