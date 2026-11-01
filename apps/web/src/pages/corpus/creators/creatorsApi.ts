import { ApiError, isRecord, requestJson } from '../../../shared/api';
import {
  bool,
  decode,
  list,
  num,
  obj,
  optBool,
  optNum,
  optObj,
  optStr,
  ShapeError,
  str,
  strList,
  type Json,
} from '../../../shared/decode';
import { toApiParams, type CreatorQuery } from './creatorsQuery';
import type {
  Analysis,
  CreatorDomain,
  CreatorItem,
  CreatorList,
  CreatorPolicy,
  CreatorStats,
  Fragment,
  Judgement,
  ManualEntry,
  Observation,
  ObservationPatrol,
  ObservationRecord,
  ObservationTransition,
  RegisterReceipt,
  Work,
  WorkResource,
} from './creatorsTypes';

/** 读写合同：`apps/api/src/local_web/creators.rs`。 */
const API = '/api/local';

// ───────────────────────── 解码 ─────────────────────────

function parseJudgement(record: Json): Judgement {
  return {
    value: optStr(record, 'value') ?? 'unknown',
    reason: optStr(record, 'reason'),
    evidenceFragmentIds: strList(record, 'evidenceFragmentIds'),
  };
}

function optJudgement(record: Json, name: string): Judgement | null {
  const value = optObj(record, name);
  return value === null ? null : parseJudgement(value);
}

function parseFragment(record: Json): Fragment {
  return {
    fragmentId: str(record, 'fragmentId'),
    field: optStr(record, 'field') ?? 'material',
    sourceType: optStr(record, 'sourceType'),
    workRef: optStr(record, 'workRef'),
    workTitle: optStr(record, 'workTitle'),
    text: optStr(record, 'text') ?? '',
  };
}

function optFragments(record: Json, name: string): Fragment[] {
  const value = record[name];
  if (value === undefined || value === null) return [];
  return list(record, name, parseFragment);
}

function parseManual(record: Json): ManualEntry {
  // 人工调整的有效值是 value.value；原人工记录还在、但支持它的材料已不可读时带 supportUnavailable。
  const inner = optObj(record, 'value');
  return {
    judgement: inner === null ? { value: 'unknown', reason: null, evidenceFragmentIds: [] } : parseJudgement(inner),
    supportFragmentIds: strList(record, 'supportFragmentIds'),
    at: optStr(record, 'at'),
    supportUnavailable: optBool(record, 'supportUnavailable') === true,
  };
}

/** 作品与作者的分析记录结构相同：没有记录时服务器给 `{}`，所有字段都是未知而不是 0。 */
export function parseAnalysis(value: unknown): Analysis {
  const record: Json = isRecord(value) ? value : {};
  const automatic = optObj(record, 'automatic') ?? {};
  const traitsSource = optObj(automatic, 'traits') ?? {};
  const traits: Record<string, Judgement> = {};
  for (const [name, entry] of Object.entries(traitsSource)) {
    if (isRecord(entry)) traits[name] = parseJudgement(entry);
  }
  const manualSource = optObj(record, 'manual') ?? {};
  const manual: Record<string, ManualEntry> = {};
  for (const [name, entry] of Object.entries(manualSource)) {
    if (isRecord(entry)) manual[name] = parseManual(entry);
  }
  const automaticEvidence = optFragments(automatic, 'evidence');
  const evidence = optFragments(record, 'evidence');
  const supportFragments = optFragments(record, 'supportFragments');
  const identitySupportFragments = optFragments(record, 'identitySupportFragments');
  return {
    origin: optStr(record, 'origin'),
    state: optStr(record, 'state'),
    lastErrorCode: optStr(record, 'lastErrorCode'),
    resultAt: optStr(record, 'resultAt'),
    resultAtDisplay: optStr(record, 'resultAtDisplay'),
    relevance: optJudgement(automatic, 'relevance'),
    traits,
    focus: optJudgement(automatic, 'focus'),
    institution: optJudgement(record, 'institution_or_brand'),
    sampleWorkCount: optNum(automatic, 'sampleWorkCount'),
    availableWorkCount: optNum(automatic, 'availableWorkCount'),
    sampleBasis: optStr(automatic, 'sampleBasis'),
    sampleLimitations: strList(automatic, 'sampleLimitations'),
    manual,
    fragments: dedupeFragments([...automaticEvidence, ...evidence, ...supportFragments, ...identitySupportFragments]),
    supportFragments,
    identitySupportFragments,
  };
}

function dedupeFragments(fragments: Fragment[]): Fragment[] {
  const seen = new Set<string>();
  return fragments.filter(fragment => (seen.has(fragment.fragmentId) ? false : (seen.add(fragment.fragmentId), true)));
}

function parseWork(record: Json): Work {
  return {
    workRef: str(record, 'workRef'),
    title: optStr(record, 'title'),
    relevance: optStr(record, 'relevance') ?? 'unknown',
    likes: optNum(record, 'likes'),
    collects: optNum(record, 'collects'),
    comments: optNum(record, 'comments'),
    acquisitionKind: optStr(record, 'acquisitionKind'),
    firstAdded: optStr(record, 'firstAdded'),
    firstAddedDisplay: optStr(record, 'firstAddedDisplay'),
    likesObservedAt: optStr(record, 'likesObservedAt'),
    likesObservedDisplay: optStr(record, 'likesObservedDisplay'),
    analysis: parseAnalysis(record.analysis),
  };
}

function parseObservation(record: Json): Observation {
  const lookup = optStr(record, 'lookupState') === 'unavailable' ? 'unavailable' : 'available';
  return {
    lookup,
    inCurrentDomain: optBool(record, 'inCurrentDomain'),
    monitoringState: optStr(record, 'monitoringState'),
    lifecycleState: optStr(record, 'lifecycleState'),
    usageRole: optStr(record, 'usageRole'),
    targetRef: optStr(record, 'targetRef'),
    existsInOtherDomains: optBool(record, 'existsInOtherDomains'),
    hasOtherPrimaryDomain: optBool(record, 'hasOtherPrimaryDomain'),
  };
}

function parseItem(record: Json): CreatorItem {
  const profile = optObj(record, 'profile') ?? {};
  const source = optObj(record, 'displayNameSource');
  return {
    creatorKey: str(record, 'creatorKey'),
    displayName: optStr(record, 'displayName'),
    displayNameKind: source === null ? null : optStr(source, 'kind'),
    authorExternalId: optStr(record, 'authorExternalId'),
    followerCount: optNum(profile, 'followerCount'),
    followerObservedAt: optStr(profile, 'observedAt'),
    biography: optStr(profile, 'biography'),
    focus: optJudgement(record, 'focus') ?? { value: 'unknown', reason: null, evidenceFragmentIds: [] },
    authorAnalysis: parseAnalysis(record.authorAnalysis),
    traits: strList(record, 'traits'),
    topicHints: strList(record, 'topicHints'),
    discoveryKeywords: strList(record, 'discoveryKeywords'),
    firstSeenWorkRef: optStr(record, 'firstSeenWorkRef'),
    firstSeenMs: optNum(record, 'firstSeenMs'),
    firstSeenDate: optStr(record, 'firstSeenDate'),
    firstSeenDisplay: optStr(record, 'firstSeenDisplay'),
    firstSeenPrecision: optStr(record, 'firstSeenPrecision'),
    observation: parseObservation(obj(record, 'observation')),
    collectedWorkCount: num(record, 'collectedWorkCount'),
    relatedWorkCount: num(record, 'relatedWorkCount'),
    matchedRelatedWorkCount: num(record, 'matchedRelatedWorkCount'),
    unrelatedWorkCount: num(record, 'unrelatedWorkCount'),
    unknownRelevanceWorkCount: num(record, 'unknownRelevanceWorkCount'),
    viralWorkCount: optNum(record, 'viralWorkCount'),
    maxRelatedLikeCount: optNum(record, 'maxRelatedLikeCount'),
    candidateHighLikeCount: optNum(record, 'candidateHighLikeCount'),
    matchedWorkRefs: strList(record, 'matchedWorkRefs'),
    representatives: list(record, 'representatives', parseWork),
  };
}

function parseStats(record: Json): CreatorStats {
  return {
    authors: num(record, 'authors'),
    works: num(record, 'works'),
    unknownAuthorWorks: num(record, 'unknownAuthorWorks'),
    relatedAuthors: num(record, 'relatedAuthors'),
    vertical: num(record, 'vertical'),
    personal: num(record, 'personal'),
    outside: optNum(record, 'outside'),
    inside: optNum(record, 'inside'),
    outsideViral: optNum(record, 'outsideViral'),
    recentlyDiscovered: num(record, 'recentlyDiscovered'),
    highLikeLeads: optNum(record, 'highLikeLeads'),
    latestAddedMs: optNum(record, 'latestAddedMs'),
    latestAddedDisplay: optStr(record, 'latestAddedDisplay'),
    unknownDateWorks: num(record, 'unknownDateWorks'),
  };
}

function parsePolicy(record: Json): CreatorPolicy {
  return {
    likeThreshold: optNum(record, 'likeThreshold'),
    revision: num(record, 'revision'),
  };
}

function parseDomain(record: Json): CreatorDomain {
  return {
    domainRef: str(record, 'domainRef'),
    name: optStr(record, 'name'),
    description: optStr(record, 'description'),
    researchGoal: optStr(record, 'researchGoal'),
    status: str(record, 'status'),
  };
}

export function parseCreatorList(value: unknown): CreatorList {
  return decode(value, root => {
    const lead = optObj(root, 'leadRule');
    return {
      domain: parseDomain(obj(root, 'domain')),
      policy: parsePolicy(obj(root, 'policy')),
      stats: parseStats(obj(root, 'stats')),
      leadRule: lead === null ? null : { minLikes: num(lead, 'minLikes'), minWorks: num(lead, 'minWorks') },
      total: num(root, 'total'),
      items: list(root, 'items', parseItem),
      asOf: optStr(root, 'asOf'),
      displayAsOf: optStr(root, 'displayAsOf'),
    };
  });
}

/** 只接受同源的本机媒体路径；其他任何地址都不当图片用。 */
export function safeAssetUrl(url: unknown, origin: string = globalThis.location?.origin ?? 'http://localhost'): string | null {
  if (typeof url !== 'string' || !url.startsWith('/api/local/') || url.startsWith('//')) return null;
  try {
    const parsed = new URL(url, origin);
    return parsed.origin === origin && /^\/api\/local\/(media|derivative)\//.test(parsed.pathname) ? parsed.pathname + parsed.search : null;
  } catch {
    return null;
  }
}

export function parseWorkResources(value: unknown): WorkResource[] {
  return decode(value, root =>
    list(root, 'items', item => {
      const identity = obj(item, 'identity');
      const display = optObj(item, 'display');
      const media = optObj(item, 'media');
      const avatar = media === null ? null : optObj(media, 'avatar');
      const avatarBlob = avatar === null ? null : optObj(avatar, 'blob');
      const cover = media === null ? null : optObj(media, 'cover');
      return {
        publicRef: str(identity, 'publicRef'),
        title: display === null ? null : optStr(display, 'title'),
        avatarUrl: avatar !== null && avatarBlob !== null && optStr(avatarBlob, 'deliveryState') === 'INLINE_SAFE'
          ? safeAssetUrl(avatar.localAssetUrl)
          : null,
        coverUrl: cover === null ? null : safeAssetUrl(cover.localAssetUrl),
      };
    }),
  );
}

export function parseRegisterReceipt(value: unknown): RegisterReceipt {
  return decode(value, root => ({
    targetRef: optStr(root, 'targetRef'),
    created: bool(root, 'created'),
    relationshipAdded: bool(root, 'relationshipAdded'),
    inCurrentDomain: bool(root, 'inCurrentDomain'),
    usageRole: str(root, 'usageRole'),
    lifecycleState: optStr(root, 'lifecycleState'),
    monitoringEnabled: optBool(root, 'monitoringEnabled'),
  }));
}

function parseObservationTransition(value: Json | null): ObservationTransition | null {
  if (value === null) return null;
  return {
    at: str(value, 'at'),
    from: optStr(value, 'from'),
    to: str(value, 'to'),
    reasonCode: str(value, 'reasonCode'),
    reason: optStr(value, 'reason'),
  };
}

function parseObservationPatrol(value: Json | null): ObservationPatrol | null {
  if (value === null) return null;
  return {
    automaticEnabled: bool(value, 'automaticEnabled'),
    lastDispatchedAt: optStr(value, 'lastDispatchedAt'),
    lastSucceededAt: optStr(value, 'lastSucceededAt'),
    nextRunAt: optStr(value, 'nextRunAt'),
    missedRuns: num(value, 'missedRuns'),
    latestHits: optNum(value, 'latestHits'),
    latestNew: optNum(value, 'latestNew'),
    state: str(value, 'state'),
  };
}

export function parseObservationRecord(value: unknown): ObservationRecord {
  return decode(value, root => {
    const state = str(root, 'state');
    if (state !== 'joined' && state !== 'not_joined') throw new ShapeError('state');
    return {
      state,
      targetRef: optStr(root, 'targetRef'),
      joinedAt: optStr(root, 'joinedAt'),
      usageRole: optStr(root, 'usageRole'),
      firstStoredAt: optStr(root, 'firstStoredAt'),
      lifecycleState: optStr(root, 'lifecycleState'),
      lifecycleChangedAt: optStr(root, 'lifecycleChangedAt'),
      monitoringState: optStr(root, 'monitoringState'),
      patrol: parseObservationPatrol(optObj(root, 'patrol')),
      lastTransition: parseObservationTransition(optObj(root, 'lastTransition')),
    };
  });
}

export async function fetchObservationRecord(domain: string, creatorKey: string, signal?: AbortSignal): Promise<ObservationRecord> {
  const params = new URLSearchParams({ domain });
  return parseObservationRecord(
    await requestJson(`${API}/creators/${encodeURIComponent(creatorKey)}/observation-record?${params}`, signal ? { signal } : {}),
  );
}

// ───────────────────────── 请求 ─────────────────────────

export async function fetchCreators(domain: string, query: CreatorQuery, signal?: AbortSignal): Promise<CreatorList> {
  const params = toApiParams(domain, query);
  return parseCreatorList(await requestJson(`${API}/creators?${params}`, signal ? { signal } : {}));
}

/** 批量读取作品资源（标题、头像、封面）。一次最多 100 个。 */
export async function fetchWorkResources(domain: string, refs: readonly string[], signal?: AbortSignal): Promise<WorkResource[]> {
  const unique = [...new Set(refs)].slice(0, 100);
  if (unique.length === 0) return [];
  const params = new URLSearchParams({ domain, publicRefs: unique.join(',') });
  return parseWorkResources(await requestJson(`${API}/work-resources?${params}`, signal ? { signal } : {}));
}

export interface RegisterRequest {
  domain: string;
  creatorKey: string;
  usageRole: 'primary' | 'reference';
}

/**
 * 加入观察。接口没有请求编号，但天然幂等：按（作者键，领域）在服务器上加锁、`ON CONFLICT DO NOTHING`，
 * 重复提交得到同一个观察目标和同一个关系。所以重试发送的就是同一个请求体。
 */
export async function registerCreator(request: RegisterRequest): Promise<RegisterReceipt> {
  const body = { domain: request.domain, usageRole: request.usageRole };
  return parseRegisterReceipt(
    await requestJson(`${API}/creators/${encodeURIComponent(request.creatorKey)}/observation-target`, { method: 'POST', body }),
  );
}

export interface PolicyRequest {
  domain: string;
  revision: number;
  likeThreshold: number | null;
}

export async function savePolicy(request: PolicyRequest): Promise<{ revision: number }> {
  const body: Record<string, unknown> = {
    domain: request.domain,
    revision: request.revision,
    likeThreshold: request.likeThreshold,
  };
  const receipt = await requestJson(`${API}/creator-discovery-policy`, { method: 'PUT', body });
  if (!isRecord(receipt) || typeof receipt.revision !== 'number' || receipt.saved !== true) {
    throw new ApiError({ kind: 'invalid_response', status: 200 });
  }
  return { revision: receipt.revision };
}

export interface OverrideRequest {
  domain: string;
  creatorKey: string;
  workRef: string | null;
  field: string;
  /** null 表示撤销人工调整。 */
  value: string | null;
  reason: string | null;
  supportFragmentIds: string[];
}

export async function saveOverride(request: OverrideRequest): Promise<void> {
  const receipt = await requestJson(`${API}/creator-discovery-overrides`, { method: 'PUT', body: request });
  if (!isRecord(receipt) || receipt.saved !== true) throw new ApiError({ kind: 'invalid_response', status: 200 });
}
