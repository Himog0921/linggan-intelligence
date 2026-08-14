import type { Prisma } from "@/lib/prisma-client";
import { runExecutionStationMaintenance } from "@/lib/services/execution-station-service";
import {
  repairExecutionQueueIntegrity,
} from "@/lib/services/execution-queue-service";
import {
  releaseExpiredLeases,
  releaseExpiredReservations,
  reapZombieRuntimes,
} from "@/lib/services/execution-queue-claim-service-v11";
import { runDueMonitorScheduler } from "@/lib/services/monitor-scheduler-service";
import { logExecutionTaskSchedulerTick } from "@/lib/services/execution-task-observability-service";
import { detectWritebackStalls } from "@/lib/services/execution-writeback-stall-alert-service";
import { refreshExecutionPlannerSnapshotsForActiveWorkspaces } from "@/lib/services/execution-planner-service";
import { expireStaleMonitorOpportunities } from "@/lib/services/monitor-opportunity-expiry-service";
import { runMonitorEvaluationAdjustments } from "@/lib/services/monitor-evaluation-runner-service";
import { runEliminationAdvisorMaintenance } from "@/lib/services/elimination-advisor-service";
import { processMonitorSurgeTrackingMaintenance } from "@/lib/services/monitor-surge-tracking-service";
import { createTopicMarketComparisonRunner } from "@/lib/services/topic-market-comparison-agent-runner";
import { runTopicMarketComparisonMaintenance } from "@/lib/services/topic-market-comparison-service";
import { processPendingComments } from "@/lib/services/comment-processor-service";
import { processOutboxBatch } from "@/lib/services/outbox-worker-service";
import { expireOverdueTaskDemands } from "@/lib/services/task-demand-service";
import { runRetentionSweep } from "@/lib/services/data-retention-service";
import { runTopicPerformanceRollupDailyMaintenance } from "@/lib/services/topic-performance-rollup-service";
import { runDemandRadarCommentSupplyMaintenance } from "@/lib/services/comment-supply-service";
import { runDemandCardMergeDailyMaintenance } from "@/lib/services/demand-card-merge-service";
import { runDataLifecycleMaintenance } from "@/lib/services/data-lifecycle-maintenance-service";
import { runTaxonomyGovernanceWeekly } from "@/lib/services/taxonomy-governance-service";
import {
  dispatchApproachingCheckpoints,
  type DispatchApproachingCheckpointsResult,
} from "@/lib/services/monitor-checkpoint-service";
import { prisma } from "@/lib/db";
import { recordExecutionLogEvent } from "@/lib/services/execution-log-service";
import { isMediaLibraryHardCutoverMaintenanceActive } from "@/lib/media-library-maintenance";
import { ensureDefaultWorkspace } from "@/lib/workspace";
import { persistPlatformCircuitBreakerStatesIfChanged } from "@/lib/services/platform-circuit-breaker-service";
import { runAuthorArchiveProjectionSweeperMaintenance } from "@/lib/services/author-archive-projection-sweeper-service";
import { reconcileStuckDiscoveryJobs } from "@/lib/services/author-deep-archive-service";
import { runV2DurableWorkerTick } from "@/lib/evidence/worker/v2-durable-worker-runtime";

const HEARTBEAT_INTERVAL_MS = 5 * 60_000;
const PRODUCTION_DISPATCH_INTERVAL_MS = 3 * 60_000;
const PRODUCTION_MAINTENANCE_INTERVAL_MS = 15 * 60_000;
// 开发环境由 dispatch 心跳按比例触发 maintenance；Mac mini 正式环境另有独立维护兜底。
const MAINTENANCE_TICK_RATIO = 3;
const MONITOR_SCHEDULER_ADVISORY_LOCK_KEY = 742_199_002_001;
const MONITOR_SCHEDULER_MAINTENANCE_ADVISORY_LOCK_KEY = 742_199_002_002;
const V2_HARD_CUT_SCHEDULER_ADVISORY_LOCK_KEY = 742_199_002_003;
const COMMENT_PROMOTION_BATCH_SIZE = 100;
// Outbox worker 本地常驻触发：手册 sec 5.9 描述了完整 worker 领取语义，
// 但本地/非 Vercel 部署没有外部 cron 入口。dispatch tick 内直接调用 processOutboxBatch，
// 让既有 OutboxEvent 能被处理。
const OUTBOX_BATCH_SIZE = 50;
// Checkpoint → Demand 调度：每个 workspace 每次 tick 扫描的 checkpoint 上限。
// 默认 200 与 findCheckpointsApproachingDue 一致；本地多 workspace 时足够覆盖。
const CHECKPOINT_DISPATCH_LIMIT_PER_WORKSPACE = 200;

type HeartbeatState = {
  intervalId: ReturnType<typeof setInterval> | null;
  maintenanceIntervalId: ReturnType<typeof setInterval> | null;
  dispatchInFlight: boolean;
  maintenanceInFlight: boolean;
  dispatchTickCount: number;
};

type MonitorSchedulerTickMode = "dispatch" | "maintenance";

type MonitorSchedulerTickStep =
  | "scheduler"
  | "queueReservationRecovery"
  | "queueLeaseRecovery"
  | "runtimeZombieReap"
  | "queueIntegrityRepair"
  | "archiveDiscoveryReconciliation"
  | "maintenance"
  | "commentPromotion"
  | "archiveProjectionSweep"
  | "planner"
  | "opportunityExpiry"
  | "evaluationAdjustments"
  | "eliminationAdvisor"
  | "surgeTracking"
  | "demandExpiry"
  | "retention"
  | "topicPerformanceRollup"
  | "commentSupply"
  | "demandCardMerge"
  | "marketComparison"
  | "dataLifecycleDryRun"
  | "taxonomyGovernance"
  | "circuitBreaker"
  | "outbox"
  | "v2DurableWorker"
  | "checkpointDispatch"
  | "writebackStallAlert";

type MonitorSchedulerStepDuration = {
  step: MonitorSchedulerTickStep;
  durationMs: number;
};

type MonitorSchedulerStepResult<T extends (...args: never[]) => unknown> =
  Awaited<ReturnType<T>> | null;

export type MonitorSchedulerTickResult = {
  mode: MonitorSchedulerTickMode;
  skipped: boolean;
  startedAt: string;
  finishedAt: string | null;
  maintenance: MonitorSchedulerStepResult<typeof runExecutionStationMaintenance>;
  queueReservationRecovery: MonitorSchedulerStepResult<
    typeof releaseExpiredReservations
  >;
  queueLeaseRecovery: MonitorSchedulerStepResult<
    typeof releaseExpiredLeases
  >;
  runtimeZombieReap: MonitorSchedulerStepResult<typeof reapZombieRuntimes>;
  queueIntegrityRepair: MonitorSchedulerStepResult<
    typeof repairExecutionQueueIntegrity
  >;
  archiveDiscoveryReconciliation: MonitorSchedulerStepResult<
    typeof reconcileStuckDiscoveryJobs
  >;
  commentPromotion: MonitorSchedulerStepResult<typeof processPendingComments>;
  archiveProjectionSweep: MonitorSchedulerStepResult<
    typeof runAuthorArchiveProjectionSweeperMaintenance
  >;
  planner: MonitorSchedulerStepResult<typeof refreshExecutionPlannerSnapshotsForActiveWorkspaces>;
  opportunityExpiry: MonitorSchedulerStepResult<typeof expireStaleMonitorOpportunities>;
  evaluationAdjustments: MonitorSchedulerStepResult<typeof runMonitorEvaluationAdjustments>;
  eliminationAdvisor: MonitorSchedulerStepResult<typeof runEliminationAdvisorMaintenance>;
  surgeTracking: MonitorSchedulerStepResult<typeof processMonitorSurgeTrackingMaintenance>;
  demandExpiry: MonitorSchedulerStepResult<typeof expireOverdueTaskDemands>;
  retention: MonitorSchedulerStepResult<typeof runRetentionSweep>;
  topicPerformanceRollup: MonitorSchedulerStepResult<
    typeof runTopicPerformanceRollupDailyMaintenance
  >;
  commentSupply: MonitorSchedulerStepResult<typeof runDemandRadarCommentSupplyMaintenance>;
  demandCardMerge: MonitorSchedulerStepResult<typeof runDemandCardMergeDailyMaintenance>;
  marketComparison: MonitorSchedulerStepResult<typeof runTopicMarketComparisonMaintenance>;
  dataLifecycleDryRun: MonitorSchedulerStepResult<typeof runDataLifecycleMaintenance>;
  taxonomyGovernance: MonitorSchedulerStepResult<typeof runTaxonomyGovernanceWeekly>;
  scheduler: MonitorSchedulerStepResult<typeof runDueMonitorScheduler>;
  circuitBreaker: MonitorSchedulerStepResult<typeof persistPlatformCircuitBreakerStatesIfChanged>;
  outbox: MonitorSchedulerStepResult<typeof processOutboxBatch>;
  v2DurableWorker: MonitorSchedulerStepResult<typeof runV2DurableWorkerTick>;
  checkpointDispatch: WorkspaceCheckpointDispatchSummary[] | null;
  writebackStallAlert: MonitorSchedulerStepResult<typeof detectWritebackStalls>;
  stepDurations: MonitorSchedulerStepDuration[];
  errors: Array<{ step: string; message: string }>;
};

/**
 * 单 workspace 的 checkpoint → demand 调度汇总（断层 #6）。
 * 每个 workspace 一条，便于在 UI / 日志里按 workspace 查看调度效果。
 */
export type WorkspaceCheckpointDispatchSummary = {
  workspaceId: string;
  result: DispatchApproachingCheckpointsResult;
};

export type V2HardCutSchedulerReceipt = {
  schemaVersion: "v2-hard-cut-scheduler-receipt/v1";
  observedAt: string;
  mode: "v2_hard_cut";
  status: "ready" | "busy" | "failed";
  lockAcquired: boolean;
  consumers: {
    legacyOutbox: { scheduled: boolean; invocations: number };
    v2DurableWorker: { scheduled: boolean; invocations: number };
  };
  worker: Awaited<ReturnType<typeof runV2DurableWorkerTick>> | null;
  errors: Array<{ step: "v2DurableWorker"; message: string }>;
};

declare global {
  // Persist the singleton across hot reloads and repeated instrumentation calls.
  var __topicDashboardMonitorSchedulerHeartbeat__: HeartbeatState | undefined;
}

function getHeartbeatState() {
  globalThis.__topicDashboardMonitorSchedulerHeartbeat__ ??= {
    intervalId: null,
    maintenanceIntervalId: null,
    dispatchInFlight: false,
    maintenanceInFlight: false,
    dispatchTickCount: 0,
  };
  return globalThis.__topicDashboardMonitorSchedulerHeartbeat__;
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

async function runTickStep<T>(
  result: MonitorSchedulerTickResult,
  step: MonitorSchedulerTickStep,
  run: () => Promise<T>
): Promise<T | null> {
  const startedAt = Date.now();
  try {
    return await run();
  } catch (error) {
    result.errors.push({ step, message: errorMessage(error) });
    await recordExecutionLogEvent({
      eventType: "scheduler.step_failed",
      stage: "scheduler",
      status: "failed",
      title: `调度步骤失败：${step}`,
      message: errorMessage(error),
      source: "scheduler_tick",
    }).catch(() => undefined);
    return null;
  } finally {
    result.stepDurations.push({
      step,
      durationMs: Date.now() - startedAt,
    });
  }
}

async function runWithSchedulerAdvisoryLock(
  lockKey: number,
  run: () => Promise<void>
): Promise<boolean> {
  return prisma.$transaction(async (tx: Prisma.TransactionClient) => {
    const rows = await tx.$queryRaw<Array<{ locked: boolean }>>`
    SELECT pg_try_advisory_xact_lock(${lockKey}) AS locked
  `;
    if (!rows[0]?.locked) return false;

    await run();
    return true;
  }, {
    maxWait: 5_000,
    timeout: 120_000,
  });
}

function createSchedulerTickResult(input: {
  mode: MonitorSchedulerTickMode;
  now: Date;
  skipped?: boolean;
  errors?: Array<{ step: string; message: string }>;
}): MonitorSchedulerTickResult {
  return {
    mode: input.mode,
    skipped: input.skipped ?? false,
    startedAt: input.now.toISOString(),
    finishedAt: null,
    maintenance: null,
    queueReservationRecovery: null,
    queueLeaseRecovery: null,
    runtimeZombieReap: null,
    queueIntegrityRepair: null,
    archiveDiscoveryReconciliation: null,
    commentPromotion: null,
    archiveProjectionSweep: null,
    planner: null,
    opportunityExpiry: null,
    evaluationAdjustments: null,
    eliminationAdvisor: null,
    surgeTracking: null,
    demandExpiry: null,
    retention: null,
    topicPerformanceRollup: null,
    commentSupply: null,
    demandCardMerge: null,
    marketComparison: null,
    dataLifecycleDryRun: null,
    taxonomyGovernance: null,
    scheduler: null,
    circuitBreaker: null,
    outbox: null,
    v2DurableWorker: null,
    checkpointDispatch: null,
    writebackStallAlert: null,
    stepDurations: [],
    errors: input.errors ?? [],
  };
}

async function runSchedulerTickCore(input: {
  mode: MonitorSchedulerTickMode;
  now: Date;
  lockKey: number;
  inFlightKey: "dispatchInFlight" | "maintenanceInFlight";
  alreadyRunningMessage: string;
  lockSkippedMessage: string;
  /** 调用来源：in-process-heartbeat / cloudflare-worker-cron / http-unknown。 */
  source?: string;
  runSteps: (result: MonitorSchedulerTickResult) => Promise<void>;
}): Promise<MonitorSchedulerTickResult> {
  const tickStartedAt = Date.now();
  const state = getHeartbeatState();
  if (state[input.inFlightKey]) {
    const skippedResult = createSchedulerTickResult({
      mode: input.mode,
      now: input.now,
      skipped: true,
      errors: [{ step: "tick", message: input.alreadyRunningMessage }],
    });
    logExecutionTaskSchedulerTick({
      ...skippedResult,
      source: input.source,
      durationMs: Date.now() - tickStartedAt,
    });
    return skippedResult;
  }

  state[input.inFlightKey] = true;
  const result = createSchedulerTickResult({
    mode: input.mode,
    now: input.now,
  });
  try {
    const lockAcquired = await runWithSchedulerAdvisoryLock(input.lockKey, async () => {
      await input.runSteps(result);
    });

    if (!lockAcquired) {
      result.skipped = true;
      result.errors.push({ step: "tick", message: input.lockSkippedMessage });
    }
  } finally {
    result.finishedAt = new Date().toISOString();
    state[input.inFlightKey] = false;
    logExecutionTaskSchedulerTick({
      ...result,
      source: input.source,
      durationMs: Date.now() - tickStartedAt,
    });
  }
  return result;
}

/**
 * 断层 #6 修复：遍历所有 workspace（Organization），为每个 workspace 调一次
 * dispatchApproachingCheckpoints，把到期的 MonitorCheckpoint 转成 TaskDemand。
 *
 * 单 workspace 失败不阻塞其他 workspace（catch 后记空结果，继续下一个）。
 * 返回值每个 workspace 一条汇总，便于在调度日志里按 workspace 归因。
 *
 * 设计取舍：
 *   - 不并发：多 workspace 串行，避免一次 tick 内同时打多个 workspace 的 DB。
 *   - 不限速：dispatchApproachingCheckpoints 内部对单 workspace 已 limit=200，
 *     且 createOrReuseTaskDemand 是幂等 upsert，重复跑也安全。
 *   - Organization 作为 workspace 代理：本系统 workspace = Organization
 *     （手册 sec 2.1，prisma.organization 即 workspace 表）。
 */
async function runCheckpointDispatchForAllWorkspaces(input: {
  now: Date;
}): Promise<WorkspaceCheckpointDispatchSummary[]> {
  const organizations = await prisma.organization.findMany({
    select: { id: true },
    orderBy: [{ createdAt: "asc" }],
  });

  const summaries: WorkspaceCheckpointDispatchSummary[] = [];
  for (const org of organizations) {
    try {
      const result = await dispatchApproachingCheckpoints({
        workspaceId: org.id,
        now: input.now,
        limit: CHECKPOINT_DISPATCH_LIMIT_PER_WORKSPACE,
      });
      summaries.push({ workspaceId: org.id, result });
    } catch {
      // 单 workspace 失败不阻塞其他 workspace；返回空结果以便汇总字段对齐。
      summaries.push({
        workspaceId: org.id,
        result: {
          scanned: 0,
          dispatched: 0,
          reused: 0,
          failed: 0,
          skipped: 0,
          details: [],
        },
      });
    }
  }
  return summaries;
}

export async function runMonitorSchedulerTick(input: {
  now?: Date;
  recoveryLimit?: number;
  source?: string;
} = {}): Promise<MonitorSchedulerTickResult> {
  const now = input.now ?? new Date();
  if (isMediaLibraryHardCutoverMaintenanceActive()) {
    return createSchedulerTickResult({ mode: "dispatch", now, skipped: true });
  }
  return runSchedulerTickCore({
    mode: "dispatch",
    now,
    source: input.source ?? "in-process-heartbeat",
    lockKey: MONITOR_SCHEDULER_ADVISORY_LOCK_KEY,
    inFlightKey: "dispatchInFlight",
    alreadyRunningMessage: "上一轮高频调度还在执行，本轮已跳过",
    lockSkippedMessage: "另一台服务正在执行高频调度，本轮已跳过",
    async runSteps(result) {
      result.queueReservationRecovery = await runTickStep(
        result,
        "queueReservationRecovery",
        () => releaseExpiredReservations({ now })
      );
      // Runtime 仅做兼容清扫；Queue 完整性修复不再读取 Runtime。
      result.runtimeZombieReap = await runTickStep(
        result,
        "runtimeZombieReap",
        () => reapZombieRuntimes({ now })
      );
      result.queueIntegrityRepair = await runTickStep(
        result,
        "queueIntegrityRepair",
        () => repairExecutionQueueIntegrity({ now })
      );
      result.archiveDiscoveryReconciliation = await runTickStep(
        result,
        "archiveDiscoveryReconciliation",
        () => reconcileStuckDiscoveryJobs({ now })
      );
      result.scheduler = await runTickStep(result, "scheduler", () =>
        runDueMonitorScheduler({ dryRun: false, now })
      );
      // 断层 #6 修复：扫到期 MonitorCheckpoint → createCheckpointDemand。
      // 必须放在 scheduler 之后（先让 monitor 调度器生成 checkpoint，再为到期 checkpoint
      // 创建 TaskDemand，衔接 monitor → demand → execution 端到端链路）。
      result.checkpointDispatch = await runTickStep(
        result,
        "checkpointDispatch",
        () => runCheckpointDispatchForAllWorkspaces({ now })
      );
      // 报告 §9.2：进度>=95 且 60 秒无 receipt 的写回停滞早期告警（3 分钟节拍检查）。
      result.writebackStallAlert = await runTickStep(
        result,
        "writebackStallAlert",
        () => detectWritebackStalls({ now })
      );
      result.commentPromotion = await runTickStep(result, "commentPromotion", () =>
        processPendingComments(COMMENT_PROMOTION_BATCH_SIZE, undefined, {
          analyze: true,
        })
      );
      result.queueLeaseRecovery = await runTickStep(result, "queueLeaseRecovery", () =>
        releaseExpiredLeases({
          now,
          batchSize: input.recoveryLimit ?? 50,
        })
      );
      // Outbox worker 本地常驻：drain 既有 OutboxEvent（
      // writeback_delivery.created / writeback_delivery.applied 等）。
      // 放在 dispatch tick 末尾，使 V1.1 lease 释放先跑完后再处理新事件。
      result.outbox = await runTickStep(result, "outbox", () =>
        processOutboxBatch({ batchSize: OUTBOX_BATCH_SIZE })
      );
      result.v2DurableWorker = await runTickStep(result, "v2DurableWorker", () =>
        runV2DurableWorkerTick(10)
      );
    },
  });
}

/**
 * Maintenance-freeze scheduler lane. Its execution plan contains no V1
 * Outbox consumer; the receipt reports the configured plan and actual
 * invocation counts, while readiness is derived from the real worker result.
 */
export async function runV2HardCutSchedulerTick(input: {
  now?: Date;
  batchSize?: number;
} = {}): Promise<V2HardCutSchedulerReceipt> {
  if (!isMediaLibraryHardCutoverMaintenanceActive()) {
    throw new Error("V2 hard-cut scheduler requires active hard-cutover maintenance.");
  }
  const now = input.now ?? new Date();
  const batchSize = input.batchSize ?? 10;
  if (!Number.isSafeInteger(batchSize) || batchSize < 1 || batchSize > 100) {
    throw new TypeError("V2 hard-cut worker batchSize must be an integer from 1 to 100.");
  }
  const plan = Object.freeze({ legacyOutbox: false, v2DurableWorker: true });
  let worker: Awaited<ReturnType<typeof runV2DurableWorkerTick>> | null = null;
  let v2Invocations = 0;
  const errors: V2HardCutSchedulerReceipt["errors"] = [];
  const lockAcquired = await runWithSchedulerAdvisoryLock(
    V2_HARD_CUT_SCHEDULER_ADVISORY_LOCK_KEY,
    async () => {
      if (!plan.v2DurableWorker) return;
      v2Invocations += 1;
      try {
        worker = await runV2DurableWorkerTick(batchSize);
      } catch (error) {
        errors.push({ step: "v2DurableWorker", message: errorMessage(error) });
      }
    },
  );
  const workerFailed = hasV2HardCutWorkerErrors(worker);
  const status: V2HardCutSchedulerReceipt["status"] = !lockAcquired
    ? "busy"
    : errors.length > 0 || worker === null || workerFailed
      ? "failed"
      : "ready";
  return {
    schemaVersion: "v2-hard-cut-scheduler-receipt/v1",
    observedAt: now.toISOString(),
    mode: "v2_hard_cut",
    status,
    lockAcquired,
    consumers: {
      legacyOutbox: { scheduled: plan.legacyOutbox, invocations: 0 },
      v2DurableWorker: { scheduled: plan.v2DurableWorker, invocations: v2Invocations },
    },
    worker,
    errors,
  };
}

function hasV2HardCutWorkerErrors(
  worker: Awaited<ReturnType<typeof runV2DurableWorkerTick>> | null,
): boolean {
  return worker !== null && worker.errors > 0;
}

export async function runMonitorSchedulerMaintenanceTick(input: {
  now?: Date;
} = {}): Promise<MonitorSchedulerTickResult> {
  const now = input.now ?? new Date();
  if (isMediaLibraryHardCutoverMaintenanceActive()) {
    return createSchedulerTickResult({ mode: "maintenance", now, skipped: true });
  }
  return runSchedulerTickCore({
    mode: "maintenance",
    now,
    lockKey: MONITOR_SCHEDULER_MAINTENANCE_ADVISORY_LOCK_KEY,
    inFlightKey: "maintenanceInFlight",
    alreadyRunningMessage: "上一轮低频维护还在执行，本轮已跳过",
    lockSkippedMessage: "另一台服务正在执行低频维护，本轮已跳过",
    async runSteps(result) {
      result.maintenance = await runTickStep(result, "maintenance", () =>
        runExecutionStationMaintenance({ now })
      );
      result.archiveProjectionSweep = await runTickStep(
        result,
        "archiveProjectionSweep",
        () => runAuthorArchiveProjectionSweeperMaintenance({ now }),
      );
      result.planner = await runTickStep(result, "planner", () =>
        refreshExecutionPlannerSnapshotsForActiveWorkspaces({ now })
      );
      result.opportunityExpiry = await runTickStep(result, "opportunityExpiry", () =>
        expireStaleMonitorOpportunities({ now })
      );
      result.evaluationAdjustments = await runTickStep(result, "evaluationAdjustments", () =>
        runMonitorEvaluationAdjustments({ now })
      );
      result.eliminationAdvisor = await runTickStep(result, "eliminationAdvisor", () =>
        runEliminationAdvisorMaintenance({ now })
      );
      result.surgeTracking = await runTickStep(result, "surgeTracking", () =>
        processMonitorSurgeTrackingMaintenance({ now })
      );
      result.demandExpiry = await runTickStep(result, "demandExpiry", () =>
        expireOverdueTaskDemands({ now })
      );
      result.retention = await runTickStep(result, "retention", () =>
        runRetentionSweep({ now })
      );
      result.topicPerformanceRollup = await runTickStep(
        result,
        "topicPerformanceRollup",
        async () => {
          const workspace = await ensureDefaultWorkspace();
          return runTopicPerformanceRollupDailyMaintenance({
            workspaceId: workspace.id,
            now,
          });
        }
      );
      result.commentSupply = await runTickStep(result, "commentSupply", async () => {
        const workspace = await ensureDefaultWorkspace();
        return runDemandRadarCommentSupplyMaintenance({
          workspaceId: workspace.id,
          now,
        });
      });
      result.demandCardMerge = await runTickStep(result, "demandCardMerge", async () => {
        const workspace = await ensureDefaultWorkspace();
        return runDemandCardMergeDailyMaintenance({
          workspaceId: workspace.id,
          now,
        });
      });
      result.marketComparison = await runTickStep(result, "marketComparison", async () => {
        const workspace = await ensureDefaultWorkspace();
        return runTopicMarketComparisonMaintenance({
          workspaceId: workspace.id,
          now,
          signalCardLimit: 4,
          runnerFactory: () =>
            createTopicMarketComparisonRunner({
              workspaceId: workspace.id,
              userId: null,
            }),
        });
      });
      result.dataLifecycleDryRun = await runTickStep(result, "dataLifecycleDryRun", async () => {
        const workspace = await ensureDefaultWorkspace();
        return runDataLifecycleMaintenance({
          workspaceId: workspace.id,
          now,
          dryRun: true,
        });
      });
      result.taxonomyGovernance = await runTickStep(result, "taxonomyGovernance", async () => {
        const workspace = await ensureDefaultWorkspace();
        return runTaxonomyGovernanceWeekly({
          workspaceId: workspace.id,
          now,
        });
      });
      result.circuitBreaker = await runTickStep(result, "circuitBreaker", () =>
        persistPlatformCircuitBreakerStatesIfChanged({ now })
      );
    },
  });
}

function runBackgroundSchedulerTick(mode: MonitorSchedulerTickMode) {
  const tick = mode === "dispatch"
    ? runMonitorSchedulerTick
    : runMonitorSchedulerMaintenanceTick;

  void tick().catch((error) => {
    console.warn(
      `[scheduler-heartbeat] ${mode} tick failed: ${errorMessage(error)}`
    );
  });
}

export function startMonitorSchedulerHeartbeat() {
  if (process.env.NEXT_RUNTIME && process.env.NEXT_RUNTIME !== "nodejs") {
    return false;
  }
  if (process.env.NODE_ENV !== "development") {
    return false;
  }

  const state = getHeartbeatState();

  // V1.1（2026-06-29）：Next.js dev 模式 hot reload 后 globalThis 可能残留旧 intervalId
  // 引用（timer 已随旧进程销毁但引用未清）。此处强制清除残留并重新启动。
  if (state.intervalId) {
    clearInterval(state.intervalId);
    state.intervalId = null;
  }
  state.dispatchTickCount = 0;
  state.dispatchInFlight = false;
  state.maintenanceInFlight = false;

  state.intervalId = setInterval(() => {
    runBackgroundSchedulerTick("dispatch");
    state.dispatchTickCount += 1;
    if (state.dispatchTickCount % MAINTENANCE_TICK_RATIO === 0) {
      runBackgroundSchedulerTick("maintenance");
    }
  }, HEARTBEAT_INTERVAL_MS);

  if (typeof state.intervalId === "object" && typeof state.intervalId.unref === "function") {
    state.intervalId.unref();
  }

  // V1.1（2026-06-29）：启动确认日志，便于排障。
  console.log(`[scheduler-heartbeat] started — tick every ${HEARTBEAT_INTERVAL_MS / 1000}s`);

  return true;
}

/**
 * Mac mini is a long-running Node process. Keep dispatch and maintenance alive
 * there when an external scheduler cannot authenticate to the public routes.
 */
export function startProductionMonitorSchedulerHeartbeat() {
  if (process.env.NEXT_RUNTIME && process.env.NEXT_RUNTIME !== "nodejs") {
    return false;
  }
  if (process.env.NODE_ENV !== "production") {
    return false;
  }

  const state = getHeartbeatState();
  if (state.intervalId) {
    clearInterval(state.intervalId);
    state.intervalId = null;
  }
  if (state.maintenanceIntervalId) {
    clearInterval(state.maintenanceIntervalId);
    state.maintenanceIntervalId = null;
  }

  runBackgroundSchedulerTick("dispatch");
  runBackgroundSchedulerTick("maintenance");
  state.intervalId = setInterval(() => {
    runBackgroundSchedulerTick("dispatch");
  }, PRODUCTION_DISPATCH_INTERVAL_MS);
  state.maintenanceIntervalId = setInterval(() => {
    runBackgroundSchedulerTick("maintenance");
  }, PRODUCTION_MAINTENANCE_INTERVAL_MS);
  if (
    typeof state.intervalId === "object" &&
    typeof state.intervalId.unref === "function"
  ) {
    state.intervalId.unref();
  }
  if (
    typeof state.maintenanceIntervalId === "object" &&
    typeof state.maintenanceIntervalId.unref === "function"
  ) {
    state.maintenanceIntervalId.unref();
  }

  console.log(
    `[scheduler-production-heartbeat] dispatch every ${
      PRODUCTION_DISPATCH_INTERVAL_MS / 60_000
    }m, maintenance every ${PRODUCTION_MAINTENANCE_INTERVAL_MS / 60_000}m`
  );
  return true;
}

export function stopMonitorSchedulerHeartbeat() {
  const state = globalThis.__topicDashboardMonitorSchedulerHeartbeat__;
  if (!state) return false;
  const stopped = Boolean(state.intervalId || state.maintenanceIntervalId);
  if (state.intervalId) clearInterval(state.intervalId);
  if (state.maintenanceIntervalId) clearInterval(state.maintenanceIntervalId);
  state.intervalId = null;
  state.maintenanceIntervalId = null;
  state.dispatchInFlight = false;
  state.maintenanceInFlight = false;
  state.dispatchTickCount = 0;
  return stopped;
}

/**
 * 最近一次成功 dispatch tick 的完成时间（报告 §10-1 watchdog 门依据）。
 *
 * 进程内 heartbeat 是唯一主叫醒器；外部 HTTP 调用（Vercel/Cloudflare cron）
 * 只在主叫醒器超过新鲜度窗口没有成功 tick 时才被放行兜底。
 */
export async function getLastSuccessfulDispatchTickAt(): Promise<Date | null> {
  const row = await prisma.schedulerTickLog.findFirst({
    where: {
      mode: "dispatch",
      skipped: false,
      errors: { equals: [] },
      finishedAt: { not: null },
    },
    orderBy: { createdAt: "desc" },
    select: { finishedAt: true },
  });
  return row?.finishedAt ?? null;
}
