import { afterEach, describe, expect, it, vi } from 'vitest';
import { COLUMN_LABELS, fetchNoteMaterial, materialReason, parseNoteMaterial } from './noteMaterial';
import { DOMAIN, noteMaterialJson, workRef } from './evidenceFixtures';

afterEach(() => vi.unstubAllGlobals());
const parse = (value: unknown) => parseNoteMaterial(value, workRef(1), DOMAIN);

describe('每篇材料读取合同', () => {
  it('保留后端ready与转写原因；Unicode字符范围包含emoji，五列不重算资格', () => {
    const value = parse(noteMaterialJson());
    expect(value.ready).toBe(false);
    expect(value.notReadyReasons).toEqual(['transcript_processing_pending']);
    expect(value.fragments[0]).toMatchObject({ start: 4, end: 8, text: '合成🙂文' });
    const prepared = noteMaterialJson(); prepared.ready = true;
    expect(parse(prepared).ready).toBe(true);
  });
  it('闭集接受11种栏目状态，保留未知/空/终止/受限的区别', () => {
    for (const state of Object.keys(COLUMN_LABELS)) {
      const value = noteMaterialJson(); value.columns[0]!.state = state;
      expect(parse(value).columns[0]!.state).toBe(state);
    }
    expect(materialReason('2:source:media_work_state_unavailable')).toBe('当前媒体作业证据不足（位置 2:source）');
  });
  it('新状态、原因、字段、处理范围和合同均使整份响应不可显示', () => {
    const changes = [
      (v: ReturnType<typeof noteMaterialJson>) => { v.columns[0]!.state = 'new_state'; },
      (v: ReturnType<typeof noteMaterialJson>) => { v.columns[0]!.reasons = ['new_reason']; },
      (v: ReturnType<typeof noteMaterialJson>) => { v.columns[0]!.field = 'new_field'; },
      (v: ReturnType<typeof noteMaterialJson>) => { v.processingScope.included.push('new_scope'); },
      (v: ReturnType<typeof noteMaterialJson>) => { v.processingScope.included.push(''); },
      (v: ReturnType<typeof noteMaterialJson>) => { v.contract = 'note-material-input.v2'; },
      (v: ReturnType<typeof noteMaterialJson>) => { v.workRef = workRef(2); },
      (v: ReturnType<typeof noteMaterialJson>) => { v.domainRef = 'other-domain'; },
      (v: ReturnType<typeof noteMaterialJson>) => { v.fragments[0]!.end = 9; },
      (v: ReturnType<typeof noteMaterialJson>) => { v.columns.pop(); },
      (v: ReturnType<typeof noteMaterialJson>) => { v.fragments.push(v.fragments[0]!); },
    ];
    for (const change of changes) { const value = noteMaterialJson(); change(value); expect(() => parse(value)).toThrow(); }
  });
  it('只发同源GET和当前domain，传递取消信号；不保存快照或启动任务', async () => {
    const mock = vi.fn(async () => new Response(JSON.stringify(noteMaterialJson()), { headers: { 'Content-Type': 'application/json' } }));
    vi.stubGlobal('fetch', mock);
    const controller = new AbortController();
    await fetchNoteMaterial(workRef(1), DOMAIN, controller.signal);
    expect(mock).toHaveBeenCalledOnce();
    expect(mock).toHaveBeenCalledWith(`/api/local/work-resources/${workRef(1)}/research-dossier/note-material?domain=${DOMAIN}`, expect.objectContaining({ method: 'GET', signal: controller.signal, cache: 'no-store', body: null }));
  });
});
