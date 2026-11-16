import { requestJson } from '../../shared/api';
import { bool, decode, field, list, num, obj, optBool, optNum, optObj, optStr, ShapeError, str, type Json } from '../../shared/decode';

export type Tier = 'candidate' | 'established' | 'significant';
export type ComparisonStatus = 'rising' | 'not_rising' | 'incomparable';
export type EvidenceKind = 'comments' | 'works' | 'creators';
export interface Interval { low: number; high: number }
export interface Period { periodStart: string; k: number; n: number; ratePer1k: number | null }
export interface DemandGroup {
  protocolVersion: number | null; mentionersTotal: number; evidenceTier: Tier;
  current: Period & { ciPer1k: Interval | null }; baseline: { k: number; n: number; ratePer1k: number | null };
  series: Period[]; flags: string[];
}
export interface Voice { commentRef: { contentRef: string; commentExternalId: string }; text: string | null; likes: number | null; workId: string; tombstone: boolean }
export interface Creator { creatorId: string; name: string | null; followers: number | null; k: number; n: number; ratePer1k: number; ciPer1k: Interval; observed: boolean | null }
export interface CreatorGroup { protocolVersion: number | null; ranked: Creator[]; insufficient: Creator[] }
export interface Card {
  problem: { id: string; statement: string; level: number; parentId: string | null; redirectedFrom: string | null };
  asOf: string; versions: { taxonomy: number; metricDef: number; judgmentPrompt: string | null; model: string | null };
  demand: { groups: DemandGroup[]; sampleVoices: Voice[]; userMethods: { status: string; reason: string | null; items: { label: string; mentioners: number }[] }; importance: { status: string } };
  supply: { state: string; judgmentStatus: string | null; coverage: Json | null; counts: Json | null; works: Json[] };
  channel: { windowDays: number; minN: number; groups: CreatorGroup[] };
  flags: string[]; exclusions: { reason: string; comments: number }[]; limitations: string[];
}
export interface Comparison { protocolVersion: number | null; currentPeriodStart: string; previousPeriodStart: string; currentRatePer1k: number | null; previousRatePer1k: number | null; status: ComparisonStatus; reason: string | null }
export interface Page { nextCursor: string | null; hasMore: boolean; asOf: string }
export interface CardList { items: Card[]; comparisons: { problemId: string; groups: Comparison[] }[]; page: Page }
export interface EvidenceReply { items: (Voice | { workId: string; basis: string } | { protocolVersion: number | null; section: string; creator: Creator })[]; page: Page; cardAsOf: string; excluded: Json }

function strings(row: Json, key: string): string[] {
  const value = field(row, key);
  if (!Array.isArray(value) || value.some(item => typeof item !== 'string')) throw new ShapeError(key);
  return value as string[];
}
function choice<T extends string>(row: Json, key: string, values: readonly T[]): T {
  const value = str(row, key);
  if (!values.some(item => item === value)) throw new ShapeError(key);
  return value as T;
}
function nullableNumber(row: Json, key: string) { field(row, key); return optNum(row, key); }
function nullableString(row: Json, key: string) { field(row, key); return optStr(row, key); }
function interval(row: Json): Interval { return { low: num(row, 'low'), high: num(row, 'high') }; }
function period(row: Json): Period { return { periodStart: str(row, 'periodStart'), k: num(row, 'k'), n: num(row, 'n'), ratePer1k: nullableNumber(row, 'ratePer1k') }; }
function creator(row: Json): Creator {
  field(row, 'observed');
  return { creatorId: str(row, 'creatorId'), name: nullableString(row, 'name'), followers: nullableNumber(row, 'followers'), k: num(row, 'k'), n: num(row, 'n'), ratePer1k: num(row, 'ratePer1k'), ciPer1k: interval(obj(row, 'ciPer1k')), observed: optBool(row, 'observed') };
}
function voice(row: Json): Voice {
  const ref = obj(row, 'commentRef');
  const result = { commentRef: { contentRef: str(ref, 'contentRef'), commentExternalId: str(ref, 'commentExternalId') }, text: nullableString(row, 'text'), likes: nullableNumber(row, 'likes'), workId: str(row, 'workId'), tombstone: bool(row, 'tombstone') };
  if (result.tombstone && result.text !== null) throw new ShapeError('tombstone');
  return result;
}
function demandGroup(row: Json): DemandGroup {
  const current = obj(row, 'current'); const ci = optObj(current, 'ciPer1k'); field(current, 'ciPer1k');
  const base = obj(row, 'baseline');
  return { protocolVersion: nullableNumber(row, 'protocolVersion'), mentionersTotal: num(row, 'mentionersTotal'), evidenceTier: choice(row, 'evidenceTier', ['candidate', 'established', 'significant']),
    current: { ...period(current), ciPer1k: ci ? interval(ci) : null }, baseline: { k: num(base, 'k'), n: num(base, 'n'), ratePer1k: nullableNumber(base, 'ratePer1k') },
    series: list(row, 'series', period), flags: strings(row, 'flags') };
}
function card(root: Json): Card {
  const problem = obj(root, 'problem'); const versions = obj(root, 'versions'); const demand = obj(root, 'demand');
  const methods = obj(demand, 'userMethods'); const importance = obj(demand, 'importance'); const supply = obj(root, 'supply'); const channel = obj(root, 'channel');
  const methodStatus = choice(methods, 'status', ['available', 'unavailable']);
  const works = field(supply, 'works');
  if (!Array.isArray(works) || works.some(item => typeof item !== 'object' || item === null || Array.isArray(item))) throw new ShapeError('works');
  field(supply, 'coverage'); field(supply, 'counts');
  return {
    problem: { id: str(problem, 'id'), statement: str(problem, 'statement'), level: num(problem, 'level'), parentId: nullableString(problem, 'parentId'), redirectedFrom: nullableString(problem, 'redirectedFrom') },
    asOf: str(root, 'asOf'), versions: { taxonomy: num(versions, 'taxonomy'), metricDef: num(versions, 'metricDef'), judgmentPrompt: nullableString(versions, 'judgmentPrompt'), model: nullableString(versions, 'model') },
    demand: { groups: list(demand, 'groups', demandGroup), sampleVoices: list(demand, 'sampleVoices', voice),
      userMethods: { status: methodStatus, reason: methodStatus === 'unavailable' ? str(methods, 'reason') : null, items: methodStatus === 'available' ? list(methods, 'items', item => ({ label: str(item, 'label'), mentioners: num(item, 'mentioners') })) : [] },
      importance: { status: choice(importance, 'status', ['not_calibrated', 'calibrated']) } },
    supply: { state: choice(supply, 'state', ['not_evaluated', 'awaiting_understanding', 'insufficient_coverage', 'covered', 'suspected_gap']), judgmentStatus: nullableString(supply, 'judgmentStatus'), coverage: optObj(supply, 'coverage'), counts: optObj(supply, 'counts'), works: works as Json[] },
    channel: { windowDays: num(channel, 'windowDays'), minN: num(channel, 'minN'), groups: list(channel, 'groups', group => ({ protocolVersion: nullableNumber(group, 'protocolVersion'), ranked: list(group, 'ranked', creator), insufficient: list(group, 'insufficient', creator) })) },
    flags: strings(root, 'flags'), exclusions: list(root, 'exclusions', item => ({ reason: str(item, 'reason'), comments: num(item, 'comments') })), limitations: strings(root, 'limitations'),
  };
}
function page(row: Json): Page {
  const result = { nextCursor: nullableString(row, 'nextCursor'), hasMore: bool(row, 'hasMore'), asOf: str(row, 'asOf') };
  if (result.hasMore !== (result.nextCursor !== null)) throw new ShapeError('nextCursor');
  return result;
}
function comparison(row: Json): Comparison {
  return { protocolVersion: nullableNumber(row, 'protocolVersion'), currentPeriodStart: str(row, 'currentPeriodStart'), previousPeriodStart: str(row, 'previousPeriodStart'), currentRatePer1k: nullableNumber(row, 'currentRatePer1k'), previousRatePer1k: nullableNumber(row, 'previousRatePer1k'), status: choice(row, 'status', ['rising', 'not_rising', 'incomparable']), reason: nullableString(row, 'reason') };
}
export async function readCards(domain: string, cursor: string | null, signal: AbortSignal): Promise<CardList> {
  const query = new URLSearchParams({ includeWeeklyComparison: 'true', limit: '50' });
  if (cursor) query.set('cursor', cursor);
  const value = await requestJson(`/api/local/opportunity/domains/${encodeURIComponent(domain)}/opportunities?${query}`, { signal });
  return decode(value, root => {
    if (str(root, 'domainRef') !== domain) throw new ShapeError('domainRef');
    const items = list(root, 'items', card); const comparisons = list(root, 'weeklyComparisons', item => ({ problemId: str(item, 'problemId'), groups: list(item, 'groups', comparison) }));
    if (items.length !== comparisons.length || items.some((item, index) => item.problem.id !== comparisons[index]?.problemId || item.demand.groups.length !== comparisons[index]?.groups.length || item.demand.groups.some((group, i) => group.protocolVersion !== comparisons[index]?.groups[i]?.protocolVersion))) throw new ShapeError('weeklyComparisons');
    return { items, comparisons, page: page(obj(root, 'page')) };
  });
}
export async function readCard(domain: string, problem: string, signal: AbortSignal): Promise<Card> {
  return decode(await requestJson(`/api/local/opportunity/domains/${encodeURIComponent(domain)}/problems/${encodeURIComponent(problem)}/card`, { signal }), card);
}
export async function readEvidence(domain: string, problem: string, kind: EvidenceKind, cursor: string | null, signal: AbortSignal): Promise<EvidenceReply> {
  const query = new URLSearchParams({ kind, limit: '30' }); if (cursor) query.set('cursor', cursor);
  return decode(await requestJson(`/api/local/opportunity/domains/${encodeURIComponent(domain)}/problems/${encodeURIComponent(problem)}/evidence?${query}`, { signal }), root => {
    const actualKind = choice(root, 'kind', ['comments', 'works', 'creators']);
    if (actualKind !== kind) throw new ShapeError('kind');
    return { items: list(root, 'items', item => kind === 'comments' ? voice(item) : kind === 'works' ? { workId: str(item, 'workId'), basis: str(item, 'basis') } : { protocolVersion: nullableNumber(item, 'protocolVersion'), section: choice(item, 'section', ['ranked', 'insufficient']), creator: creator(obj(item, 'creator')) }), page: page(obj(root, 'page')), cardAsOf: str(root, 'cardAsOf'), excluded: obj(root, 'excluded') };
  });
}
