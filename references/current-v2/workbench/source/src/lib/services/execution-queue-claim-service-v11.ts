/**
 * execution-queue-claim-service-v11.ts
 *
 * V1.1 采集架构重构 — Agent D (Execution Queue)
 *
 * 目标：让 ExecutionQueueEntry 成为执行状态权威（手册第 4.5 节、第 5.4 节、第 6 节）。
 *
 * V1.1 说明（重要）：
 * - ExecutionJob + ExecutionQueueEntry 是唯一执行队列。
 * - 插件只通过 `/api/execution-stations/sync` 领取 reservation、start_job、续租、
 *   commit_raw_snapshot 和 release_job。
 * - 旧 CollectionTask 领取 / lease / ingest 路径只返回 410，不再参与执行。
 *
 * 本文件实现的功能（手册 5.4 / 5.5 / 6.2 / 6.5）：
 *   1. claimNextExecutionJobs — 工位按 lane 容量领取任务
 *   2. releaseExpiredReservations — reserved 超过 startBefore 自动释放回 queued
 *   3. releaseExpiredLeases — in_progress lease 过期标记 stale_result 并释放
 *   4. startJob — reserved → in_progress（手册 5.5 start_job operation）
 *   5. progressLease — 续租 + 更新进度
 *
 * 命名后缀 `-v11` 是为了保留重构语义；它不是兼容分支。
 *
 * 不变量（手册第 3 节）：
 *   - #4 同一 ExecutionJob 同一时刻只能有一个有效执行权
 *   - #6 旧 leaseEpoch / leaseToken 不能推进 ExecutionJob 状态
 *   - 同一账号同一时刻只允许一个 active job（partial unique index 兜底）
 *
 * SQL 约定：
 *   - 所有 SQL 用 prisma.$executeRaw`...` / $queryRaw 模板字符串参数化（防注入）
 *   - 复用现有 reserveNextExecutionTask 的 CTE + FOR UPDATE SKIP LOCKED 模式
 *     （见本文件的 claimNextExecutionJobs 实现）
 */

import { randomUUID } from "node:crypto";

import { Prisma } from "@/lib/prisma-client";
import { prisma } from "@/lib/db";
import {
  capabilityCanRun,
} from "@/lib/execution-task-routing";
import {
  markExecutionStationMailboxesChanged,
} from "@/lib/services/execution-station-mailbox-service";
import { evaluatePlatformAccountClaimEligibility } from "@/lib/services/execution-platform-account-eligibility";
import { recordExecutionLogEvent } from "@/lib/services/execution-log-service";
import {
  resolveExecutionIdentity,
  type ExecutionIdentityKind,
} from "@/lib/services/execution-identity";
import { resolveRunnableXhsNoteDetailTarget } from "@/lib/xhs-execution-target-url";

// ---------------------------------------------------------------------------
// 类型
// ---------------------------------------------------------------------------

type QueueDbClient = Prisma.TransactionClient | typeof prisma;
type QueueRootClient = typeof prisma;

function supportsQueueTransaction(db: QueueDbClient): db is QueueRootClient {
  return "$transaction" in db && typeof db.$transaction === "function";
}

async function withinQueueTransaction<T>(
  db: QueueDbClient,
  callback: (tx: Prisma.TransactionClient) => Promise<T>,
): Promise<T> {
  if (supportsQueueTransaction(db)) {
    return db.$transaction((tx) => callback(tx));
  }
  return callback(db);
}

/** 手册 5.4 claim 输入。 */
export type ClaimCapacity = {
  stationId: string;
  workspaceId: string;
  /** 工位已登记的采集能力，claim 前会按每条任务的 collectionProfile 再过滤一次。 */
  stationCapabilities: string[];
  /** 工位当前持有的有效账号（手册 5.4 原则 5：未绑定健康账号不允许 reserved）。 */
  healthyAccounts: Array<{
    platform: string;
    platformAccountId: string;
    /**
     * `platform_account` means a real account shared across workstations;
     * `browser_session` means a station-local browser session.
     */
    executionIdentityKind?: ExecutionIdentityKind | null;
  }>;
  lanes: Array<{
    /** 手册 6.1 lane 名：manual_hot | monitor_checkpoint | monitor_patrol | data_sync | governance | archive | comments。 */
    lane: string;
    platform: string;
    /** 当前队列里属于本 lane 的 reserved 任务预计剩余执行秒数。 */
    remainingWorkSeconds: number;
    /** 目标工作时长（手册 6.3 targetWorkSeconds）。 */
    targetWorkSeconds: number;
    /** 单 lane 最大预留任务数（手册 6.3 maxReservedTasks）。 */
    maxReservedTasks: number;
    /**
     * lane mailbox 变化时，本轮需要主动尝试补货。
     * 这解决“新任务已经进队列，但 30% 补货线把它再次挡住”的冲突。
     */
    ignoreReplenishThreshold?: boolean;
  }>;
};

/** claimNextExecutionJobs 单 lane 返回。 */
export type ClaimedReservation = {
  jobId: string;
  reserveToken: string;
  reservationEpoch: number;
  startBefore: Date;
  reservedUntil: Date;
  lane: string;
  platform: string;
  platformAccountId: string;
  executionIdentityKey: string;
  /** 任务规格预览，给插件用。Agent E 会拼成完整 taskSpec。 */
  taskSpecPreview: {
    id: string;
    platform: string;
    platformAccountId: string;
    lane: string;
    jobType: string;
    taskType: string | null;
    source: string | null;
    taskStrategy: string | null;
    collectionProfile: string;
    targetKey: string;
    target: string | null;
    requiredFields: unknown;
    payload: unknown;
  };
};

export type ClaimNextExecutionJobsResult = {
  stationId: string;
  reservations: ClaimedReservation[];
  /** 按 lane 维度的跳过原因（无任务、容量满、无健康账号等）。 */
  skippedLanes: Array<{
    lane: string;
    platform: string;
    reason: ClaimSkipReason;
  }>;
};

type ClaimSkipReason =
  | "lane_below_threshold" // remainingWorkSeconds >= targetWorkSeconds * 0.3，不补货
  | "no_healthy_account" // 该 platform 无健康账号
  | "capability_mismatch" // 工位没有该 lane 内任何 profile 对应的执行能力
  | "execution_identity_busy" // 所有可用执行身份都已有 active job
  | "max_reserved_reached" // 当前 reserved 数已达 maxReservedTasks
  | "no_candidate" // 队列无可领任务
  | "claim_empty"; // 候选 SQL 命中 0 条（被其他工位抢光）

type CandidateJob = {
  id: string;
  jobId: string;
  estimatedSeconds: number;
  jobType: string;
  collectionProfile: string;
  targetKey: string;
  target: string | null;
  originalTaskType: string | null;
  originalSource: string | null;
  originalTaskStrategy: string | null;
  requiredFields: unknown;
  payload: unknown;
};

const CLAIM_KNOWN_COLLECTION_PROFILES = [
  "list_scan",
  "note_detail",
  "note_full",
  "comment_probe",
  "author_profile",
  "author_links",
] as const;

const PROFILE_SAMPLE_FIELDS: Record<string, string[]> = {
  list_scan: ["noteId", "likeCount"],
  note_detail: ["content"],
  note_full: ["content", "mediaUrls", "comments"],
  comment_probe: ["comments"],
  author_profile: ["authorId"],
  author_links: ["noteId", "runnableDetailUrl"],
};

/** startJob 输入（手册 5.5 start_job operation）。 */
export type StartJobInput = {
  jobId: string;
  reserveToken: string;
  reservationEpoch: number;
  stationId: string;
  startedAt?: Date;
  now?: Date;
};

export type StartJobResult =
  | {
      status: "started";
      attemptId: string;
      captureId: string;
      executionPlanVersion: string;
      leaseToken: string;
      leaseEpoch: number;
      leaseExpiresAt: Date;
    }
  | {
      status: "rejected";
      reason:
        | "job_not_found"
        | "status_not_reserved"
        | "reserve_token_mismatch"
        | "reservation_epoch_mismatch"
        | "start_before_expired"
        | "station_mismatch"
        | "account_unhealthy"
        | "execution_plan_version_missing";
    };

/** progressLease 输入。 */
export type ProgressLeaseInput = {
  jobId: string;
  leaseToken: string;
  leaseEpoch: number;
  /** 0-100 进度百分比，可选。 */
  progress?: number;
  /** 阶段标签，可选。 */
  stage?: string;
  now?: Date;
};

export type ProgressLeaseResult =
  | {
      status: "renewed";
      leaseExpiresAt: Date;
      leaseEpoch: number;
    }
  | {
      status: "rejected";
      reason: "job_not_found" | "lease_token_mismatch" | "lease_epoch_mismatch" | "status_not_in_progress";
    };

// ---------------------------------------------------------------------------
// lane 配额 + TTL（手册第 6.2、6.5 节）
// ---------------------------------------------------------------------------

/**
 * 手册 6.2 lane 默认容量占比（10 工位下）。
 * 仅作为参考常量；实际 lane 调度由 ExecutionJob 创建时的 lane 字段决定，
 * claim 阶段只按 lane + platform 领取，不按配额阻塞（配额在 ExecutionJob 创建时由 Agent C 拆分）。
 */
export const LANE_QUOTA = {
  manual_hot: 0.2,
  monitor_checkpoint: 0.3,
  monitor_patrol: 0.25,
  data_sync: 0.1,
  governance: 0.1,
  archive: 0.05,
  comments: 0.05,
} as const;

export type LaneName = keyof typeof LANE_QUOTA;

const CLAIM_LANE_PRIORITY: Record<string, number> = {
  manual_hot: 0,
  archive: 10,
  governance: 20,
  comments: 30,
  data_sync: 40,
  monitor_patrol: 50,
  monitor_checkpoint: 60,
};

export function sortClaimLanesByDispatchPriority<T extends { lane: string }>(
  lanes: T[]
): T[] {
  return [...lanes].sort((left, right) => {
    const leftPriority = CLAIM_LANE_PRIORITY[left.lane] ?? 100;
    const rightPriority = CLAIM_LANE_PRIORITY[right.lane] ?? 100;
    return leftPriority - rightPriority;
  });
}

/**
 * 手册 6.5 预约 / 租约 TTL（按 collectionProfile）。
 * 单位：毫秒。
 */
export const PROFILE_TTL_MS: Record<
  string,
  { reservationTtlMs: number; leaseTtlMs: number; progressIntervalMs: number }
> = {
  list_scan: { reservationTtlMs: 5 * 60_000, leaseTtlMs: 2 * 60_000, progressIntervalMs: 30_000 },
  note_detail: { reservationTtlMs: 10 * 60_000, leaseTtlMs: 5 * 60_000, progressIntervalMs: 30_000 },
  note_full: { reservationTtlMs: 20 * 60_000, leaseTtlMs: 10 * 60_000, progressIntervalMs: 60_000 },
  comment_probe: { reservationTtlMs: 15 * 60_000, leaseTtlMs: 8 * 60_000, progressIntervalMs: 30_000 },
  author_links: { reservationTtlMs: 20 * 60_000, leaseTtlMs: 10 * 60_000, progressIntervalMs: 60_000 },
};

const DEFAULT_RESERVATION_TTL_MS = 5 * 60_000;
const DEFAULT_LEASE_TTL_MS = 5 * 60_000;

/** 手册 6.4 claim 触发条件：remainingWorkSeconds < targetWorkSeconds * 0.3 才补货。 */
const CLAIM_REPLENISH_THRESHOLD = 0.3;

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

function ttlForProfile(profile: string | null | undefined) {
  if (profile && profile in PROFILE_TTL_MS) {
    return PROFILE_TTL_MS[profile];
  }
  return {
    reservationTtlMs: DEFAULT_RESERVATION_TTL_MS,
    leaseTtlMs: DEFAULT_LEASE_TTL_MS,
    progressIntervalMs: 30_000,
  };
}

function profileCanRun(
  capabilities: string[],
  platform: string,
  profile: string,
  requiredFields: unknown
) {
  return capabilityCanRun(capabilities, {
    platform,
    collectionProfile: profile,
    requiredFields,
  });
}

function supportedProfilesForLane(input: {
  capabilities: string[];
  platform: string;
}) {
  return CLAIM_KNOWN_COLLECTION_PROFILES.filter((profile) =>
    profileCanRun(
      input.capabilities,
      input.platform,
      profile,
      PROFILE_SAMPLE_FIELDS[profile] ?? []
    )
  );
}

function candidateCanRun(input: {
  capabilities: string[];
  platform: string;
  candidate: Pick<CandidateJob, "collectionProfile" | "requiredFields">;
}) {
  return profileCanRun(
    input.capabilities,
    input.platform,
    input.candidate.collectionProfile,
    input.candidate.requiredFields
  );
}

function objectPayloadOrNull(value: unknown) {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  if (typeof value !== "string" || !value.trim()) return null;
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function isXhsNoteDetailCandidate(candidate: {
  platform?: string;
  targetKey: string;
  collectionProfile: string;
  originalTaskStrategy: string | null;
  payload: unknown;
}) {
  if (candidate.platform !== "xhs") return false;
  if (!candidate.targetKey.startsWith("xhs:note:")) return false;
  const payload = objectPayloadOrNull(candidate.payload);
  const targetPageType =
    typeof payload?.targetPageType === "string" ? payload.targetPageType.trim() : "";
  return (
    candidate.collectionProfile === "note_full" ||
    candidate.collectionProfile === "note_detail" ||
    candidate.originalTaskStrategy === "detail_probe" ||
    targetPageType === "detail"
  );
}

function taskSpecTargetForCandidate(candidate: {
  platform?: string;
  targetKey: string;
  target: string | null;
  collectionProfile: string;
  originalTaskStrategy: string | null;
  payload: unknown;
}) {
  if (!isXhsNoteDetailCandidate(candidate)) return candidate.target;
  return resolveRunnableXhsNoteDetailTarget({
    platform: candidate.platform,
    targetKey: candidate.targetKey,
    target: candidate.target,
    payload: objectPayloadOrNull(candidate.payload),
  });
}

function nowOr(inputNow: Date | undefined): Date {
  return inputNow ?? new Date();
}

/**
 * 同一执行身份同一时刻只允许一个 active job 的业务层预检（手册 5.4 原则 3）。
 * partial unique index `idx_execution_queue_execution_identity_active_unique` 是兜底，
 * 业务层先查降低冲突概率（手册 1.4 V1.1 修正点 2 的 TOCTOU 防护层级）。
 *
 * `reservedExecutionIdentityKey` 会将真实账号保持为跨工位互斥，同时让
 * `runtime:<platform>` 这类浏览器会话占位值只在所属工位内互斥。
 */
async function executionIdentityHasActiveJob(
  db: QueueDbClient,
  platform: string,
  executionIdentityKey: string,
  platformAccountId: string,
): Promise<boolean> {
  const rows = await db.$queryRaw<Array<{ count: bigint }>>(Prisma.sql`
    SELECT COUNT(*)::bigint AS count
    FROM "ExecutionQueueEntry"
    WHERE "platform" = ${platform}
      AND (
        "reservedExecutionIdentityKey" = ${executionIdentityKey}
        -- 迁移已回填的旧 active 行本应都有 execution identity；保留这个
        -- fallback 是为了在异常回滚/半升级时宁可保守地继续锁住旧账号，不能
        -- 让一个空 identity 的历史 reservation 与新 reservation 并发执行。
        OR (
          "reservedExecutionIdentityKey" IS NULL
          AND "reservedPlatformAccountId" = ${platformAccountId}
        )
      )
      AND "status" IN ('reserved'::"ExecutionQueueEntryStatus", 'in_progress'::"ExecutionQueueEntryStatus")
  `);
  return Number(rows[0]?.count ?? 0) > 0;
}

/**
 * 更新 lane 级 mailbox（手册 4.16：reserved 过期释放 / queue 回 queued 都要 bump lane version）。
 *
 * 现有 markExecutionStationMailboxesChanged 只处理 station 级；
 * lane 级 mailbox 是 Agent A 新建的模型，目前无封装。这里直接 upsert。
 */
async function markExecutionLaneMailboxChanged(input: {
  db: QueueDbClient;
  workspaceId: string;
  platform: string;
  lane: string;
  delta?: number;
}): Promise<void> {
  if (!("executionLaneMailbox" in input.db)) return;
  await input.db.executionLaneMailbox.upsert({
    where: {
      workspaceId_platform_lane: {
        workspaceId: input.workspaceId,
        platform: input.platform,
        lane: input.lane,
      },
    },
    create: {
      workspaceId: input.workspaceId,
      platform: input.platform,
      lane: input.lane,
      version: 1,
      pendingCount: Math.max(0, input.delta ?? 1),
      lastChangedAt: new Date(),
    },
    update: {
      version: { increment: 1 },
      pendingCount: { increment: input.delta ?? 1 },
      lastChangedAt: new Date(),
    },
  });
}

// ---------------------------------------------------------------------------
// 功能 1：claimNextExecutionJobs（手册 5.4）
// ---------------------------------------------------------------------------

/**
 * 工位按 lane 容量领取任务。
 *
 * 实现要点：
 * 1. 按 lane 逐个评估（手册 5.4 原则 1：不全局混领）。
 * 2. remainingWorkSeconds >= targetWorkSeconds * 0.3 时不补货（手册 6.4）。
 * 3. 同一账号同一时刻只允许一个 active job（手册 5.4 原则 3）。
 * 4. claim 时同时绑定 stationId + platformAccountId（手册 5.4 原则 4）。
 * 5. 未绑定健康账号不允许 reserved（手册 5.4 原则 5）。
 * 6. 同一 platform 同一 lane 内只查一个候选账号（避免在 claim SQL 内做复杂账号匹配）。
 *
 * claim SQL 复用现有 reserveNextExecutionTask（execution-queue-service.ts:111-168）
 * 的 CTE + FOR UPDATE SKIP LOCKED 模式，但写入 V1.1 新字段：
 * reservedPlatformAccountId / reservationEpoch / startBefore。
 *
 * 注：由于 ExecutionQueueEntryStatus enum 当前只有 6 个值（queued/reserved/in_progress/
 * succeeded/failed/cancelled），raw_committed/writeback_pending/expired 等中间态由
 * ExecutionJob.status（String）承载；ExecutionQueueEntry.status 仅维护到 reserved / in_progress。
 */
export async function claimNextExecutionJobs(
  input: ClaimCapacity & { db?: QueueRootClient; now?: Date }
): Promise<ClaimNextExecutionJobsResult> {
  const db = input.db ?? prisma;
  const now = nowOr(input.now);

  const healthyAccountsByPlatform = new Map<string, Array<{
    platformAccountId: string;
    executionIdentityKey: string;
  }>>();
  for (const account of input.healthyAccounts) {
    const list = healthyAccountsByPlatform.get(account.platform) ?? [];
    const identity = resolveExecutionIdentity({
      platform: account.platform,
      stationId: input.stationId,
      platformAccountId: account.platformAccountId,
      kind: account.executionIdentityKind,
    });
    list.push({
      platformAccountId: account.platformAccountId,
      executionIdentityKey: identity.key,
    });
    healthyAccountsByPlatform.set(account.platform, list);
  }

  const reservations: ClaimedReservation[] = [];
  const skippedLanes: ClaimNextExecutionJobsResult["skippedLanes"] = [];

  for (const lanePlan of sortClaimLanesByDispatchPriority(input.lanes)) {
    const accounts = healthyAccountsByPlatform.get(lanePlan.platform) ?? [];
    if (accounts.length === 0) {
      skippedLanes.push({ lane: lanePlan.lane, platform: lanePlan.platform, reason: "no_healthy_account" });
      continue;
    }

    // 手册 6.4：remainingWorkSeconds >= targetWorkSeconds * 0.3 时不补货。
    // 但如果 lane mailbox 已变化，本轮代表“有新任务或释放任务待处理”，需要主动尝试一次。
    if (
      !lanePlan.ignoreReplenishThreshold &&
      lanePlan.remainingWorkSeconds >= lanePlan.targetWorkSeconds * CLAIM_REPLENISH_THRESHOLD
    ) {
      skippedLanes.push({
        lane: lanePlan.lane,
        platform: lanePlan.platform,
        reason: "lane_below_threshold",
      });
      continue;
    }

    // 计算还需要补多少秒 / 多少条任务
    const secondsToReplenish = lanePlan.ignoreReplenishThreshold
      ? Math.max(30, lanePlan.targetWorkSeconds - lanePlan.remainingWorkSeconds)
      : Math.max(0, lanePlan.targetWorkSeconds - lanePlan.remainingWorkSeconds);
    if (secondsToReplenish <= 0) {
      skippedLanes.push({
        lane: lanePlan.lane,
        platform: lanePlan.platform,
        reason: "max_reserved_reached",
      });
      continue;
    }

    // 按 estimatedSeconds 平均估算补货条数（保守，避免一次抢光）
    // 实际领取数受 maxReservedTasks 限制
    let budgetByTime = Number.MAX_SAFE_INTEGER;
    if (lanePlan.targetWorkSeconds > 0) {
      // 用 lane.targetWorkSeconds / maxReservedTasks 作为平均任务耗时上限估算
      const avgEstimated = Math.max(30, Math.ceil(lanePlan.targetWorkSeconds / Math.max(1, lanePlan.maxReservedTasks)));
      budgetByTime = Math.max(1, Math.ceil(secondsToReplenish / avgEstimated));
    }
    const limit = Math.min(lanePlan.maxReservedTasks, budgetByTime);
    if (limit <= 0) {
      skippedLanes.push({
        lane: lanePlan.lane,
        platform: lanePlan.platform,
        reason: "max_reserved_reached",
      });
      continue;
    }

    const laneReservations = await claimLaneJobs({
      db,
      stationId: input.stationId,
      workspaceId: input.workspaceId,
      stationCapabilities: input.stationCapabilities,
      lane: lanePlan.lane,
      platform: lanePlan.platform,
      candidateAccounts: accounts,
      limit,
      now,
    });

    if (laneReservations.reservations.length === 0) {
      skippedLanes.push({
        lane: lanePlan.lane,
        platform: lanePlan.platform,
        reason: laneReservations.reason ?? (laneReservations.hadCandidate ? "claim_empty" : "no_candidate"),
      });
      continue;
    }

    reservations.push(...laneReservations.reservations);
  }

  // 整批 claim 完成后，bump station mailbox（唤醒推迟）
  if (reservations.length > 0) {
    try {
      await markExecutionStationMailboxesChanged({
        db,
        stationId: input.stationId,
        workspaceId: input.workspaceId,
        wakeReason: "claim_fulfilled",
      });
    } catch {
      // mailbox bump 失败不影响 claim 成功
    }
  }

  return { stationId: input.stationId, reservations, skippedLanes };
}

/**
 * 单 lane 内的 claim 执行。
 *
 * 由于"同一账号同时刻只允许一个 active job"的约束需要在领到 jobId 后逐个绑定账号，
 * 我们不直接在 claim CTE 里写 account 关联，而是：
 *   - 应用层先用 candidate SQL 选候选 jobId（按 priority/availableAt）
 *   - 应用层为每个 candidate 匹配一个空闲健康账号
 *   - 单条原子 UPDATE ... WHERE id=$id AND status='queued' AND reserveToken IS NULL
 *     绑定 station + account
 *
 * 并发安全保证：
 *   - 真正的 race 防护在 UPDATE 语句的 WHERE 子句（status='queued' AND reserveToken IS NULL）
 *   - 如果两个工位同时选中同一 candidate，UPDATE 受影响行数 = 0 的那个会感知到，跳过
 *   - partial unique index `idx_execution_queue_execution_identity_active_unique` 兜底
 *     （手册 1.4 V1.1 修正点 2），同一执行身份同时刻第二个 reserved 触发唯一冲突被跳过
 *
 * 注意：返回的 reservedPlatformAccountId 是真实写入 QueueEntry 的账号。
 */
async function claimLaneJobs(input: {
  db: QueueRootClient;
  stationId: string;
  workspaceId: string;
  stationCapabilities: string[];
  lane: string;
  platform: string;
  candidateAccounts: Array<{
    platformAccountId: string;
    executionIdentityKey: string;
  }>;
  limit: number;
  now: Date;
}): Promise<{
  reservations: ClaimedReservation[];
  hadCandidate: boolean;
  reason?: Extract<ClaimSkipReason, "capability_mismatch" | "execution_identity_busy">;
}> {
  // 跟踪本批次已使用的执行身份（同一批次内不重复绑定）。
  const usedExecutionIdentities = new Set<string>();

  const supportedProfiles = supportedProfilesForLane({
    capabilities: input.stationCapabilities,
    platform: input.platform,
  });
  if (supportedProfiles.length === 0) {
    return { reservations: [], hadCandidate: false, reason: "capability_mismatch" };
  }

  // 候选账号池：先过滤掉已有 active job 的执行身份（手册 5.4 原则 3）。
  const freeAccounts: Array<{ platformAccountId: string; executionIdentityKey: string }> = [];
  for (const account of input.candidateAccounts) {
    if (usedExecutionIdentities.has(account.executionIdentityKey)) continue;
    const hasActive = await executionIdentityHasActiveJob(
      input.db,
      input.platform,
      account.executionIdentityKey,
      account.platformAccountId,
    );
    if (!hasActive) freeAccounts.push(account);
  }
  if (freeAccounts.length === 0) {
    return { reservations: [], hadCandidate: false, reason: "execution_identity_busy" };
  }

  // 候选 jobId 列表：用 SQL 选 limit + freeAccounts.length 条候选（多选几条避免账号不够）
  // 不持锁；真正的 race 防护在 UPDATE WHERE 子句。
  const candidateRows = await input.db.$queryRaw<CandidateJob[]>(Prisma.sql`
    SELECT
      qe."id",
      qe."jobId",
      qe."estimatedSeconds",
      j."jobType",
      j."collectionProfile",
      j."targetKey",
      j."target",
      j."originalTaskType",
      j."originalSource",
      j."originalTaskStrategy",
      j."requiredFields",
      j."payload"
    FROM "ExecutionQueueEntry" qe
    INNER JOIN "ExecutionJob" j ON j."id" = qe."jobId"
    WHERE qe."status" = 'queued'::"ExecutionQueueEntryStatus"
      AND (qe."availableAt" IS NULL OR qe."availableAt" <= ${input.now})
      AND (qe."expiresAt" IS NULL OR qe."expiresAt" > ${input.now})
      AND qe."workspaceId" = ${input.workspaceId}
      AND qe."platform" = ${input.platform}
      AND qe."lane" = ${input.lane}
      AND j."collectionProfile" IN (${Prisma.join(supportedProfiles)})
    ORDER BY qe."priority" DESC, qe."availableAt" ASC, qe."id" ASC
    LIMIT ${input.limit + freeAccounts.length}
    FOR UPDATE OF qe SKIP LOCKED
  `);

  if (candidateRows.length === 0) {
    return { reservations: [], hadCandidate: false };
  }

  const runnableCandidates = candidateRows.filter((candidate) =>
    candidateCanRun({
      capabilities: input.stationCapabilities,
      platform: input.platform,
      candidate,
    })
  );
  if (runnableCandidates.length === 0) {
    return { reservations: [], hadCandidate: true, reason: "capability_mismatch" };
  }

  // 逐条尝试绑定账号 + 写 reservation 字段
  // 真正的原子保护在 claimLaneJobsAtomic 的 UPDATE WHERE 子句
  return await claimLaneJobsAtomic({
    db: input.db,
    stationId: input.stationId,
    workspaceId: input.workspaceId,
    lane: input.lane,
    platform: input.platform,
    freeAccounts,
    limit: input.limit,
    now: input.now,
    preselectedCandidates: runnableCandidates.map((row) => ({
      id: row.id,
      jobId: row.jobId,
      estimatedSeconds: row.estimatedSeconds,
      jobType: row.jobType,
      collectionProfile: row.collectionProfile,
      targetKey: row.targetKey,
      target: row.target,
      originalTaskType: row.originalTaskType,
      originalSource: row.originalSource,
      originalTaskStrategy: row.originalTaskStrategy,
      requiredFields: row.requiredFields,
      payload: row.payload,
    })),
  });
}

/**
 * 真正的 claim 逻辑：在单个事务内完成 SELECT ... FOR UPDATE SKIP LOCKED + UPDATE。
 *
 * 这是手册 5.4 claim SQL 结构的落地实现，与现有 reserveNextExecutionTask
 * （execution-queue-service.ts:111-168）的 CTE + FOR UPDATE SKIP LOCKED 模式一致，
 * 但写入 V1.1 新字段：
 *   - reservedPlatformAccountId
 *   - reservationEpoch = reservationEpoch + 1
 *   - startBefore = now + reservationTtl
 *   - reservedUntil = now + reservationTtl
 *
 * 同时维护 ExecutionJob.status: queued → reserved
 * （手册 4.5 状态机收敛到 ExecutionQueueEntry + ExecutionJob）
 */
async function claimLaneJobsAtomic(input: {
  db: QueueRootClient;
  stationId: string;
  workspaceId: string;
  lane: string;
  platform: string;
  freeAccounts: Array<{
    platformAccountId: string;
    executionIdentityKey: string;
  }>;
  limit: number;
  now: Date;
  preselectedCandidates: Array<{
    id: string;
    jobId: string;
    estimatedSeconds: number;
    jobType: string;
    collectionProfile: string;
    targetKey: string;
    target: string | null;
    originalTaskType: string | null;
    originalSource: string | null;
    originalTaskStrategy: string | null;
    requiredFields: unknown;
    payload: unknown;
  }>;
}): Promise<{ reservations: ClaimedReservation[]; hadCandidate: boolean }> {
  const reservations: ClaimedReservation[] = [];
  const usedExecutionIdentities = new Set<string>();

  // 候选已经按 priority 排序，逐条尝试；账号用完即停
  for (const candidate of input.preselectedCandidates) {
    if (reservations.length >= input.limit) break;
    if (usedExecutionIdentities.size >= input.freeAccounts.length) break;

    // 选下一个未使用的账号
    const nextAccount = input.freeAccounts.find(
      (account) => !usedExecutionIdentities.has(account.executionIdentityKey),
    );
    if (!nextAccount) break;

    const ttl = ttlForProfile(candidate.collectionProfile);
    const reservedUntil = new Date(input.now.getTime() + ttl.reservationTtlMs);
    const startBefore = new Date(input.now.getTime() + ttl.reservationTtlMs);
    const reserveToken = randomUUID();

    // V1.1（2026-06-29 事务修复）：QueueEntry UPDATE + ExecutionJob UPDATE
    // + mailbox bump 包裹在同一个 $transaction 中，防止状态分裂（QueueEntry 已
    // reserved 但 Job 仍是 queued）。QueueEntry UPDATE 的 WHERE 子句天然防多工位并发。
    let updatedRow: { reservationEpoch: number } | undefined;
    try {
      const result = await input.db.$transaction(async (tx) => {
        const rows = await tx.$queryRaw<Array<{ reservationEpoch: number }>>(Prisma.sql`
          UPDATE "ExecutionQueueEntry"
          SET
            "status" = 'reserved'::"ExecutionQueueEntryStatus",
            "reservedByStationId" = ${input.stationId},
            "reservedPlatformAccountId" = ${nextAccount.platformAccountId},
            "reservedExecutionIdentityKey" = ${nextAccount.executionIdentityKey},
            "reserveToken" = ${reserveToken},
            "reservationEpoch" = "reservationEpoch" + 1,
            "reservedUntil" = ${reservedUntil},
            "startBefore" = ${startBefore},
            "updatedAt" = ${input.now}
          WHERE "id" = ${candidate.id}
            AND "status" = 'queued'::"ExecutionQueueEntryStatus"
            AND "reserveToken" IS NULL
          RETURNING "reservationEpoch"
        `);
        if (!rows[0]) return null;

        // 同步 ExecutionJob.status（同一事务内，防崩溃导致分裂）
        await tx.executionJob.updateMany({
          where: { id: candidate.jobId, status: "queued" },
          data: { status: "reserved", updatedAt: input.now },
        });

        // bump lane mailbox（同一事务内）
        await markExecutionLaneMailboxChanged({
          db: tx,
          workspaceId: input.workspaceId,
          platform: input.platform,
          lane: input.lane,
          delta: -1,
        }).catch((error) => {
          console.warn("[swallowed]", "claim lane mailbox update", error);
        });

        return { reservationEpoch: rows[0].reservationEpoch };
      }, {
        maxWait: 2_000,
        timeout: 8_000,
      });

      updatedRow = result ?? undefined;
    } catch (error) {
      if (isUniqueViolation(error)) {
        usedExecutionIdentities.add(nextAccount.executionIdentityKey);
        continue;
      }
      throw error;
    }

    if (!updatedRow) {
      continue;
    }

    usedExecutionIdentities.add(nextAccount.executionIdentityKey);

    await recordExecutionLogEvent({
      db: input.db,
      workspaceId: input.workspaceId,
      jobId: candidate.jobId,
      queueEntryId: candidate.id,
      stationId: input.stationId,
      platform: input.platform,
      lane: input.lane,
      source: candidate.originalSource,
      eventType: "execution_queue.reserved",
      stage: "reservation",
      status: "accepted",
      title: "任务已派给插件工位",
      message: "系统已经把任务预留给一个在线工位，等待插件确认开始执行。",
      details: {
        platformAccountId: nextAccount.platformAccountId,
        executionIdentityKey: nextAccount.executionIdentityKey,
        reservationEpoch: updatedRow.reservationEpoch,
        startBefore: startBefore.toISOString(),
        reservedUntil: reservedUntil.toISOString(),
        collectionProfile: candidate.collectionProfile,
        taskType: candidate.originalTaskType,
        targetKey: candidate.targetKey,
      },
    });

    reservations.push({
      jobId: candidate.jobId,
      reserveToken,
      reservationEpoch: updatedRow.reservationEpoch,
      startBefore,
      reservedUntil,
      lane: input.lane,
      platform: input.platform,
      platformAccountId: nextAccount.platformAccountId,
      executionIdentityKey: nextAccount.executionIdentityKey,
      taskSpecPreview: {
        id: candidate.jobId,
        platform: input.platform,
        platformAccountId: nextAccount.platformAccountId,
        lane: input.lane,
        jobType: candidate.jobType,
        taskType: candidate.originalTaskType,
        source: candidate.originalSource,
        taskStrategy: candidate.originalTaskStrategy,
        collectionProfile: candidate.collectionProfile,
        targetKey: candidate.targetKey,
        target: taskSpecTargetForCandidate({
          platform: input.platform,
          targetKey: candidate.targetKey,
          target: candidate.target,
          collectionProfile: candidate.collectionProfile,
          originalTaskStrategy: candidate.originalTaskStrategy,
          payload: candidate.payload,
        }),
        requiredFields: candidate.requiredFields,
        payload: candidate.payload,
      },
    });
  }

  return { reservations, hadCandidate: input.preselectedCandidates.length > 0 };
}

// ---------------------------------------------------------------------------
// 功能 2：releaseExpiredReservations
// ---------------------------------------------------------------------------

/**
 * 释放过期 reservation。
 *
 * 手册 5.4 原则 6：reserved 未开始超过 startBefore 自动释放回 queued。
 * 手册 4.16：reserved 过期释放回 queued → lane mailbox version +1。
 *
 * 同时按 startBefore 和 reservedUntil 释放，并清理 V1.1 reservation 字段。
 * reservationEpoch 不重置，保留增长语义；释放后 bump lane mailbox。
 */
export async function releaseExpiredReservations(input: {
  db?: QueueDbClient;
  now?: Date;
  /** 一次最多处理多少条；防止大表全表扫描卡死。V1.1 要求"异常分页修复"。 */
  batchSize?: number;
  /** V1.1（2026-06-29）：可选的 workspaceId 过滤，防止跨工作区泄露。 */
  workspaceId?: string;
} = {}): Promise<{ released: number }> {
  const db = input.db ?? prisma;
  const now = nowOr(input.now);
  const batchSize = input.batchSize ?? 200;

  // V1.1（2026-06-29）：支持 workspaceId 过滤。
  const workspaceFilter = input.workspaceId
    ? Prisma.sql`AND "workspaceId" = ${input.workspaceId}`
    : Prisma.empty;

  // 先找出过期的 reserved 记录（分页），用于后续 bump lane mailbox
  const expired = await db.$queryRaw<
    Array<{
      id: string;
      jobId: string;
      workspaceId: string | null;
      platform: string;
      lane: string;
    }>
  >(Prisma.sql`
    SELECT "id", "jobId", "workspaceId", "platform", "lane"
    FROM "ExecutionQueueEntry"
    WHERE "status" = 'reserved'::"ExecutionQueueEntryStatus"
      AND (
        ("startBefore" IS NOT NULL AND "startBefore" < ${now})
        OR ("startBefore" IS NULL AND "reservedUntil" IS NOT NULL AND "reservedUntil" < ${now})
      )
      ${workspaceFilter}
    ORDER BY "id" ASC
    LIMIT ${batchSize}
  `);

  if (expired.length === 0) return { released: 0 };

  // Queue 与 Job 的回收必须同一事务提交，避免出现 Queue 已可领而 Job 仍 reserved 的裂缝。
  const ids = expired.map((row) => row.id);
  const jobIds = expired.map((row) => row.jobId);
  const result = await withinQueueTransaction(db, async (tx) => {
    const queueUpdate = await tx.executionQueueEntry.updateMany({
      where: { id: { in: ids }, status: "reserved" },
      data: {
        status: "queued",
        reservedByStationId: null,
        reservedPlatformAccountId: null,
        reservedExecutionIdentityKey: null,
        reserveToken: null,
        reservedUntil: null,
        startBefore: null,
        lastErrorCode: "RESERVATION_EXPIRED",
        lastErrorScope: "infra",
        lastErrorMessage: "reservation expired before start",
      },
    });
    if (jobIds.length > 0) {
      await tx.executionJob.updateMany({
        where: { id: { in: jobIds }, status: "reserved" },
        data: { status: "queued", updatedAt: now },
      });
    }
    return queueUpdate;
  });

  // bump lane mailbox（手册 4.16）
  const laneKeys = new Map<string, { workspaceId: string; platform: string; lane: string }>();
  for (const row of expired) {
    const key = `${row.workspaceId ?? "__null__"}:${row.platform}:${row.lane}`;
    if (!laneKeys.has(key) && row.workspaceId) {
      laneKeys.set(key, { workspaceId: row.workspaceId, platform: row.platform, lane: row.lane });
    }
  }
  for (const meta of laneKeys.values()) {
    await markExecutionLaneMailboxChanged({
      db,
      workspaceId: meta.workspaceId,
      platform: meta.platform,
      lane: meta.lane,
      delta: 1,
    }).catch((error) => {
      console.warn("[swallowed]", "expired reservation lane mailbox update", error);
    });
  }

  // 这里没法直接知道原 stationId（已清空），所以走 workspace 级 mailbox bump。
  const workspaceIds = new Set<string>();
  for (const row of expired) {
    if (row.workspaceId) workspaceIds.add(row.workspaceId);
  }
  for (const workspaceId of workspaceIds) {
    await markExecutionStationMailboxesChanged({
      db,
      workspaceId,
      wakeReason: "reservation_expired",
    }).catch((error) => {
      console.warn("[swallowed]", "expired reservation station mailbox update", error);
    });
  }

  for (const row of expired) {
    await recordExecutionLogEvent({
      db,
      workspaceId: row.workspaceId,
      jobId: row.jobId,
      queueEntryId: row.id,
      platform: row.platform,
      lane: row.lane,
      eventType: "execution_queue.reservation_expired",
      stage: "reservation",
      status: "released",
      title: "工位未按时开始，任务已回到队列",
      message: "任务曾经派给工位，但插件没有在有效时间内开始执行，系统已把它放回等待队列。",
      reasonCode: "reservation_expired",
    });
  }

  return { released: result.count };
}

// ---------------------------------------------------------------------------
// 功能 3：releaseExpiredLeases
// ---------------------------------------------------------------------------

/**
 * 释放过期 lease。
 *
 * 手册不变量 6：旧 leaseEpoch / leaseToken 不能推进 ExecutionJob 状态。
 * 当 leaseExpiresAt 过期时：
 *   1. 把当前 TaskAttempt 标记为 stale_result（手册 4.6 TaskAttempt.result 新增 enum 值）
 *   2. 根据是否达到 attempt 上限决定：
 *      - 未达上限：QueueEntry.status 回 queued，可被重新 claim
 *      - 达上限：QueueEntry.status → failed
 *   3. bump lane mailbox（手册 4.16）
 *
 * V1.1（2026-06-29 事务修复）：
 *   - 每行操作包裹在 $transaction 中，防止 QueueEntry/Job/TaskAttempt 状态分裂。
 *   - UPDATE WHERE 重新验证 leaseExpiresAt < now，防止 TOCTOU 释放刚续的租约。
 *   - TaskAttempt 标记失败时记录日志，不再静默吞错。
 *
 * V1.1（2026-06-30 rt 僵尸根修）：
 *   - QueueEntry lease 过期后同步关闭 ExecutionTaskRuntime，避免 UI 继续读到 running。
 */
export async function releaseExpiredLeases(input: {
  db?: QueueDbClient;
  now?: Date;
  batchSize?: number;
  maxAttempts?: number;
  /** V1.1（2026-06-29）：可选的 workspaceId 过滤，防止跨工作区泄露。 */
  workspaceId?: string;
} = {}): Promise<{ released: number; failed: number }> {
  const db = input.db ?? prisma;
  const now = nowOr(input.now);
  const batchSize = input.batchSize ?? 200;
  const maxAttempts = input.maxAttempts ?? 3;

  // V1.1（2026-06-29）：支持 workspaceId 过滤。
  const workspaceFilter = input.workspaceId
    ? Prisma.sql`AND "workspaceId" = ${input.workspaceId}`
    : Prisma.empty;

  const expired = await db.$queryRaw<
    Array<{
      id: string;
      jobId: string;
      workspaceId: string | null;
      platform: string;
      lane: string;
      lastAttemptId: string | null;
      attemptCount: number;
    }>
  >(Prisma.sql`
    SELECT "id", "jobId", "workspaceId", "platform", "lane",
           "lastAttemptId", "attemptCount"
    FROM "ExecutionQueueEntry"
    WHERE "status" = 'in_progress'::"ExecutionQueueEntryStatus"
      AND "leaseExpiresAt" IS NOT NULL
      AND "leaseExpiresAt" < ${now}
      ${workspaceFilter}
    ORDER BY "leaseExpiresAt" ASC, "id" ASC
    LIMIT ${batchSize}
  `);

  if (expired.length === 0) return { released: 0, failed: 0 };

  let released = 0;
  let failed = 0;

  for (const row of expired) {
    const isFailure = row.attemptCount >= maxAttempts;

    // V1.1（2026-06-29）：每行包裹在独立事务中，防止 QueueEntry/Job/Attempt 状态分裂。
    // WHERE 中重新验证 leaseExpiresAt < now，防止 TOCTOU（插件恰好在 SELECT 后续了租约）。
    const committed = await db.$transaction(async (tx) => {
      const updated = await tx.executionQueueEntry.updateMany({
        where: {
          id: row.id,
          status: "in_progress",
          // TOCTOU 守卫：重新检查 leaseExpiresAt 仍 < now
          leaseExpiresAt: { lt: now },
        },
        data: isFailure
          ? {
              status: "failed",
              reservedByStationId: null,
              reservedPlatformAccountId: null,
              reservedExecutionIdentityKey: null,
              reserveToken: null,
              reservedUntil: null,
              startBefore: null,
              leaseToken: null,
              leaseExpiresAt: null,
              lastErrorCode: "LEASE_EXPIRED",
              lastErrorScope: "infra",
              lastErrorMessage: `lease expired after ${row.attemptCount} attempts`,
            }
          : {
              status: "queued",
              reservedByStationId: null,
              reservedPlatformAccountId: null,
              reservedExecutionIdentityKey: null,
              reserveToken: null,
              reservedUntil: null,
              startBefore: null,
              leaseToken: null,
              leaseExpiresAt: null,
              lastErrorCode: "LEASE_EXPIRED",
              lastErrorScope: "infra",
              lastErrorMessage: `lease expired at attempt ${row.attemptCount}, requeued`,
            },
      });

      if (updated.count === 0) return false; // 租约已被续，跳过

      // 同步 ExecutionJob
      if (row.jobId) {
        await tx.executionJob.updateMany({
          where: { id: row.jobId, status: "in_progress" },
          data: { status: isFailure ? "failed" : "queued", updatedAt: now },
        });

        // 同步 rt 影子表：QueueEntry 的过期 lease 已确认释放后，不能让 rt 继续显示 running。
        await tx.executionTaskRuntime.updateMany({
          where: {
            jobId: row.jobId,
            status: { in: ["dispatched", "running", "paused", "stopping"] },
          },
          data: {
            status: isFailure ? "failed" : "pending",
            activeExecutor: null,
            leaseToken: null,
            leaseExpiresAt: null,
            ...(isFailure
              ? {
                  completedAt: now,
                  errorMessage: `lease expired after ${row.attemptCount} attempts`,
                }
              : {}),
            updatedAt: now,
          },
        });
      }

      // 标记 TaskAttempt 为 stale_result
      if (row.lastAttemptId) {
        try {
          await tx.$executeRaw(Prisma.sql`
            UPDATE "TaskAttempt"
            SET "result" = 'stale_result'::"ExecutionTaskAttemptResult",
                "endedAt" = ${now}
            WHERE "id" = ${row.lastAttemptId}
              AND "result" IS NULL
          `);
        } catch (err) {
          // V1.1（2026-06-29）：记录日志，不再静默吞错。
          // enum 未扩展时 TaskAttempt UPDATE 会因 invalid enum value 失败，
          // 此时记录 warning 并继续；其他错误也记录但同样不阻塞主流程。
          console.warn(
            "[releaseExpiredLeases] failed to mark TaskAttempt stale_result:",
            err instanceof Error ? err.message : String(err),
            { lastAttemptId: row.lastAttemptId, jobId: row.jobId },
          );
        }
      }

      // bump lane mailbox
      if (row.workspaceId) {
        await markExecutionLaneMailboxChanged({
          db: tx,
          workspaceId: row.workspaceId,
          platform: row.platform,
          lane: row.lane,
          delta: isFailure ? 0 : 1,
        }).catch((error) => {
          console.warn("[swallowed]", "expired lease lane mailbox update", error);
        });
      }

      return true;
    }, {
      maxWait: 2_000,
      timeout: 8_000,
    });

    if (committed) {
      await recordExecutionLogEvent({
        db,
        workspaceId: row.workspaceId,
        jobId: row.jobId,
        queueEntryId: row.id,
        taskAttemptId: row.lastAttemptId,
        platform: row.platform,
        lane: row.lane,
        eventType: "execution_queue.lease_expired",
        stage: "lease",
        status: isFailure ? "failed" : "released",
        title: isFailure ? "执行租约过期，任务已失败" : "执行租约过期，任务已回到队列",
        message: isFailure
          ? "插件开始执行后没有按时交回有效进展，并且重试次数已用完。"
          : "插件开始执行后没有按时交回有效进展，系统已释放执行权并准备重试。",
        reasonCode: "LEASE_EXPIRED",
        details: {
          attemptCount: row.attemptCount,
          maxAttempts,
        },
      });
      if (isFailure) failed++; else released++;
    }
  }

  return { released, failed };
}

// ---------------------------------------------------------------------------
// 功能 3.5：reapZombieRuntimes（rt 僵尸回收，2026-06-30）
// ---------------------------------------------------------------------------

/**
 * 回收僵尸 ExecutionTaskRuntime。
 *
 * 背景（2026-06-30 根因）：历史“让任务结束”的部分路径
 *   只关闭 QueueEntry + Job，漏关 rt 影子表。于是 rt 永远停在 running 系，而
 *   /api/scheduler/snapshot 读 rt.status=running → UI 永久显示“执行中”。
 *   旧 reconcile 曾信任僵尸 rt，把 QueueEntry 反复拉回 in_progress 孤儿。
 *   当前 Queue 完整性修复已不再读取 Runtime，本函数只负责清掉展示残留。
 *
 * 本函数把 rt∈(dispatched/running/paused/stopping) 且 lease 过期/缺失、
 * 但对应 ExecutionJob 已进入终态/排队态的 rt，跟随 Job 权威表关闭：
 *   Job.failed     → rt.failed
 *   Job.succeeded  → rt.completed
 *   Job.cancelled  → rt.stopped
 *   Job.queued     → rt.pending（可被重新调度）
 *   Job.in_progress/created 等活跃态 → 不动（rt 可能合法运行中）
 *
 * TOCTOU 守卫：WHERE 要求 rt.leaseExpiresAt IS NULL OR < now，
 * 不会误杀刚续租的运行中任务（续租会把 leaseExpiresAt 推到未来）。
 *
 * 调度 tick 中仍先执行本函数，让旧 Runtime 展示字段尽快对齐 Job 权威状态。
 */
export async function reapZombieRuntimes(input: {
  db?: QueueDbClient;
  now?: Date;
} = {}): Promise<{ reaped: number }> {
  const db = input.db ?? prisma;
  const now = nowOr(input.now);

  const reapedRows = await db.$queryRaw<Array<{
    jobId: string;
    workspaceId: string | null;
    targetStatus: string;
  }>>(Prisma.sql`
    WITH zombie AS (
      SELECT
        rt."jobId" AS "jobId",
        CASE
          WHEN ct."status" = 'failed'::"ExecutionJobStatus" THEN 'failed'::"ExecutionTaskStatus"
          WHEN ct."status" = 'succeeded'::"ExecutionJobStatus" THEN 'completed'::"ExecutionTaskStatus"
          WHEN ct."status" = 'cancelled'::"ExecutionJobStatus" THEN 'stopped'::"ExecutionTaskStatus"
          WHEN ct."status" = 'queued'::"ExecutionJobStatus" THEN 'pending'::"ExecutionTaskStatus"
        END AS "targetStatus"
      FROM "ExecutionTaskRuntime" rt
      INNER JOIN "ExecutionJob" ct ON ct."id" = rt."jobId"
      WHERE rt."status"::text IN ('dispatched', 'running', 'paused', 'stopping')
        AND (rt."leaseExpiresAt" IS NULL OR rt."leaseExpiresAt" < ${now})
        AND ct."status" IN (
          'failed'::"ExecutionJobStatus",
          'succeeded'::"ExecutionJobStatus",
          'cancelled'::"ExecutionJobStatus",
          'queued'::"ExecutionJobStatus"
        )
    )
    UPDATE "ExecutionTaskRuntime" rt
    SET
      "status" = zombie."targetStatus",
      "activeExecutor" = NULL,
      "leaseToken" = NULL,
      "leaseExpiresAt" = NULL,
      "progress" = CASE
        WHEN zombie."targetStatus" = 'completed'::"ExecutionTaskStatus" THEN 100
        ELSE rt."progress"
      END,
      "completedAt" = CASE
        WHEN zombie."targetStatus" IN (
          'failed'::"ExecutionTaskStatus",
          'completed'::"ExecutionTaskStatus",
          'stopped'::"ExecutionTaskStatus"
        ) THEN ${now}
        ELSE rt."completedAt"
      END,
      "errorMessage" = CASE
        WHEN zombie."targetStatus" = 'failed'::"ExecutionTaskStatus"
          THEN COALESCE(rt."errorMessage", 'zombie runtime reaped: job terminal, lease expired')
        ELSE rt."errorMessage"
      END,
      "updatedAt" = ${now}
    FROM zombie
    WHERE rt."jobId" = zombie."jobId"
      -- TOCTOU 守卫：UPDATE 时重新确认仍是 running系 且 lease 仍过期/null
      AND rt."status"::text IN ('dispatched', 'running', 'paused', 'stopping')
      AND (rt."leaseExpiresAt" IS NULL OR rt."leaseExpiresAt" < ${now})
    RETURNING rt."jobId", rt."workspaceId", zombie."targetStatus"::text AS "targetStatus"
  `);

  if (!Array.isArray(reapedRows)) {
    return { reaped: Number(reapedRows) || 0 };
  }

  for (const row of reapedRows) {
    await recordExecutionLogEvent({
      db,
      workspaceId: row.workspaceId,
      jobId: row.jobId,
      eventType: "execution_runtime.zombie_reaped",
      stage: "runtime",
      status: row.targetStatus,
      title: "后台修正了卡住的运行状态",
      message: "任务租约已经过期或任务已结束，但运行账本仍显示执行中，系统已自动对齐状态。",
      reasonCode: "runtime_zombie_reaped",
      details: {
        targetStatus: row.targetStatus,
      },
    });
  }

  return { reaped: reapedRows.length };
}

// ---------------------------------------------------------------------------
// 功能 4：startJob（手册 5.5 start_job operation）
// ---------------------------------------------------------------------------

/**
 * reserved → in_progress。
 *
 * 服务端条件更新（手册 5.5）：
 *   - status = reserved
 *   - reserveToken 匹配
 *   - reservationEpoch 匹配
 *   - startBefore 未过期
 *   - stationId 匹配
 *   - account 仍健康（查 PlatformAccount）
 *
 * 成功后：
 *   - status → in_progress
 *   - leaseEpoch + 1
 *   - 生成 leaseToken / leaseExpiresAt
 *   - 创建 TaskAttempt.startedAt
 */
export async function startJob(input: StartJobInput & { db?: QueueRootClient }): Promise<StartJobResult> {
  const db = input.db ?? prisma;
  const now = nowOr(input.now);

  return db.$transaction(
    async (tx) => {
      const rows = await tx.$queryRaw<
        Array<{
          id: string;
          jobId: string;
          workspaceId: string | null;
          platform: string;
          lane: string;
          status: string;
          reserveToken: string | null;
          reservationEpoch: number;
          startBefore: Date | null;
          reservedByStationId: string | null;
          reservedPlatformAccountId: string | null;
          attemptCount: number;
          collectionProfile: string;
          executionPlanVersion: string | null;
        }>
      >(Prisma.sql`
        SELECT
          qe."id",
          qe."jobId",
          qe."workspaceId",
          qe."platform",
          qe."lane",
          qe."status"::text AS "status",
          qe."reserveToken",
          qe."reservationEpoch",
          qe."startBefore",
          qe."reservedByStationId",
          qe."reservedPlatformAccountId",
          qe."attemptCount",
          j."collectionProfile",
          j."executionPlanVersion"
        FROM "ExecutionQueueEntry" qe
        INNER JOIN "ExecutionJob" j ON j."id" = qe."jobId"
        WHERE qe."jobId" = ${input.jobId}
        FOR UPDATE
      `);

      const row = rows[0];
      if (!row) {
        return { status: "rejected", reason: "job_not_found" } satisfies StartJobResult;
      }
      if (row.status !== "reserved") {
        return { status: "rejected", reason: "status_not_reserved" } satisfies StartJobResult;
      }
      if (row.reserveToken !== input.reserveToken) {
        return { status: "rejected", reason: "reserve_token_mismatch" } satisfies StartJobResult;
      }
      if (row.reservationEpoch !== input.reservationEpoch) {
        return { status: "rejected", reason: "reservation_epoch_mismatch" } satisfies StartJobResult;
      }
      if (row.startBefore && row.startBefore.getTime() < now.getTime()) {
        return { status: "rejected", reason: "start_before_expired" } satisfies StartJobResult;
      }
      if (row.reservedByStationId !== input.stationId) {
        return { status: "rejected", reason: "station_mismatch" } satisfies StartJobResult;
      }
      if (!row.executionPlanVersion) {
        return { status: "rejected", reason: "execution_plan_version_missing" } satisfies StartJobResult;
      }

      // 校验账号仍健康
      if (row.reservedPlatformAccountId && row.workspaceId) {
        const accountRows = await tx.$queryRaw<
          Array<{
            healthStatus: string;
            purpose: string;
            cooldownUntil: Date | null;
            riskControlUntil: Date | null;
            dailyWindowDate: string | null;
            dailyTaskCount: number;
            dailyOpenedCount: number;
            dailySuccessCount: number;
            dailyFailedCount: number;
            rawProfile: string | null;
            lastCheckedAt: Date | null;
          }>
        >(Prisma.sql`
          SELECT "healthStatus", "purpose", "cooldownUntil", "riskControlUntil",
                 "dailyWindowDate", "dailyTaskCount", "dailyOpenedCount",
                 "dailySuccessCount", "dailyFailedCount", "rawProfile", "lastCheckedAt"
          FROM "PlatformAccount"
          WHERE "workspaceId" = ${row.workspaceId}
            AND "stationId" = ${row.reservedByStationId}
            AND "platformAccountId" = ${row.reservedPlatformAccountId}
            AND "platform" = ${row.platform}
          LIMIT 1
        `);
        const account = accountRows[0];
        if (
          !account ||
          !evaluatePlatformAccountClaimEligibility(
            { ...account, platformAccountId: row.reservedPlatformAccountId },
            now,
          ).eligible
        ) {
          return { status: "rejected", reason: "account_unhealthy" } satisfies StartJobResult;
        }
      }

      // 生成 lease 三件套
      const ttl = ttlForProfile(row.collectionProfile);
      const leaseToken = randomUUID();
      const nextLeaseEpoch = row.reservationEpoch + 1; // leaseEpoch 接续 reservationEpoch 递增（手册 4.5 同源语义）
      const leaseExpiresAt = new Date(now.getTime() + ttl.leaseTtlMs);

      // QueueEntry: reserved → in_progress
      await tx.$executeRaw(Prisma.sql`
        UPDATE "ExecutionQueueEntry"
        SET
          "status" = 'in_progress'::"ExecutionQueueEntryStatus",
          "leaseToken" = ${leaseToken},
          "leaseEpoch" = ${nextLeaseEpoch},
          "leaseExpiresAt" = ${leaseExpiresAt},
          "updatedAt" = ${now}
        WHERE "id" = ${row.id}
          AND "status" = 'reserved'::"ExecutionQueueEntryStatus"
      `);

      // ExecutionJob: reserved → in_progress
      await tx.executionJob.updateMany({
        where: { id: input.jobId, status: "reserved" },
        data: { status: "in_progress", updatedAt: now },
      });

      // 创建 TaskAttempt，关联当前 ExecutionJob。
      const attempt = await tx.taskAttempt.create({
        data: {
          jobId: input.jobId,
          stationId: input.stationId,
          platform: row.platform,
          source: row.lane,
          attemptNumber: row.attemptCount + 1,
          leaseToken,
          leaseEpoch: nextLeaseEpoch,
        },
      });

      // 更新 QueueEntry.lastAttemptId
      await tx.executionQueueEntry.update({
        where: { id: row.id },
        data: { lastAttemptId: attempt.id, attemptCount: { increment: 1 } },
      });

      await tx.executionTaskRuntime.upsert({
        where: { jobId: input.jobId },
        create: {
          jobId: input.jobId,
          workspaceId: row.workspaceId,
          status: "running",
          progress: 0,
          activeExecutor: input.stationId,
          assignedStationId: input.stationId,
          leaseToken,
          leaseEpoch: nextLeaseEpoch,
          leaseExpiresAt,
          currentAttemptId: attempt.id,
          dispatchedAt: now,
          startedAt: now,
        },
        update: {
          status: "running",
          activeExecutor: input.stationId,
          assignedStationId: input.stationId,
          leaseToken,
          leaseEpoch: nextLeaseEpoch,
          leaseExpiresAt,
          currentAttemptId: attempt.id,
          startedAt: now,
          updatedAt: now,
        },
      });

      return {
        status: "started",
        attemptId: attempt.id,
        captureId: attempt.captureId,
        executionPlanVersion: row.executionPlanVersion,
        leaseToken,
        leaseEpoch: nextLeaseEpoch,
        leaseExpiresAt,
      } satisfies StartJobResult;
    },
    {
      isolationLevel: Prisma.TransactionIsolationLevel.ReadCommitted,
      timeout: 5_000,
    }
  );
}

// ---------------------------------------------------------------------------
// 功能 5：progressLease（手册 5.5 progress_update operation 的续租部分）
// ---------------------------------------------------------------------------

/**
 * 续租 + 更新进度。
 *
 * 校验（手册不变量 6）：
 *   - leaseToken 匹配
 *   - leaseEpoch 匹配（旧 epoch 不能续）
 *   - status = in_progress
 *
 * 成功后：
 *   - leaseExpiresAt 续到 now + leaseTtl
 *   - leaseEpoch 保持不变（只有 start_job 时 +1；progress 不递增）
 *
 * 进度数据（progress / stage）由 Agent E 写入 TaskStatusProjection，不在本服务处理。
 */
export async function progressLease(
  input: ProgressLeaseInput & { db?: QueueRootClient }
): Promise<ProgressLeaseResult> {
  const db = input.db ?? prisma;
  const now = nowOr(input.now);
  // 报告 §9.1：采集阶段进度封顶 95。100% 是"服务端已确认终态包入库"的语义，
  // 只能在终态（completed）同步时由服务端置位，插件经 progress_update 自报
  // 的进度到不了 100（防止"进度 100% 但服务端没收到结果"的误导展示）。
  const progress = typeof input.progress === "number" && input.progress >= 0
    ? Math.min(Math.floor(input.progress), 95)
    : null;

  // V1.1（2026-06-29）：FOR UPDATE 必须在显式事务中才能持有行锁。
  // 手册不变量 6：旧 leaseEpoch/leaseToken 不能推进状态。
  return db.$transaction(async (tx) => {
    const rows = await tx.$queryRaw<
      Array<{
        id: string;
        jobId: string;
        collectionProfile: string;
        status: string;
        leaseToken: string | null;
        leaseEpoch: number;
      }>
    >(Prisma.sql`
      SELECT
        qe."id",
        qe."jobId",
        j."collectionProfile",
        qe."status"::text AS "status",
        qe."leaseToken",
        qe."leaseEpoch"
      FROM "ExecutionQueueEntry" qe
      INNER JOIN "ExecutionJob" j ON j."id" = qe."jobId"
      WHERE qe."jobId" = ${input.jobId}
      FOR UPDATE
    `);

    const row = rows[0];
    if (!row) {
      return { status: "rejected", reason: "job_not_found" } satisfies ProgressLeaseResult;
    }
    if (row.status !== "in_progress") {
      return { status: "rejected", reason: "status_not_in_progress" } satisfies ProgressLeaseResult;
    }
    if (row.leaseToken !== input.leaseToken) {
      return { status: "rejected", reason: "lease_token_mismatch" } satisfies ProgressLeaseResult;
    }
    if (row.leaseEpoch !== input.leaseEpoch) {
      return { status: "rejected", reason: "lease_epoch_mismatch" } satisfies ProgressLeaseResult;
    }

    const ttl = ttlForProfile(row.collectionProfile);
    const newLeaseExpiresAt = new Date(now.getTime() + ttl.leaseTtlMs);

    await tx.$executeRaw(Prisma.sql`
      UPDATE "ExecutionQueueEntry"
      SET
        "leaseExpiresAt" = ${newLeaseExpiresAt},
        "updatedAt" = ${now}
      WHERE "id" = ${row.id}
        AND "status" = 'in_progress'::"ExecutionQueueEntryStatus"
        AND "leaseToken" = ${input.leaseToken}
        AND "leaseEpoch" = ${input.leaseEpoch}
    `);

    // V1.1（2026-06-29）：进度持久化。
    if (progress !== null) {
      await tx.$executeRaw(Prisma.sql`
        UPDATE "ExecutionTaskRuntime"
        SET "progress" = ${progress},
            "updatedAt" = ${now}
        WHERE "jobId" = ${input.jobId}
      `);
    }

    return {
      status: "renewed",
      leaseExpiresAt: newLeaseExpiresAt,
      leaseEpoch: row.leaseEpoch,
    } satisfies ProgressLeaseResult;
  }, {
    maxWait: 3_000,
    timeout: 10_000,
  });
}

// ---------------------------------------------------------------------------
// 工具：检测 unique violation（手册 1.4 V1.1 修正点 2 的 partial unique index 兜底）
// ---------------------------------------------------------------------------

function isUniqueViolation(error: unknown): boolean {
  if (!error || typeof error !== "object") return false;
  // Prisma 把 Postgres unique_violation (23505) 包成 P2002
  const code = (error as { code?: string }).code;
  if (code === "P2002") return true;
  // 直接是 Postgres error
  const pgCode = (error as { pgCode?: string; constraint?: string }).pgCode;
  if (pgCode === "23505") return true;
  const message = (error as { message?: string }).message ?? "";
  if (/unique constraint|duplicate key/i.test(message)) return true;
  return false;
}
