import { expect, it } from 'vitest';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';
import { parseDossierPayload } from './dossierDecode';
import { dossierTabStatus } from './dossierTabStatus';
const normal = () => parseDossierPayload(dossierJson(1, {
  materials: [dossierMaterialJson(1, { kind: 'body', blocks: [{ ref: 'body', kind: 'paragraph', text: '已有正文' }] })],
  readingView: { mainBlockRefs: ['body'], supplementBlockRefs: [] },
  coverage: { imageInventory: { basis: 'platform_image_list', expectedPages: 0, acquiredPages: 0, processedPages: 0, missingPages: [], unknownOrder: false }, sources: { body: { state: 'available', units: 1, readings: 1 }, images: { state: 'not_applicable', units: 0, readings: 0 }, audio: { state: 'not_applicable', units: 0, readings: 0 }, videoFrames: { state: 'not_applicable', units: 0, readings: 0 } } },
})).dossier;
it('normal reading has no quality or ready badge; unknown does not become normal', () => {
  expect(dossierTabStatus(normal(), null, false)).toBeNull();
  expect(dossierTabStatus(undefined, null, false)?.label).toBe('未读取');
  expect(dossierTabStatus(undefined, null, true)?.label).toBe('读取中');
});
it('keeps missing and review facts together, including during failed refresh of retained reading', () => {
  const d = normal();
  d.coverage.sources.images = { state: 'partial', units: 2, readings: 1 };
  d.coverage.issues.push({ key: 'review', code: 'source', message: '出处待核对', resolution: 'source_refresh' });
  expect(dossierTabStatus(d, null, false)?.label).toBe('待补·核对');
  const failed = dossierTabStatus(d, new Error('refresh failed'), false);
  expect(failed?.label).toBe('刷新失败');
  expect(failed?.description).toContain('有待补内容；有待核对事项');
  expect(failed?.description).toContain('保留上次可读版本');
  expect(dossierTabStatus(undefined, new Error('read failed'), false)?.label).toBe('不可读');
});
