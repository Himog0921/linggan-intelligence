import { afterEach, expect, it, vi } from 'vitest';
import { dossierExport, dossierMarkdownExport, downloadFile } from './exportDossier';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, workRef } from './evidenceFixtures';
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it('JSON 导出是收到的完整底稿原样，不含类型投影或第二份正文', () => {
  const raw = dossierJson(1, { bodyText: '全文'.repeat(60000) });
  const file = dossierExport(parseDossierPayload(raw));
  expect(JSON.parse(file.data)).toEqual(raw.dossier); expect(file.name).toBe(`研究底稿-${workRef(1)}.json`); expect(file.type).toContain('application/json');
  expect(JSON.parse(file.data)).not.toHaveProperty('raw');
});
it('现用 downloadFile 使用文件原字符串与 MIME，点击后回收本地对象 URL', () => {
  vi.useFakeTimers();
  const NativeBlob = Blob, parts: BlobPart[][] = [], blobs: Blob[] = [], names: string[] = [];
  const revoke = vi.fn();
  vi.stubGlobal('Blob', class extends NativeBlob { constructor(data: BlobPart[], options?: BlobPropertyBag) { super(data, options); parts.push(data); } });
  vi.stubGlobal('URL', class extends URL { static override createObjectURL(blob: Blob) { blobs.push(blob); return 'blob:synthetic'; } static override revokeObjectURL(url: string) { revoke(url); } });
  vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function(this: HTMLAnchorElement) { names.push(this.download); });
  const file = { name: '合成底稿.md', type: 'text/markdown;charset=utf-8', data: '# 完整 Markdown\n\n最后一段\n' };
  downloadFile(file);
  expect(parts).toEqual([[file.data]]); expect(blobs[0]?.type).toBe(file.type); expect(names).toEqual([file.name]); expect(revoke).not.toHaveBeenCalled();
  vi.advanceTimersByTime(1000); expect(revoke).toHaveBeenCalledWith('blob:synthetic');
});
it('Markdown 文件单独读取冻结 asOf/revision，尚未点击下载时只返回同一文本', async () => {
  const text = '# 完整 Markdown\n\n末尾';
  const mock = vi.fn().mockResolvedValue(new Response(text, { headers: { 'Content-Type': 'text/markdown' } })); vi.stubGlobal('fetch', mock);
  const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
  const payload = parseDossierPayload(dossierJson(1));
  const file = await dossierMarkdownExport(payload, DOMAIN);
  expect(file.data).toBe(text); expect(file.type).toBe('text/markdown;charset=utf-8'); expect(click).not.toHaveBeenCalled();
  const url = new URL(String(mock.mock.calls[0]?.[0]), 'http://localhost');
  expect(url.searchParams.get('asOf')).toBe(payload.dossier.asOf); expect(url.searchParams.get('expectedRevision')).toBe(payload.dossier.revision);
});
