import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OcrRecognition } from './OcrRecognition';
import { OcrRecognitionResults } from './OcrRecognitionResults';
import { parseOcrRecognition, type OcrRecognitionRun } from './ocrRecognitionTypes';

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
const reply = (value: unknown) => new Response(JSON.stringify(value), { status: 200, headers: { 'Content-Type': 'application/json' } });
function run(change: Partial<OcrRecognitionRun> = {}): OcrRecognitionRun {
  return { runRef: 'run-a', workRef: 'work-a', state: 'running', completedImages: 1, totalImages: 3, currentStage: '正在识别内页。',
    modelLabel: 'Synthetic vision', unavailableReason: null, coverTitle: null, coverTitleStatus: null, images: [], pendingCount: 0,
    failureCode: null, batchRef: null, batchRevision: null, cancelRequested: false, scope: null, ...change };
}
describe('work-scoped OCR state and receipts', () => {
  it('shows the concrete unavailable reason and keeps dispatch disabled', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => reply(run({ runRef: null, state: 'unavailable', currentStage: '当前配置不可用。', unavailableReason: '图片文字识别工作开关尚未启用。' }))));
    render(<OcrRecognition workRef="work-a"/>);
    await screen.findByText('图片文字识别工作开关尚未启用。');
    expect(screen.getByRole('button', { name: 'OCR 识别' })).toBeDisabled();
  });
  it('does not keep saying stopping after the persisted batch has stopped', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => reply(run({ state: 'stopped', cancelRequested: true }))));
    render(<OcrRecognition workRef="work-a"/>);
    expect(await screen.findByRole('button', { name: '已停止 33%' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '正在停止' })).not.toBeInTheDocument();
  });
  it.each([['running', 1, 33, '识别中 33%'], ['finalizing', 3, 99, '整理结果 99%'], ['succeeded', 3, 100, '查看识别结果']] as const)(
    'shows persisted %s percentage, and never announces completion before readable results',
    async (state, completedImages, percent, label) => {
      vi.stubGlobal('fetch', vi.fn(async () => reply(run({ state, completedImages }))));
      render(<OcrRecognition workRef="work-a"/>);
      await waitFor(() => expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', String(percent)));
      expect(screen.getByRole('button', { name: label })).toBeInTheDocument();
      expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuetext', percent + '%');
      expect(screen.queryByText('1/3')).not.toBeInTheDocument();
    });
  it('does not create a second command after a lost acknowledgement', async () => {
    const ready = run({ runRef: null, state: 'ready', completedImages: 0, scope: { sha256: 'a'.repeat(64), imageCount: 3, modelLabel: 'Synthetic vision', endpointUrl: 'http://127.0.0.1:9/v1' } });
    const commands: unknown[] = []; let accepted = false;
    const fetch = vi.fn(async (_path: string, init?: RequestInit) => {
      if (init?.method === 'POST') {
        commands.push(JSON.parse(String(init.body)));
        if (commands.length === 1) throw new TypeError('synthetic lost acknowledgement');
        accepted = true; return reply({ runRef: 'run-a' });
      }
      return reply(accepted ? run({ state: 'succeeded', completedImages: 3 }) : ready);
    });
    vi.stubGlobal('fetch', fetch); const user = userEvent.setup();
    render(<OcrRecognition workRef="work-a"/>);
    await user.click(await screen.findByRole('button', { name: 'OCR 识别' }));
    expect(commands).toHaveLength(0);
    await user.click(screen.getByRole('button', { name: '确认并开始识别' }));
    await user.click(await screen.findByRole('button', { name: '核对本次请求' }));
    await screen.findByRole('button', { name: '查看识别结果' });
    expect(commands).toHaveLength(2); expect(commands[0]).toEqual(commands[1]);
  });
  it('cannot put a late work A response in work B', async () => {
    let releaseA: ((value: Response) => void) | undefined;
    vi.stubGlobal('fetch', vi.fn((path: string) => path.includes('work-a')
      ? new Promise<Response>(resolve => { releaseA = resolve; })
      : Promise.resolve(reply(run({ workRef: 'work-b', state: 'unavailable', unavailableReason: 'B 尚无原图', currentStage: 'B 尚无原图' })))));
    const view = render(<OcrRecognition key="work-a" workRef="work-a"/>);
    view.rerender(<OcrRecognition key="work-b" workRef="work-b"/>);
    await screen.findByText('B 尚无原图');
    await act(async () => releaseA?.(reply(run({ currentStage: 'A 私有结果', coverTitle: 'A 标题' }))));
    expect(screen.queryByText('A 私有结果')).not.toBeInTheDocument();
    expect(screen.queryByText('A 标题')).not.toBeInTheDocument();
  });
  it('keeps readable text around a Unicode uncertainty and keeps excluded graphics only in raw evidence', async () => {
    const raw = '{"regions":[{"text":"读🌱叶片K边缘清晰。"}]}';
    const result = run({ state: 'succeeded', completedImages: 1, totalImages: 1, pendingCount: 1, images: [{
      imageRef: 'image-a', ordinal: 2, role: 'page', state: 'succeeded', slotKey: 'slot-a', aliases: [], originalUrl: '/api/local/test-image',
      rawResultRef: 'raw-a', rawResultSha256: 'a'.repeat(64), rawJson: raw, invocationRef: 'invocation-a', finishReason: 'stop', usage: null, estimatedCostUsd: null,
      regions: [{ regionRef: 'region-a', text: '读🌱叶片K边缘清晰。', role: 'body', status: 'adopted', reason: null, sourceStart: 0, sourceEnd: 10, bbox: null,
        spans: [{ start: 1, end: 2, status: 'pending', reason: '图形或文字待确认' }, { start: 4, end: 5, status: 'excluded', reason: '图形干扰' }] }],
    }] });
    const { container, rerender } = render(<OcrRecognitionResults run={result}/>);
    expect(container.querySelector('p[data-status]')).toHaveTextContent('读🌱〔待核验〕叶片〔疑似干扰文字，见原始识别〕边缘清晰。');
    const user = userEvent.setup();
    await user.click(screen.getByText('原始识别与运行回执'));
    await user.click(screen.getByText('第 2 图 · 原始 JSON 与用量'));
    expect(screen.getByLabelText('第 2 图原始 JSON').textContent).toBe(raw);
    expect(screen.getAllByRole('link', { name: '核对原图' })[0]).toHaveAttribute('href', '/api/local/test-image');
    expect(screen.getByText('估算费用 USD').nextElementSibling).toHaveTextContent('未提供');
    rerender(<OcrRecognitionResults run={{ ...result, images: result.images.map(image => ({ ...image, originalUrl: null, rawJson: null, regions: [] })) }} onOpen={vi.fn()}/>);
    expect(screen.queryByRole('button', { name: '核对原图' })).not.toBeInTheDocument();
    expect(screen.queryByRole('link', { name: '核对原图' })).not.toBeInTheDocument();
    expect(screen.getAllByText('原图当前不可用').length).toBeGreaterThan(0);
  });
  it('offers manual adoption for a saved readable cover even when it has only a title or no regions', () => {
    const result = run({ state: 'succeeded', completedImages: 1, totalImages: 1, images: [{
      imageRef: 'cover-a', ordinal: 1, role: 'cover', state: 'succeeded', slotKey: 'cover-slot', aliases: [], originalUrl: '/api/local/cover',
      rawResultRef: 'raw-cover', rawResultSha256: 'b'.repeat(64), rawJson: '{}', invocationRef: 'invocation-cover', finishReason: 'stop', usage: null, estimatedCostUsd: null,
      adoptionRef: 'adoption-cover', reviewRevision: 0, machineText: '封面机器预览', humanBody: null, regions: [],
    }] });
    render(<OcrRecognitionResults run={result} onSaved={vi.fn()}/>);
    expect(screen.getByRole('button', { name: '人工核对并采用' })).toBeInTheDocument();
  });
  it('offers manual adoption for a saved readable cover with only a main title region', () => {
    const result = run({ state: 'succeeded', completedImages: 1, totalImages: 1, images: [{
      imageRef: 'cover-title-only', ordinal: 1, role: 'cover', state: 'succeeded', slotKey: 'cover-slot', aliases: [], originalUrl: '/api/local/cover-title',
      rawResultRef: 'raw-cover', rawResultSha256: 'c'.repeat(64), rawJson: '{}', invocationRef: 'invocation-cover', finishReason: 'stop', usage: null, estimatedCostUsd: null,
      adoptionRef: 'adoption-cover', reviewRevision: 0, machineText: '封面机器预览', humanBody: null,
      regions: [{ regionRef: 'title-only', text: '仅封面标题', role: 'main_title', status: 'adopted', reason: null, sourceStart: 0, sourceEnd: 5, bbox: null, spans: [] }],
    }] });
    render(<OcrRecognitionResults run={result} onSaved={vi.fn()}/>);
    expect(screen.getByRole('button', { name: '人工核对并采用' })).toBeInTheDocument();
  });
  it('rejects guessed coordinates and mismatched persisted counts at the transport boundary', () => {
    expect(() => parseOcrRecognition(run({ completedImages: 4 }))).toThrow();
    const payload = run({ images: [{
      imageRef: 'image', ordinal: 1, role: 'cover', state: 'succeeded', slotKey: null, aliases: [], originalUrl: null, rawResultRef: null, rawResultSha256: null, rawJson: null,
      invocationRef: null, finishReason: null, usage: null, estimatedCostUsd: null, regions: [{ regionRef: 'r', text: '字', role: 'body', status: 'pending', reason: null, sourceStart: 0, sourceEnd: 1, bbox: [0, 0, 999, 999], spans: [] }],
    }] });
    expect(() => parseOcrRecognition(payload)).toThrow();
  });
});
