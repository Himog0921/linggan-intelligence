import { fireEvent, render, screen, within } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { emptyMaterialNote, processingLabel } from './dossierPresent';
import { DossierDocument } from './DossierDocument';
import { DossierBlocks } from './DossierBlocks';
import { DossierReading } from './DossierReading';
import { DossierPanel } from './DossierPanel';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import { createFetchMock, ok, renderWithProviders } from './testKit';

it('按后端规范章节引用顺序读取，不按材料数组重新排序；相同文字在不同图片保留', () => {
  const a = dossierMaterialJson(1, { blocks: [{ ref: 'a-title', kind: 'heading', text: '封面整理标题' }, { ref: 'a', kind: 'paragraph', text: '第一页正文' }, { ref: 'a-repeat', kind: 'paragraph', text: '真实重复' }] });
  const b = dossierMaterialJson(2, { blocks: [{ ref: 'b', kind: 'paragraph', text: '第二页正文' }, { ref: 'b-repeat', kind: 'paragraph', text: '真实重复' }] });
  const dossier = parseDossierPayload(dossierJson(1, { materials: [b, a], note: { platformTitle: null, displayTitle: '封面整理标题', displayTitleBasis: 'cover', titleBlockRefs: ['a-title'] }, readingView: { mainBlockRefs: ['a-title', 'a', 'b', 'a-repeat', 'b-repeat'], supplementBlockRefs: [] } })).dossier;
  const source = vi.fn(); const view = render(<DossierReading dossier={dossier} titleRendered onSource={source} />);
  expect([...view.container.querySelectorAll('[data-block-ref]')].map(node => node.getAttribute('data-block-ref'))).toEqual(['a', 'b', 'a-repeat', 'b-repeat']);
  expect(screen.queryByText('封面整理标题')).not.toBeInTheDocument();
  expect(screen.getAllByText('真实重复')).toHaveLength(2);
});
it('视频口播与画面采用规范章节，保持实际时间定位', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [
    dossierMaterialJson(1, { kind: 'video_frame', page: null, blocks: [{ ref: 'frame', kind: 'paragraph', text: '画面文字', time: { startMs: 1250, endMs: 1250 } }] }),
    dossierMaterialJson(2, { kind: 'audio', page: null, blocks: [{ ref: 'speech', kind: 'paragraph', text: '完整口播', time: { startMs: 1000, endMs: 3000 } }] }),
  ], readingView: { mainBlockRefs: ['speech'], supplementBlockRefs: ['frame'] } })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(within(screen.getByRole('region', { name: '视频口播' })).getByText('完整口播')).toBeVisible();
  expect(within(screen.getByRole('region', { name: '画面文字' })).getAllByText('画面文字')).toHaveLength(2);
  expect(screen.getByText('00:01')).toBeVisible();
  expect(screen.getByText('00:01.250')).toBeVisible();
});
it('研究底稿只读打开保留局部失败，不默认加载候选或提交准备', async () => {
  const kit = createFetchMock(); kit.install();
  const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
  kit.route('GET', path, ok(dossierJson(1, { note: { platformTitle: null, displayTitle: null, displayTitleBasis: 'missing' }, materials: [dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null, issues: [{ key: 'ocr', code: 'recognition_failed', message: '此页识别失败', resolution: 'source_refresh' }] })] })));
  try {
    renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active />);
    expect(await screen.findByRole('heading', { name: '缺失内容' })).toBeVisible();
    expect(screen.queryByText('此页识别失败')).not.toBeInTheDocument();
    expect(screen.queryByText(/^核对说明/)).not.toBeInTheDocument();
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
  const copy = state === 'completed' ? (reason === 'no_audio_stream_observed' ? '视频口播：未发现音轨。' : '视频口播：未检出语音。') : `视频口播：${state === 'pending' ? '待处理' : state === 'running' ? '处理中' : '处理失败'}。尚无可用转录。`;
  expect(screen.getByText(copy)).toBeVisible();
});
it('机器处理失败仍保留已有文字，不把失败当作没有口播', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', processing: { state: 'failed', reason: 'asr_failed' }, blocks: [{ ref: 'kept', kind: 'paragraph', text: '已有口播仍完整可读' }] })], readingView: { mainBlockRefs: ['kept'], supplementBlockRefs: [] } })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(screen.queryByText('处理失败，已有读本仍可读。')).not.toBeInTheDocument();
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
    expect(await screen.findByText('来源尚未取得。')).toBeVisible();
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
  expect(screen.queryByText('〔此页识别失败〕')).not.toBeInTheDocument();
  expect(screen.queryByText('来源需重新处理')).not.toBeInTheDocument();
  expect(screen.getByText('来源')).toBeVisible();
  expect(dossier.materials[0]?.issues[0]?.message).toBe('来源需重新处理');
  if (hasText) expect(screen.getByText('已经识别的真实文字')).toBeVisible();
});

it.each(['image', 'video_frame'] as const)('%s 的机器处理说明保留在来源，不逐项填充正文', kind => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind, sourceState: 'known_empty', blocks: [], reading: null, currentRevisionRef: null,
    processing: { state: 'completed', reason: kind === 'image' ? 'no_text_observed' : 'no_frame_text_observed' } })] })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(screen.queryByText('处理完成，未检出文字。')).not.toBeInTheDocument();
  expect(screen.queryByText('处理完成，尚无当前读本。')).not.toBeInTheDocument();
});
it('完成但无文字保留待补状态，不展示页码或内部待整理诊断', () => {
  const first = dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null, processing: { state: 'completed', reason: null } });
  const second = dossierMaterialJson(2, { blocks: [], reading: null, currentRevisionRef: null, processing: { state: 'completed', reason: null },
    issues: [{ key: 'pending-layout', code: 'existing_result_pending', message: '已有识别结果，待整理。', resolution: 'source_refresh' }] });
  const dossier = parseDossierPayload(dossierJson(1, { materials: [first, second] })).dossier;
  render(<DossierReading dossier={dossier} titleRendered />);
  expect(screen.queryByText('已有识别结果，待整理。')).not.toBeInTheDocument();
  expect(screen.queryByText('处理完成，尚无当前读本。')).not.toBeInTheDocument();
  expect(screen.getByRole('region', { name: '待补内容' })).toBeVisible();
  expect(screen.queryByText(/第 1、2 页文字待补全/)).not.toBeInTheDocument();
  expect(screen.queryByText('处理完成，暂无可读文字。')).not.toBeInTheDocument();
});

it('人工核验的空口播与机器未检出语音分开表达', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, {
    kind: 'audio', blocks: [], sourceState: 'present',
    reading: { generatedBy: 'human_edit', adoption: 'human_edited', verification: 'verified', ruleVersion: 'human.v1', review: null },
    processing: { state: 'completed', reason: null },
  })] })).dossier;
  const unit = dossier.materials[0]!;
  expect(processingLabel(unit)).toBe('已确认无口播。');
  expect(emptyMaterialNote(unit)).toBe('已确认无口播。');
  expect(emptyMaterialNote({ ...unit, reading: null, issues: [{ key: 'empty', code: 'recognition_empty', message: '无可用转录', resolution: 'reading_edit' }] })).toBe('未取得可用转录。');
});

it('多音轨的人工空口播只描述该条材料，不否定其它可读口播', () => {
  const payload = parseDossierPayload(dossierJson(1, { materials: [
    dossierMaterialJson(1, { kind: 'audio', blocks: [], sourceState: 'present', reading: { generatedBy: 'human_edit', adoption: 'human_edited', verification: 'verified', ruleVersion: 'human.v1', review: null } }),
    dossierMaterialJson(2, { kind: 'audio', blocks: [{ ref: 'speech-present', kind: 'paragraph', text: '另一音轨的真实口播' }] }),
  ], readingView: { mainBlockRefs: ['speech-present'], supplementBlockRefs: [] } }));
  renderWithProviders(<DossierDocument payload={payload} domain={DOMAIN} active busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}} />);
  expect(screen.getByText('音轨 1：已确认无口播。')).toBeVisible();
  expect(screen.queryByText('视频口播：已确认无口播。')).not.toBeInTheDocument();
  expect(screen.getByText('另一音轨的真实口播')).toBeVisible();
});

it.each(['works', 'dossier'] as const)('%s 只显示规范有效内容，封面与换行段落一致，诊断和背景不进入读本', surface => {
  const payload = parseDossierPayload(dossierJson(1, { materials: [
    dossierMaterialJson(1, { page: null, issues: [{ key: 'order', code: 'page_order_unknown', message: '平台图片页序尚未确认', resolution: 'source_refresh' }], blocks: [
      { ref: 'cover-a', kind: 'heading', text: '智商 133 但专注力差' },
      { ref: 'cover-b', kind: 'heading', text: '除了用药家庭中做好这几点' },
      { ref: 'background', kind: 'paragraph', text: '万物' },
      { ref: 'unreliable', kind: 'gap', text: '此处文字识别不可靠，待对照原图' },
      { ref: 'step-a', kind: 'heading', text: '第一步：清理桌面，' },
      { ref: 'step-b', kind: 'heading', text: '只留一科。' },
    ] }),
    dossierMaterialJson(2, { kind: 'audio', page: null, sourceState: 'present', reading: null, currentRevisionRef: null, blocks: [], processing: { state: 'pending', reason: null } }),
    dossierMaterialJson(3, { kind: 'video_frame', page: null, sourceState: 'present', reading: null, currentRevisionRef: null, blocks: [] }),
  ], readingView: { mainBlockRefs: ['cover-a', 'cover-b', 'step-a', 'step-b'], supplementBlockRefs: [], sections: [{ ref: 'images', kind: 'images', gaps: [], groups: [
    { ref: 'cover', mode: 'heading', blockRefs: ['cover-a', 'cover-b'], joiners: ['，'], continuation: { fromPrevious: false, toNext: false } },
    { ref: 'step', mode: 'paragraph', blockRefs: ['step-a', 'step-b'], joiners: [''], continuation: { fromPrevious: false, toNext: false } },
  ] }] }, coverage: {
    imageInventory: { basis: 'observed_set', expectedPages: null, acquiredPages: 1, processedPages: 1, missingPages: [], unknownOrder: true },
    sources: { body: { state: 'not_acquired', units: 0, readings: 0 }, images: { state: 'available', units: 1, readings: 1 }, audio: { state: 'not_processed', units: 1, readings: 0 }, videoFrames: { state: 'not_processed', units: 1, readings: 0 } },
  } }));
  const view = renderWithProviders(surface === 'works' ? <DossierReading dossier={payload.dossier} titleRendered/> : <DossierDocument payload={payload} domain={DOMAIN} active busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}}/>);
  expect(screen.getByRole('heading', { name: '智商 133 但专注力差，除了用药家庭中做好这几点' })).toBeVisible();
  expect(screen.getByText('第一步：清理桌面，只留一科。')).toBeVisible();
  expect(view.container.textContent).not.toMatch(/万物|识别不可靠|总页数未知|角色未确认|画面文字/);
  expect(screen.queryByText('平台图片页序尚未确认')).not.toBeInTheDocument();
  expect(screen.getAllByText(/尚无可用转录/)).toHaveLength(1);
  expect(payload.dossier.materials[0]?.blocks).toHaveLength(6);
  expect(payload.dossier.materials[0]?.issues[0]?.code).toBe('page_order_unknown');
});


it.each([false, true])('未知页码的全 gap 不冒充完成；含有效文字 %s', hasText => {
  const blocks = [{ ref: 'diagnostic', kind: 'gap', text: '此处文字识别不可靠，待对照原图' },
    ...(hasText ? [{ ref: 'effective', kind: 'paragraph', text: '真实可用的图片文字' }] : [])];
  const payload = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { page: null, blocks, processing: { state: 'completed', reason: null } })],
    readingView: { mainBlockRefs: hasText ? ['effective'] : [], supplementBlockRefs: [] }, coverage: {
      imageInventory: { basis: 'observed_set', expectedPages: null, acquiredPages: 1, processedPages: 1, missingPages: [], unknownOrder: true },
      sources: { body: { state: 'not_applicable', units: 0, readings: 0 }, images: { state: 'available', units: 1, readings: 1 }, audio: { state: 'not_applicable', units: 0, readings: 0 }, videoFrames: { state: 'not_applicable', units: 0, readings: 0 } },
    } }));
  renderWithProviders(<DossierDocument payload={payload} domain={DOMAIN} active busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}}/>);
  expect(screen.queryByText(/此处文字识别不可靠/)).not.toBeInTheDocument();
  if (hasText) {
    expect(screen.queryByText('暂无可用文字。')).not.toBeInTheDocument();
    expect(screen.getByText('真实可用的图片文字')).toBeVisible();
  } else {
    expect(screen.getAllByText('暂无可用文字。')).toHaveLength(1);
    expect(screen.getByText('含待补内容')).toBeVisible();
    expect(screen.queryByText('内容就绪')).not.toBeInTheDocument();
  }
});

it.each(['works', 'document'])('%s 保留两张原图来源与后台冲突数据，正文不显示核对提示', surface => {
  const issue = { key: 'conflict', code: 'cross_image_parameter_conflict', message: '山河之旅时长：30min；15分钟。', resolution: 'reading_edit' };
  const payload = parseDossierPayload(dossierJson(1, { materials: [
    dossierMaterialJson(1, { issues: [issue], originalUrl: '/api/local/media/a' }),
    dossierMaterialJson(2, { issues: [issue], originalUrl: '/api/local/media/b' }),
  ] }));
  const onSource = vi.fn();
  renderWithProviders(surface === 'works' ? <DossierReading dossier={payload.dossier} onSource={onSource}/> : <DossierDocument payload={payload} domain={DOMAIN} active busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}}/>);
  expect(screen.queryByLabelText('跨图参数待核对')).not.toBeInTheDocument();
  expect(screen.queryByText('山河之旅时长：30min；15分钟。', { exact: false })).not.toBeInTheDocument();
  expect(payload.dossier.materials.map(unit => unit.issues[0]?.code)).toEqual(['cross_image_parameter_conflict', 'cross_image_parameter_conflict']);
  if (surface === 'works') {
    fireEvent.click(screen.getByText('来源'));
    const sourceButtons = screen.getAllByRole('button');
    expect(sourceButtons).toHaveLength(2);
    sourceButtons.forEach(button => fireEvent.click(button));
    expect(onSource.mock.calls.map(([unit]) => unit.originalUrl)).toEqual(['/api/local/media/a', '/api/local/media/b']);
  } else {
    expect(screen.getAllByRole('link', { name: '查看原图' }).map(link => link.getAttribute('href'))).toEqual(['/api/local/media/a', '/api/local/media/b']);
  }
});
