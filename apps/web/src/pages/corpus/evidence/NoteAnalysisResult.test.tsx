import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { DossierReading } from './DossierReading';
import { NoteAnalysisResult } from './NoteAnalysisResult';
import { parseDossierPayload } from './dossierDecode';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';
import type { AnalysisRead } from './noteAnalysisTypes';

afterEach(cleanup);
const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, {
  kind: 'body', blocks: [{ ref: 'canonical-body', kind: 'paragraph', text: '这是当前原文。' }],
})] })).dossier;
const material = dossier.materials[0]!;
function saved(citationId = 'canonical-body', sourceVersion = material.sourceVersion!): AnalysisRead {
  const statement = { id: 'discussion', text: '这是内容理解。', kind: 'observation' as const, evidenceRefs: ['e1'] };
  return {
    contract: 'note-analysis.read.v1', requestRef: 'request', workRef: dossier.workRef, scope: 'note',
    run: { state: 'succeeded', batchRef: 'batch', itemRef: 'item', invocationRef: 'invocation', failureCode: null },
    freshness: 'current', coverage: { materialColumns: ['body'], includedCommentCount: 0, authorCommentCount: 0, platformCommentCount: null, issues: [] },
    versions: { methodVersion: 'v0.3', taskConfigRef: 'config', skillVersionRef: null, promptVersionRef: null, modelConfigRef: null, createdAt: '2026-11-04T00:00:00Z' },
    result: { contract: 'note-analysis.result.v1', summary: '这是摘要。', summaryBasisIds: ['discussion'],
      understanding: { discussion: statement, answer: { ...statement, id: 'answer' }, audience: null, situation: null, offeredValue: null },
      features: [], feedback: [], explanations: [], tags: [], methodCandidates: [], limitations: [],
      evidence: [{ id: 'e1', kind: 'material', sourceRef: material.sourceRef!, sourceVersion, citationId, start: 0, end: 7, quote: '这是当前原文。' }],
    },
  };
}

it('来源按钮定位并聚焦实际阅读组件的规范块，而不是旧材料属性', () => {
  const view = render(<><DossierReading dossier={dossier} /><NoteAnalysisResult read={saved()} dossier={dossier} /></>);
  const block = view.container.querySelector<HTMLElement>('[data-block-ref="canonical-body"]')!;
  const scroll = vi.fn(); block.scrollIntoView = scroll;
  fireEvent.click(screen.getAllByText('依据', { exact: true })[0]!);
  fireEvent.click(screen.getAllByRole('button', { name: '回到原文' })[0]!);
  expect(scroll).toHaveBeenCalledWith({ block: 'center' });
  expect(document.activeElement).toBe(block);
});

it.each([
  ['title-without-a-reading-block', material.sourceVersion!],
  ['canonical-body', 'older-source-version'],
])('没有对应同版阅读块时保留引句，不给出错误跳转 (%s)', (citationId, version) => {
  render(<><DossierReading dossier={dossier} /><NoteAnalysisResult read={saved(citationId, version)} dossier={dossier} /></>);
  fireEvent.click(screen.getAllByText('依据', { exact: true })[0]!);
  expect(screen.queryByRole('button', { name: '回到原文' })).not.toBeInTheDocument();
  expect(screen.getAllByText('这是当前原文。').length).toBeGreaterThan(1);
});
