import { isRecord, requestJson } from '../../../shared/api';

function object(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) throw new Error('采集管理读取格式无效');
  return value;
}
function text(value: unknown): string {
  if (typeof value !== 'string') throw new Error('采集管理文字字段无效');
  return value;
}
function boolean(value: unknown): boolean {
  if (typeof value !== 'boolean') throw new Error('采集管理状态字段无效');
  return value;
}
function count(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error('采集管理计数字段无效');
  return value;
}
function nullable<T>(value: unknown, decode: (value: unknown) => T): T | null {
  return value === null ? null : decode(value);
}
function list<T>(value: unknown, decode: (value: unknown) => T): T[] {
  if (!Array.isArray(value)) throw new Error('采集管理列表字段无效');
  return value.map(decode);
}
function lane(value: unknown) {
  const row = object(value);
  return { lane: text(row.lane), label: text(row.label), summary: text(row.summary), readable: boolean(row.readable) };
}
function domain(value: unknown) {
  const row = object(value);
  const status = text(row.status);
  if (status !== 'active' && status !== 'paused') throw new Error('领域状态无效');
  return {
    domainRef: text(row.domainRef), name: text(row.name), description: nullable(row.description, text), researchGoal: nullable(row.researchGoal, text), status,
    targetCount: count(row.targetCount), creatorTargetCount: nullable(row.creatorTargetCount, count), keywordTargetCount: nullable(row.keywordTargetCount, count), primaryTargetCount: count(row.primaryTargetCount), referenceTargetCount: count(row.referenceTargetCount), sampleCount: nullable(row.sampleCount, count),
    commentCount: nullable(row.commentCount, count), lastReadable: boolean(row.lastReadable), lastObservation: nullable(row.lastObservation, text), lanes: list(row.lanes, lane),
  };
}
export type DomainEntry = ReturnType<typeof domain>;
function target(value: unknown) {
  const row = object(value);
  return { targetRef: text(row.targetRef), name: text(row.name), kindLabel: text(row.kindLabel) };
}
function relation(value: unknown) {
  const row = object(value);
  const role = text(row.role);
  if (role !== 'primary' && role !== 'reference') throw new Error('领域角色无效');
  return { ...target(row), domainRef: text(row.domainRef), role, lifecycleLabel: text(row.lifecycleLabel) };
}
export function decodeDomains(value: unknown) {
  const row = object(value);
  const common = { asOf: nullable(row.asOf, text), returnTo: text(row.returnTo) };
  if (!boolean(row.readable)) return { ...common, readable: false as const, domains: [], targets: [], relations: [], targetsReadable: false, relationsReadable: false };
  return { ...common, readable: true as const, domains: list(row.domains, domain), targets: list(row.targets, target), relations: list(row.relations, relation), targetsReadable: boolean(row.targetsReadable), relationsReadable: boolean(row.relationsReadable) };
}
export type DomainsRead = ReturnType<typeof decodeDomains>;
function runtimeLane(value: unknown) {
  const row = object(value);
  return { name: text(row.name), needs: text(row.needs), detail: text(row.detail), label: text(row.label), className: text(row.className), capacityState: text(row.capacityState), reasonCode: nullable(row.reasonCode, text), available: boolean(row.available) };
}
function capability(value: unknown) {
  const row = object(value);
  return { capability: text(row.capability), label: text(row.label), stateWord: text(row.stateWord), className: text(row.className), detail: text(row.detail), executionFailures: count(row.executionFailures) };
}
function account(value: unknown) {
  const row = object(value);
  return { accepting: boolean(row.accepting), installationRef: nullable(row.installationRef, text), installationLabel: text(row.installationLabel), accountRef: nullable(row.accountRef, text), credentialValid: boolean(row.credentialValid), bindingState: text(row.bindingState), bindingLabel: text(row.bindingLabel), bindingTone: text(row.bindingTone), observed: nullable(row.observed, text), eligibilityReason: text(row.eligibilityReason), hasLiveLease: boolean(row.hasLiveLease) };
}
function station(value: unknown) {
  const row = object(value);
  return { stationRef: text(row.stationRef), name: text(row.name), stateLabel: text(row.stateLabel), stateTone: text(row.stateTone), pluginVersion: nullable(row.pluginVersion, text), lastSeen: nullable(row.lastSeen, text), dailyUsed: count(row.dailyUsed), dailyQuota: count(row.dailyQuota), quotaExhausted: boolean(row.quotaExhausted), litSegments: count(row.litSegments), claimWindowOpen: boolean(row.claimWindowOpen), dispatchLabel: text(row.dispatchLabel), dispatchHint: text(row.dispatchHint), dispatchAt: nullable(row.dispatchAt, text), capSummary: text(row.capSummary), capTone: text(row.capTone), capabilities: nullable(row.capabilities, value => list(value, capability)), account: nullable(row.account, account), selectorHealth: list(row.selectorHealth, value => { const item = object(value); return { label: text(item.label), tone: text(item.tone), detail: text(item.detail) }; }) };
}
export type StationEntry = ReturnType<typeof station>;
function installation(value: unknown) {
  const row = object(value);
  return { installationRef: text(row.installationRef), pluginVersion: text(row.pluginVersion), browser: text(row.browser), firstSeen: text(row.firstSeen), recent: boolean(row.recent) };
}
function projection(value: unknown) {
  const row = object(value); const board = object(row.board); const drawer = object(row.drawer); const meta = object(row.meta);
  return {
    meta: { vacantStations: nullable(meta.vacantStations, count), unclaimedInstallations: nullable(meta.unclaimedInstallations, count) },
    board: { state: text(board.state), tone: text(board.tone), badge: text(board.badge), headNote: text(board.headNote), metrics: list(board.metrics, value => { const item = object(value); return { label: text(item.label), value: text(item.value) }; }) },
    lanes: nullable(row.lanes, value => list(value, runtimeLane)), stations: nullable(row.stations, value => list(value, station)), unclaimed: nullable(row.unclaimed, value => list(value, installation)), accountObservationAvailable: boolean(row.accountObservationAvailable), staffedCount: nullable(row.staffedCount, count),
    running: nullable(row.running, value => list(value, value => { const item = object(value); return { target: text(item.target), station: text(item.station), started: text(item.started), expires: text(item.expires), units: count(item.units) }; })),
    schedule: nullable(row.schedule, value => list(value, value => { const item = object(value); return { target: text(item.target), interval: text(item.interval), state: text(item.state), last: text(item.last), next: text(item.next) }; })),
    drawer: { coverage: text(drawer.coverage), readCount: count(drawer.readCount), picked: count(drawer.picked), capabilityFailures: count(drawer.capabilityFailures), executionFailures: count(drawer.executionFailures), oldest: nullable(drawer.oldest, text), pauses: nullable(drawer.pauses, value => list(value, value => { const item = object(value); return { reason: text(item.reason), by: text(item.by), at: text(item.at) }; })), dispatched: text(drawer.dispatched), succeeded: text(drawer.succeeded) },
  };
}
export type RuntimeProjection = ReturnType<typeof projection>;
export function decodeRuntime(value: unknown) {
  const row = object(value);
  return { asOf: nullable(row.asOf, text), returnTo: text(row.returnTo), readable: boolean(row.readable), projection: row.readable === true ? projection(row.projection) : null };
}
export type RuntimeRead = ReturnType<typeof decodeRuntime>;
export function managementPath(page: 'domains' | 'runtime') {
  const query = new URLSearchParams(window.location.search);
  const params = new URLSearchParams();
  for (const name of ['domain', 'return_to']) { const value = query.get(name); if (value !== null) params.set(name, value); }
  return `/api/collection/workspace/${page}?${params.toString()}`;
}
export async function loadDomains(signal?: AbortSignal) { return decodeDomains(await requestJson(managementPath('domains'), { signal })); }
export async function loadRuntime(signal?: AbortSignal) { return decodeRuntime(await requestJson(managementPath('runtime'), { signal })); }
