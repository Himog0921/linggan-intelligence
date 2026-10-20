import * as d from '../../../shared/decode';
import { requestJson } from '../../../shared/api';

export const relevanceLabels = { related: '相关', not_related: '不相关', unknown: '未知' };
export const perspectiveLabels = { parent_lived_experience: '家长亲历', adult_self_report: '成人自述', professional_education: '专业科普', organization_promotion: '机构推广', other: '其他', unknown: '看不出' };
export const stateLabels = { available: '可用', not_provided: '未提供', not_acquired: '尚未取得', waiting: '尚待取得', known_empty: '已知为空', terminal_failure: '处理已终止', restricted: '受限', not_adopted: '未采用', not_connected: '尚未接通', needs_relayer: '需要重新分层', invalid_source: '来源无效' };
export type Scenario = 'baseline' | 'candidate';
export type Relevance = keyof typeof relevanceLabels;

function choice<T extends string>(root: d.Json, key: string, choices: readonly T[]): T {
  const raw = d.str(root, key); const found = choices.find(value => value === raw);
  if (found === undefined) throw new d.ShapeError(key);
  return found;
}
function fixed(root: d.Json, key: string, expected: string) {
  if (d.str(root, key) !== expected) throw new d.ShapeError(key);
}
function identity(root: d.Json) {
  return { domainRef: d.str(root, 'domainRef'), workRef: d.str(root, 'workRef'), snapshotIdentity: d.str(root, 'snapshotIdentity'), directionVersionRef: d.str(root, 'directionVersionRef'), skillVersionRef: d.str(root, 'skillVersionRef') };
}
function assignment(root: d.Json) {
  return { relevance: choice(root, 'relevance', ['related', 'not_related', 'unknown']), primaryDirection: d.optStr(root, 'primaryDirection'), alsoRelated: d.optStr(root, 'alsoRelated'), reason: d.optStr(root, 'reason') };
}
function judgment(root: d.Json) {
  fixed(root, 'origin', 'offline_mock');
  return { origin: 'offline_mock' as const, identity: identity(d.obj(root, 'identity')), ...assignment(root) };
}
export type Judgment = ReturnType<typeof judgment>;
function output(root: d.Json) {
  fixed(root, 'origin', 'offline_mock'); fixed(root, 'contract', 'note-interpretation-offline.v1');
  return { ...assignment(root), identity: identity(d.obj(root, 'identity')), confidence: choice(root, 'confidence', ['certain', 'possible']), perspective: choice(root, 'perspective', ['parent_lived_experience', 'adult_self_report', 'professional_education', 'organization_promotion', 'other', 'unknown']), keywords: d.list(root, 'keywords', k => ({ text: d.str(k, 'text'), citationId: d.str(k, 'citationId'), start: d.num(k, 'start'), end: d.num(k, 'end') })) };
}
function catalogue(root: d.Json) {
  fixed(root, 'origin', 'offline_mock');
  return { versionRef: d.str(root, 'versionRef'), groups: d.list(root, 'groups', g => ({ groupRef: d.str(g, 'groupRef'), name: d.str(g, 'name') })), directions: d.list(root, 'directions', dir => ({ directionRef: d.str(dir, 'directionRef'), groupRef: d.str(dir, 'groupRef'), name: d.str(dir, 'name'), definition: d.str(dir, 'definition'), exampleWorkRefs: d.strList(dir, 'exampleWorkRefs') })) };
}
function counts(root: d.Json): [string, number][] {
  return Object.keys(root).map(key => [key, d.num(root, key)]);
}
export function parsePreview(value: unknown) {
  return d.decode(value, root => {
    fixed(root, 'origin', 'offline_mock'); fixed(root, 'contract', 'note-interpretation-offline-preview.v1');
    const sample = d.obj(root, 'sample'); fixed(sample, 'origin', 'offline_mock');
    const summary = d.obj(root, 'summary');
    return {
      scenario: choice(root, 'scenario', ['baseline', 'candidate']),
      sample: { manifestSha256: d.str(sample, 'manifestSha256'), fixtureRevision: d.str(sample, 'fixtureRevision'), asOf: d.str(sample, 'asOf') },
      catalogue: catalogue(d.obj(root, 'catalogue')), baselineCatalogue: catalogue(d.obj(root, 'baselineCatalogue')), candidateCatalogue: catalogue(d.obj(root, 'candidateCatalogue')),
      comparison: d.list(root, 'comparison', item => ({ kind: choice(item, 'kind', ['keep', 'merge', 'delete', 'new']), oldRefs: d.strList(item, 'oldRefs'), newRefs: d.strList(item, 'newRefs') })),
      notes: d.list(root, 'notes', note => {
        const input = d.obj(note, 'input'); fixed(input, 'origin', 'offline_mock'); fixed(input, 'contract', 'note-interpretation-offline-input.v1');
        const result = d.obj(note, 'result'); const manual = d.optObj(result, 'judgment');
        return { title: d.str(note, 'title'), likes: d.optNum(note, 'likes'), publishedWeek: d.optStr(note, 'publishedWeek'),
          input: { identity: identity(d.obj(input, 'identity')), columns: d.list(input, 'columns', col => ({ field: choice(col, 'field', ['title', 'cover_headline', 'body', 'tags', 'ocr']), state: choice(col, 'state', Object.keys(stateLabels) as (keyof typeof stateLabels)[]) })), fragments: d.list(input, 'fragments', f => ({ field: d.str(f, 'field'), text: d.str(f, 'text'), citationId: d.str(f, 'citationId'), start: d.num(f, 'start'), end: d.num(f, 'end') })) },
          baseline: output(d.obj(note, 'baseline')), result: { machine: output(d.obj(result, 'machine')), effective: output(d.obj(result, 'effective')), judgment: manual ? judgment(manual) : null, manualApplied: d.bool(result, 'manualApplied'), needsReview: d.bool(result, 'needsReview') } };
      }),
      summary: { total: d.num(summary, 'total'), unknown: d.num(summary, 'unknown'), notRelated: d.num(summary, 'notRelated'), noSuitableDirection: d.num(summary, 'noSuitableDirection'), unknownLikes: d.num(summary, 'unknownLikes'), unknownWeek: d.num(summary, 'unknownWeek'), directions: d.list(summary, 'directions', dir => ({ directionRef: d.str(dir, 'directionRef'), count: d.num(dir, 'count'), possible: d.num(dir, 'possible'), over1000: d.num(dir, 'over1000'), over5000: d.num(dir, 'over5000'), unknownLikes: d.num(dir, 'unknownLikes'), unknownWeek: d.num(dir, 'unknownWeek'), weeks: counts(d.obj(dir, 'weeks')), perspectives: counts(d.obj(dir, 'perspectives')), keywords: d.list(dir, 'keywords', k => ({ text: d.str(k, 'text'), count: d.num(k, 'count') })) })) },
    };
  });
}
export type Preview = ReturnType<typeof parsePreview>;
export type Note = Preview['notes'][number];
export async function fetchPreview(signal?: AbortSignal) { return parsePreview(await requestJson('/api/local/ai-offline-preview', { signal })); }
export async function recompute(scenario: Scenario, judgments: Judgment[]) {
  return parsePreview(await requestJson('/api/local/ai-offline-preview', { method: 'POST', body: { scenario, judgments } }));
}
