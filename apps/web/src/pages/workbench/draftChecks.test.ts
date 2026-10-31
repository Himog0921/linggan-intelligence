import { beforeEach, describe, expect, it, vi } from 'vitest';
import { requestJson } from '../../shared/api';
import type { WorkSummary } from './workbenchModel';
import { checkSavedDrafts } from './draftChecks';

vi.mock('../../shared/api', async original => ({ ...(await original<typeof import('../../shared/api')>()), requestJson: vi.fn() }));
const draft: WorkSummary = { workId: 'a', domainRef: 'd', title: '稿件', revisionId: 'r1', savedAt: '2026-10-29' };
describe('saved draft checks', () => {
  beforeEach(() => vi.resetAllMocks());
  it('requests counts for saved revisions without sending manuscript content', async () => {
    vi.mocked(requestJson).mockResolvedValue({ items: [{ workId: 'a', revisionId: 'r1', must: 1, warn: 2, notRun: 1 }] });
    const signal = new AbortController().signal;
    expect(await checkSavedDrafts([draft], signal)).toEqual({ a: { revisionId: 'r1', must: 1, warn: 2, notRun: 1 } });
    expect(requestJson).toHaveBeenCalledWith('/ops/api/creation/saved-draft-checks', { method: 'POST', signal, body: { domainRef: 'd', items: [{ workId: 'a', revisionId: 'r1' }] } });
  });
  it('rejects a response for a different revision', async () => {
    vi.mocked(requestJson).mockResolvedValue({ items: [{ workId: 'a', revisionId: 'r2', must: 0, warn: 0, notRun: 0 }] });
    await expect(checkSavedDrafts([draft], new AbortController().signal)).rejects.toThrow();
  });
  it('leaves omitted results unknown', async () => {
    vi.mocked(requestJson).mockResolvedValue({ items: [] });
    expect(await checkSavedDrafts([draft], new AbortController().signal)).toEqual({});
  });
});
