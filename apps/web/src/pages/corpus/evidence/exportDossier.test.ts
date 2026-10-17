import { afterEach, describe, expect, it, vi } from 'vitest';
import { downloadDossier, dossierExport } from './exportDossier';
import { parseDossierPayload } from './dossierDecode';
import { dossierJson, workRef } from './evidenceFixtures';

afterEach(() => {
  vi.useRealTimers();
});

describe('单篇导出（L14）', () => {
  const payload = parseDossierPayload(dossierJson(1));

  it('Markdown 是服务器给的标准底稿；文件名「研究底稿-<作品引用>.md」', () => {
    expect(dossierExport(payload, 'md')).toEqual({ name: `研究底稿-${workRef(1)}.md`, type: 'text/markdown;charset=utf-8', data: '# 合成底稿\n\n合成作品标题\n' });
  });

  it('JSON 是服务器给的底稿原样序列化（含平台作者编号，旧页如此，DR-9）', () => {
    const file = dossierExport(payload, 'json');
    expect(file.name).toBe(`研究底稿-${workRef(1)}.json`);
    expect(file.type).toBe('application/json;charset=utf-8');
    const parsed = JSON.parse(file.data) as { metadata: { authorExternalId: string }; workRef: string };
    expect(parsed.metadata.authorExternalId).toBe('synthetic-author-id');
    expect(parsed.workRef).toBe(workRef(1));
  });

  it('经浏览器下载：创建对象地址、点链接、稍后回收', () => {
    vi.useFakeTimers();
    const created: Blob[] = [];
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined);
    vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: (blob: Blob) => (created.push(blob), 'blob:synthetic'), revokeObjectURL: vi.fn() }));
    downloadDossier(payload, 'md');
    expect(click).toHaveBeenCalledTimes(1);
    expect(created[0]?.type).toBe('text/markdown;charset=utf-8');
    vi.advanceTimersByTime(1000);
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:synthetic');
    vi.unstubAllGlobals();
  });
});
