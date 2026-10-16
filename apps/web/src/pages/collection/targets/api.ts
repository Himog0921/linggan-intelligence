import { isRecord, requestJson } from '../../../shared/api';

// All incoming fields are decoded before rendering; reads never substitute a zero/empty
// collection for an unknown server fact. These DTOs belong only to the Collection workspace.
export function record(v: unknown): Record<string, unknown> {
  if (!isRecord(v)) throw new Error('采集数据格式不符');
  return v;
}
function str(v: unknown): string {
  if (typeof v !== 'string') throw new Error('采集文字格式不符');
  return v;
}
function num(v: unknown): number {
  if (typeof v !== 'number' || !Number.isFinite(v)) throw new Error('采集数字格式不符');
  return v;
}
function bool(v: unknown): boolean {
  if (typeof v !== 'boolean') throw new Error('采集状态格式不符');
  return v;
}
function nullable<T>(v: unknown, decode: (v: unknown) => T): T | null {
  return v === null ? null : decode(v);
}
function array<T>(v: unknown, decode: (v: unknown) => T): T[] {
  if (!Array.isArray(v)) throw new Error('采集列表格式不符');
  return v.map(decode);
}
export function decodeAction(v: unknown) {
  const r = record(v);
  return { kind: str(r.kind), label: str(r.label), tab: str(r.tab), fragment: str(r.fragment) };
}
export type TargetAction = ReturnType<typeof decodeAction>;
export function decodeTarget(v: unknown) {
  const r = record(v),
    a = record(r.avatar);
  return {
    targetRef: str(r.targetRef),
    kind: str(r.kind),
    platform: str(r.platform),
    identity: str(r.identity),
    handle: str(r.handle),
    name: str(r.name),
    group: str(r.group),
    domainName: nullable(r.domainName, str),
    lifecycle: str(r.lifecycle),
    monitoring: bool(r.monitoring),
    avatar: { state: str(a.state), path: nullable(a.path, str) },
    archiveLabel: str(r.archiveLabel),
    archiveTone: str(r.archiveTone),
    patrolLabel: str(r.patrolLabel),
    patrolTone: str(r.patrolTone),
    works: str(r.works),
    details: str(r.details),
    recentNew: nullable(r.recentNew, num),
    recentHits: nullable(r.recentHits, num),
    last: nullable(r.last, str),
    next: nullable(r.next, str),
    action: decodeAction(r.action),
  };
}
export type Target = ReturnType<typeof decodeTarget>;
export function decodeList(v: unknown) {
  const r = record(v);
  return {
    rows: array(r.rows, decodeTarget),
    currentDomain: nullable(r.currentDomain, str),
    limit: num(r.limit),
    counts: nullable(r.counts, (v) => {
      const c = record(v);
      return {
        total: num(c.total),
        creator: num(c.creator),
        keyword: num(c.keyword),
        archiving: num(c.archiving),
        monitoring: num(c.monitoring),
      };
    }),
    domains: array(r.domains, (v) => {
      const d = record(v);
      return { domainRef: str(d.domainRef), name: str(d.name), status: str(d.status) };
    }),
  };
}
export type TargetList = ReturnType<typeof decodeList>;
function decodeWork(v: unknown) {
  const w = record(v);
  return {
    publicRef: str(w.publicRef),
    externalId: str(w.externalId),
    title: nullable(w.title, str),
    creatorName: nullable(w.creatorName, str),
    creatorId: nullable(w.creatorId, str),
    position: nullable(w.position, num),
    published: nullable(w.published, str),
    source: str(w.source),
    kind: nullable(w.kind, str),
    complete: bool(w.complete),
    detailLabel: str(w.detailLabel),
    detailState: str(w.detailState),
    media: str(w.media),
    comments: nullable(w.comments, num),
    last: nullable(w.last, str),
  };
}
export type CatalogWork = ReturnType<typeof decodeWork>;
function decodeCount(v: unknown) {
  const r = record(v),
    state = str(r.state);
  if (state !== 'known' && state !== 'unknown') throw new Error('采集计数状态不符');
  return state === 'known' ? num(r.value) : null;
}
function decodeInspector(v: unknown) {
  const i = record(v),
    a = record(i.archive),
    e = record(i.execution),
    p = record(i.patrol),
    c = record(i.coverage);
  return {
    archive: str(a.state),
    execution: str(e.state),
    patrol: str(p.state),
    last: nullable(p.lastSucceededAt, str),
    next: nullable(p.nextRunAt, str),
    hits: decodeCount(p.latestHits),
    latestNew: decodeCount(p.latestNew),
    works: decodeCount(c.directoryWorks),
    details: decodeCount(c.capturedDetails),
    missing: decodeCount(c.missingDetails),
    quarantined: decodeCount(c.quarantinedRecords),
    blocked: decodeCount(c.blockedDetails),
  };
}
function decodeRule(v: unknown) {
  const r = record(v);
  return {
    ruleRef: str(r.ruleRef),
    slot: str(r.slot),
    enabled: bool(r.enabled),
    interval: nullable(r.interval, num),
    scroll: nullable(r.scroll, num),
    top: nullable(r.top, num),
    days: nullable(r.days, num),
    next: nullable(r.next, str),
    last: nullable(r.last, str),
    revision: num(r.revision),
  };
}
function decodePoint(v: unknown) {
  const p = record(v);
  return {
    publicRef: str(p.publicRef),
    title: nullable(p.title, str),
    published: str(p.published),
    date: str(p.date),
    epochMs: num(p.epochMs),
    value: num(p.value),
    discussionRate: nullable(p.discussionRate, num),
    association: str(p.association),
    new: bool(p.new),
  };
}
function decodeProjection(v: unknown) {
  const p = record(v),
    s = record(p.summary),
    e = record(p.exclusions),
    r = record(p.receipt),
    window = str(p.window);
  return {
    status: str(p.status),
    asOf: str(p.asOf),
    // The owner enum's serde form keeps the digit adjacent to "recent";
    // navigation uses the owner's existing as_str / parse query spelling.
    window: window === 'recent90_days' ? 'recent_90_days' : window,
    metric: str(p.metric),
    linked: nullable(s.linkedWorkCount, num),
    lowerBound: num(s.linkedWorkCountLowerBound),
    confirmed: num(s.confirmedAuthorWorkCount),
    eligible: num(s.eligiblePointCount),
    exclusions: {
      authorNotVerified: num(e.authorNotVerified),
      authorMismatch: num(e.authorMismatch),
      publishedAtNotQualified: num(e.publishedAtNotQualified),
      outsideWindow: num(e.outsideWindow),
      metricUnknown: num(e.metricUnknown),
      scanTruncated: bool(e.scanTruncated),
    },
    receipt: {
      scanLimit: num(r.scanLimit),
      probedCount: num(r.probedCount),
      scannedCount: num(r.scannedCount),
      returnedCount: num(r.returnedCount),
      truncated: bool(r.truncated),
    },
    points: array(p.points, decodePoint),
  };
}
export type Lifecycle = ReturnType<typeof decodeProjection>;
export function decodeDrawer(v: unknown) {
  const r = record(v),
    d = record(r.decision),
    l = record(r.lifecycle);
  return {
    target: decodeTarget(r.target),
    contentGapAction: nullable(r.contentGapAction, decodeAction),
    decision: { title: str(d.title), note: str(d.note), action: nullable(d.action, decodeAction) },
    inspector: nullable(r.inspector, decodeInspector),
    catalog: nullable(r.catalog, (v) => array(v, decodeWork)),
    rules: nullable(r.rules, (v) => array(v, decodeRule)),
    blocked: nullable(r.blocked, (v) =>
      array(v, (b) => {
        const r = record(b);
        return { publicRef: str(r.publicRef), externalId: str(r.externalId), title: nullable(r.title, str) };
      }),
    ),
    lifecycle: {
      state: str(l.state),
      projection: l.state === 'ready' ? decodeProjection(l.projection) : null,
    },
  };
}
export type Drawer = ReturnType<typeof decodeDrawer>;
export function decodeRulePanel(v: unknown) {
  const r = record(v),
    f = record(r.form);
  return {
    targetRef: str(r.targetRef),
    name: str(r.name),
    kind: str(r.kind),
    lifecycle: str(r.lifecycle),
    slot: nullable(r.slot, str),
    rankings: array(r.rankings, str),
    form: {
      expected_revision: num(f.expected_revision),
      idempotency_key: str(f.idempotency_key),
      automatic_enabled: bool(f.automatic_enabled),
      creator_follow_details: bool(f.creator_follow_details),
      fixed_interval_seconds: str(f.fixed_interval_seconds),
      surface_key: str(f.surface_key),
      ranking_key: str(f.ranking_key),
      scroll_rounds: str(f.scroll_rounds),
      top_by_likes: str(f.top_by_likes),
      published_within_days: str(f.published_within_days),
      task_contract_version: str(f.task_contract_version),
    },
    receipt: nullable(r.receipt, (v) => {
      const q = record(v);
      return {
        receiptRef: str(q.receiptRef),
        outcome: str(q.outcome),
        reason: str(q.reason),
        expectedRevision: num(q.expectedRevision),
        recordedAt: str(q.recordedAt),
        appliedRuleRevisionRef: nullable(q.appliedRuleRevisionRef, str),
      };
    }),
  };
}
export type RulePanel = ReturnType<typeof decodeRulePanel>;
export function decodeDeletion(v: unknown) {
  const r = record(v);
  return {
    targetRef: str(r.targetRef),
    name: str(r.name),
    blocked: bool(r.blocked),
    rules: num(r.rules),
    requests: num(r.requests),
    workOrders: num(r.workOrders),
    leases: num(r.leases),
    tasks: num(r.tasks),
    retainedWorks: num(r.retainedWorks),
    retainedDetails: num(r.retainedDetails),
    blockingMaterialUsages: num(r.blockingMaterialUsages),
    blockingWorkOrderUsages: num(r.blockingWorkOrderUsages),
    blockingRiskSignals: num(r.blockingRiskSignals),
    retirements: num(r.retirements),
  };
}
export type Deletion = ReturnType<typeof decodeDeletion>;
export async function readList(query: string, signal: AbortSignal) {
  return decodeList(await requestJson(`/api/collection/workspace/targets?${query}`, { signal }));
}

export function decodeAssignment(value: unknown) {
  const r = record(value);
  return { targetRef: str(r.targetRef), name: str(r.name), assignable: bool(r.assignable) };
}
