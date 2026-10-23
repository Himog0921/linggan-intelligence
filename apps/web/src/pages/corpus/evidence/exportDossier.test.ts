import { afterEach, expect, it, vi } from 'vitest';
import { dossierExport, downloadDossier, downloadDossierMarkdown } from './exportDossier';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, workRef } from './evidenceFixtures';
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it('JSON 导出是收到的完整底稿原样，不含类型投影或第二份正文', () => {
  const raw = dossierJson(1, { bodyText: '全文'.repeat(60000) });
  const file = dossierExport(parseDossierPayload(raw));
  expect(JSON.parse(file.data)).toEqual(raw.dossier); expect(file.name).toBe(`研究底稿-${workRef(1)}.json`); expect(file.type).toContain('application/json');
  expect(JSON.parse(file.data)).not.toHaveProperty('raw');
});
it('用户点击才下载 JSON，Markdown 单独获取同一截止时间', async () => {
  const blobs: Blob[] = []; vi.stubGlobal('URL', class extends URL { static override createObjectURL(blob: Blob) { blobs.push(blob); return 'blob:synthetic'; } static override revokeObjectURL() {} });
  const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
  const mock = vi.fn().mockResolvedValue(new Response('# 完整 Markdown\n\n末尾', { headers: { 'Content-Type': 'text/markdown' } })); vi.stubGlobal('fetch', mock);
  const payload = parseDossierPayload(dossierJson(1)); downloadDossier(payload); expect(mock).not.toHaveBeenCalled(); expect(click).toHaveBeenCalledTimes(1);
  await downloadDossierMarkdown(payload, DOMAIN); expect(mock).toHaveBeenCalledTimes(1); expect(click).toHaveBeenCalledTimes(2); expect(blobs[1]?.type).toBe('text/markdown;charset=utf-8');
});
