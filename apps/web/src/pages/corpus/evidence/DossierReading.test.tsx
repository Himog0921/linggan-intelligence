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
it('只读打开保留空文字原件与局部失败，不默认加载候选或提交准备', async () => {
  const kit = createFetchMock(); kit.install();
  const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
  kit.route('GET', path, ok(dossierJson(1, { note: { platformTitle: null, displayTitle: null, displayTitleBasis: 'missing' }, materials: [dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null, issues: [{ key: 'ocr', code: 'recognition_failed', message: '此页识别失败', resolution: 'source_refresh' }] })] })));
  try {
    renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active onOpenImage={vi.fn()} onApplied={vi.fn()} />);
    expect(await screen.findByText('暂无可读文字。')).toBeVisible();
    expect(screen.getByText('此页识别失败')).toBeVisible();
    expect(screen.getByText('对照原图 · 第 1 页')).toBeVisible();
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
it('只有附件时未取得正文收进来源详情，原件入口仍默认可达', async () => {
  const kit = createFetchMock(); kit.install();
  const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
  kit.route('GET', path, ok(dossierJson(1, { note: { platformTitle: null, displayTitle: null, displayTitleBasis: 'missing' }, materials: [
    dossierMaterialJson(2, { kind: 'body', sourceState: 'not_acquired', blocks: [], reading: null, currentRevisionRef: null, originalUrl: null }),
    dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null }),
  ] })));
  try {
    const view = renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active onOpenImage={vi.fn()} onApplied={vi.fn()} />);
    expect(await screen.findByText('暂无可读文字。')).toBeVisible();
    expect(screen.getByText('对照原图 · 第 1 页')).toBeVisible();
    const disclosure = screen.getByText('正文来源状态').closest('details');
    expect(disclosure).not.toHaveAttribute('open');
    expect(screen.getByText('来源尚未取得，不能判断原作是否没有内容。')).not.toBeVisible();
    expect(screen.queryByRole('heading', { name: '正文' })).not.toBeInTheDocument();
    await view.user.click(screen.getByText('正文来源状态'));
    expect(screen.getByText('来源尚未取得，不能判断原作是否没有内容。')).toBeVisible();
  } finally { vi.unstubAllGlobals(); }
});
