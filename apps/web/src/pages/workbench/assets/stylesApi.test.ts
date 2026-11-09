import { beforeEach, describe, expect, it, vi } from 'vitest';
import { requestJson } from '../../../shared/api';
import { ShapeError } from '../../../shared/decode';
import { createStyle, listStyles, parseStyle } from './stylesApi';
vi.mock('../../../shared/api', async () => {
  const actual = await vi.importActual<typeof import('../../../shared/api')>('../../../shared/api');
  return { ...actual, requestJson: vi.fn() };
});
const domain = '00000000-0000-4000-8000-000000000001';
const account = '00000000-0000-4000-8000-000000000002';
const style = { code: 'FG-0001', accountRef: account, revision: 1, name: '她本人 · 原声', sourceNote: '她自己的稿子 1 篇', structure: ['先说结论'], rhythm: [], wording: ['稳'], skip: [], savedAt: 'now' };
beforeEach(() => vi.clearAllMocks());
describe('风格库接口', () => {
  it('reads styles for one domain and keeps every part of the style', async () => {
    vi.mocked(requestJson).mockResolvedValue({ items: [style] });
    expect(await listStyles(domain)).toEqual([style]);
    expect(requestJson).toHaveBeenCalledWith(`/ops/api/creation/styles?domainRef=${domain}`, { signal: undefined });
  });
  it('refuses a style whose revision is not a positive whole number', () => {
    expect(() => parseStyle({ ...style, revision: 0 })).toThrow(ShapeError);
    expect(() => parseStyle({ ...style, revision: 1.5 })).toThrow(ShapeError);
  });
  it('sends a new style with expected revision 0 so the server assigns the code', async () => {
    vi.mocked(requestJson).mockResolvedValue({ style, replayed: false });
    const command = { requestId: account, accountRef: account, expectedRevision: 0, name: style.name, sourceNote: style.sourceNote, structure: ['先说结论'], rhythm: [], wording: [], skip: [] };
    expect((await createStyle(command)).style.code).toBe('FG-0001');
    expect(requestJson).toHaveBeenCalledWith('/ops/api/creation/styles', { method: 'POST', body: command });
  });
});
