import { render, screen, within } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { processingLabel } from './dossierPresent';
import { DossierBlocks } from './DossierBlocks';
import { DossierReading } from './DossierReading';
import { DossierPanel } from './DossierPanel';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import { createFetchMock, ok, renderWithProviders } from './testKit';

it('按后端主文和补充引用顺序读取，不按材料数组重新排序；相同文字在不同图片保留', () => {
  const a = dossierMaterialJson(1, { blocks: [{ ref: 'a-title', kind: 'heading', text: '封面整理标题' }, { ref: 'a', kind: 'paragraph', text: '第一页正文' }, { ref: 'a-repeat', kind: 'paragraph', text: '真实重复' }] });
  const b = dossierMaterialJson(2, { blocks: [{ ref: 'b', kind: 'paragraph', text: '第二页正文' }, { ref: 'b-repeat', kind: 'paragraph', text: '真实重复' }] });
  const dossier = parseDossierPayload(dossierJson(1, { materials: [b, a], note: { platformTitle: null, displayTitle: '封面整理标题', displayTitleBasis: 'cover', titleBlockRefs: ['a-title'] }, readingView: { mainBlockRefs: ['a-title', 'a', 'b', 'a-repeat', 'b-repeat'], supplementBlockRefs: [] } })).dossier;
  const source = vi.fn(); const view = render(<DossierReading dossier={dossier} titleRendered onSource={source} />);
  expect([...view.container.querySelectorAll('[data-block-ref]')].map(node => node.getAttribute('data-block-ref'))).toEqual(['a', 'b', 'a-repeat', 'b-repeat']);
  expect(screen.queryByText('封面整理标题')).not.toBeInTheDocument();
  expect(screen.getAllByText('真实重复')).toHaveLength(2);
});
it('视频口播作为主文、画面和图片作为补充，保持实际时间定位', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [
    dossierMaterialJson(1, { kind: 'video_frame', page: null, blocks: [{ ref: 'frame', kind: 'paragraph', text: '画面文字', time: { startMs: 1250, endMs: 1250 } }] }),
    dossierMaterialJson(2, { kind: 'audio', page: null, blocks: [{ ref: 'speech', kind: 'paragraph', text: '完整口播', time: { startMs: 1000, endMs: 3000 } }] }),
  ], readingView: { mainBlockRefs: ['speech'], supplementBlockRefs: ['frame'] } })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(within(screen.getByRole('region', { name: '主文' })).getByText('完整口播')).toBeVisible();
  expect(within(screen.getByRole('region', { name: '补充材料' })).getByText('画面文字')).toBeVisible();
  expect(screen.getByText('1.250 秒')).toBeVisible();
});
it('研究底稿只读打开保留局部失败，不默认加载候选或提交准备', async () => {
  const kit = createFetchMock(); kit.install();
  const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
  kit.route('GET', path, ok(dossierJson(1, { note: { platformTitle: null, displayTitle: null, displayTitleBasis: 'missing' }, materials: [dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null, issues: [{ key: 'ocr', code: 'recognition_failed', message: '此页识别失败', resolution: 'source_refresh' }] })] })));
  try {
    renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active />);
    expect(await screen.findByText('此页识别失败')).toBeVisible();
    expect(screen.getByRole('heading', { name: '缺失内容' })).toBeVisible();
    expect(screen.queryByText('对照原图 · 第 1 页')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
    expect(kit.of('GET', `${path}/candidates`)).toHaveLength(0);
    expect(kit.of('POST', `${path}/prepare`)).toHaveLength(0);
  } finally { vi.unstubAllGlobals(); }
});

it.each([
  ['pending', null, 'present', '待处理。'],
  ['running', null, 'present', '处理中。'],
  ['failed', 'asr_failed', 'present', '处理失败。'],
  ['completed', 'no_audio_stream_observed', 'not_applicable', '未发现音轨。'],
  ['completed', 'no_speech_observed', 'known_empty', '处理完成，未检出语音。'],
] as const)('口播状态 %s / %s 按真实执行结果表达', (state, reason, sourceState, expected) => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', sourceState, blocks: [], reading: null, currentRevisionRef: null, processing: { state, reason } })] })).dossier;
  expect(processingLabel(dossier.materials[0]!)).toBe(expected);
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(screen.getByText(expected)).toBeVisible();
});
it('机器处理失败仍保留已有文字，不把失败当作没有口播', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', processing: { state: 'failed', reason: 'asr_failed' }, blocks: [{ ref: 'kept', kind: 'paragraph', text: '已有口播仍完整可读' }] })], readingView: { mainBlockRefs: ['kept'], supplementBlockRefs: [] } })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(screen.getByText('处理失败，已有读本仍可读。')).toBeVisible();
  expect(screen.getByText('已有口播仍完整可读')).toBeVisible();
  expect(screen.queryByText('未发现音轨。')).not.toBeInTheDocument();
});

it('普通机器块不重复未知署名，显式引述和对话角色继续可读', () => {
  const { container } = render(<DossierBlocks blocks={[
    { ref: 'ordinary', kind: 'paragraph', text: '普通图片文字', speaker: 'unconfirmed' },
    { ref: 'quote', kind: 'quote', text: '明确引用的话', speaker: 'other' },
    { ref: 'another', kind: 'paragraph', text: '下一页图片文字', speaker: 'unconfirmed' },
    { ref: 'dialogue', kind: 'dialogue', turns: [{ speaker: 'unconfirmed', text: '对话中的未确认发言' }, { speaker: 'author', text: '作者回应' }] },
  ]} />);
  expect(screen.getAllByText('表达者未确认')).toHaveLength(1);
  expect(screen.getByText('表达者未确认').tagName).toBe('DT');
  expect(screen.getByText('他人表述')).toBeVisible();
  expect(screen.getByText('明确引用的话').tagName).toBe('BLOCKQUOTE');
  expect(container.querySelector('[data-block-ref="ordinary"]')).toHaveAttribute('data-speaker', 'unconfirmed');
  expect(container.querySelector('[data-block-ref="another"]')).toHaveAttribute('data-speaker', 'unconfirmed');
});
it('只有附件时研究底稿直接说明正文未取得，不再需要展开来源详情', async () => {
  const kit = createFetchMock(); kit.install();
  const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
  kit.route('GET', path, ok(dossierJson(1, { note: { platformTitle: null, displayTitle: null, displayTitleBasis: 'missing' }, materials: [
    dossierMaterialJson(2, { kind: 'body', sourceState: 'not_acquired', blocks: [], reading: null, currentRevisionRef: null, originalUrl: null }),
    dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null }),
  ], coverage: { sources: { body: { state: 'not_acquired', units: 1, readings: 0 }, images: { state: 'not_processed', units: 1, readings: 0 }, audio: { state: 'not_applicable', units: 0, readings: 0 }, videoFrames: { state: 'not_applicable', units: 0, readings: 0 } } } })));
  try {
    renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active />);
    expect(await screen.findByText('来源尚未取得，不能判断原作是否没有内容。')).toBeVisible();
    expect(screen.queryByText('对照原图 · 第 1 页')).not.toBeInTheDocument();
    expect(screen.queryByText('正文来源状态')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: '正文' })).not.toBeInTheDocument();
  } finally { vi.unstubAllGlobals(); }
});

it.each([
  [null, false, true],
  ['   ', false, true],
  [null, true, false],
  ['可读平台标题', false, false],
] as const)('仅识别缺口不冒充原文：标题 %s，已有原文 %s', (title, hasText, empty) => {
  const gap = { ref: 'gap-only', kind: 'gap', text: '〔此页识别失败〕' };
  const issue = { key: 'gap-diagnostic', code: 'recognition_failed', message: '来源需重新处理', resolution: 'source_refresh' };
  const dossier = parseDossierPayload(dossierJson(1, { note: { platformTitle: title, displayTitle: title, displayTitleBasis: title?.trim() ? 'platform' : 'missing' }, materials: [dossierMaterialJson(1, {
    blocks: hasText ? [gap, { ref: 'actual-text', kind: 'paragraph', text: '已经识别的真实文字' }] : [gap], issues: [issue],
  })] })).dossier;
  render(<DossierReading dossier={dossier} titleRendered onSource={vi.fn()} />);
  if (empty) expect(screen.getByText('暂无可读文字。')).toBeVisible();
  else expect(screen.queryByText('暂无可读文字。')).not.toBeInTheDocument();
  expect(screen.getByText('〔此页识别失败〕')).toBeVisible();
  expect(screen.getByText('来源需重新处理')).toBeVisible();
  expect(screen.getByRole('button', { name: '查看来源' })).toBeVisible();
  if (hasText) expect(screen.getByText('已经识别的真实文字')).toBeVisible();
});

it.each(['image', 'video_frame'] as const)('%s 的明确空识别原因与完成但无读本区分', kind => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind, sourceState: 'known_empty', blocks: [], reading: null, currentRevisionRef: null,
    processing: { state: 'completed', reason: kind === 'image' ? 'no_text_observed' : 'no_frame_text_observed' } })] })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(screen.getByText('处理完成，未检出文字。')).toBeVisible();
  expect(screen.queryByText('处理完成，尚无当前读本。')).not.toBeInTheDocument();
});
it('completed不能证明已有可用结果；明确pending issue才展示待整理', () => {
  const first = dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null, processing: { state: 'completed', reason: null } });
  const second = dossierMaterialJson(2, { blocks: [], reading: null, currentRevisionRef: null, processing: { state: 'completed', reason: null },
    issues: [{ key: 'pending-layout', code: 'existing_result_pending', message: '已有识别结果，待整理。', resolution: 'source_refresh' }] });
  const dossier = parseDossierPayload(dossierJson(1, { materials: [first, second] })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(screen.getAllByText('已有识别结果，待整理。')).toHaveLength(1);
  expect(screen.getByText('处理完成，尚无当前读本。')).toBeVisible();
  expect(screen.queryByText('处理完成，暂无可读文字。')).not.toBeInTheDocument();
});
