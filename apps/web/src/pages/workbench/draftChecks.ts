import { requestJson } from '../../shared/api';
import { decode, list, num, str, ShapeError } from '../../shared/decode';
import type { WorkSummary } from './workbenchModel';

export interface DraftCheckSummary {
  revisionId: string;
  must: number;
  warn: number;
  notRun: number;
}

/** The server reads and checks saved revisions; unavailable items remain absent. */
export async function checkSavedDrafts(drafts: WorkSummary[], signal: AbortSignal): Promise<Record<string, DraftCheckSummary>> {
  if (!drafts.length) return {};
  const domain = drafts[0]?.domainRef;
  if (drafts.length > 25 || drafts.some(draft => draft.domainRef !== domain)) throw new Error('invalid draft batch');
  const raw = await requestJson('/ops/api/creation/saved-draft-checks', {
    method: 'POST', signal, body: { domainRef: domain, items: drafts.map(({ workId, revisionId }) => ({ workId, revisionId })) },
  });
  return decode(raw, root => {
    const seen = new Set<string>();
    return Object.fromEntries(list(root, 'items', item => {
    const workId = str(item, 'workId');
    const revisionId = str(item, 'revisionId');
    const expected = drafts.find(draft => draft.workId === workId);
    const must = num(item, 'must'); const warn = num(item, 'warn'); const notRun = num(item, 'notRun');
    if (!expected || seen.has(workId) || expected.revisionId !== revisionId || ![must, warn, notRun].every(n => Number.isSafeInteger(n) && n >= 0)) throw new ShapeError('items');
    seen.add(workId);
    return [workId, { revisionId, must, warn, notRun }];
    }));
  });
}
