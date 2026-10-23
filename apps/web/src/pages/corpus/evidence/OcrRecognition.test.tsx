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
    modelLabel: 'Synthetic vision', unavailableReason: null, images: [], pendingCount: 0,
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
  it.each(['network', 503])('does not create a second command after an uncertain acknowledgement (%s)', async failure => {
    const ready = run({ runRef: null, state: 'ready', completedImages: 0, scope: { sha256: 'a'.repeat(64), imageCount: 3, modelLabel: 'Synthetic vision', endpointUrl: 'http://127.0.0.1:9/v1', images: [1,2,3].map(n => ({ slotKey: `slot-${n}`, ordinal: n, role: 'page' as const, originalUrl: null })) } });
    const commands: unknown[] = []; let accepted = false;
    const fetch = vi.fn(async (_path: string, init?: RequestInit) => {
      if (init?.method === 'POST') {
        commands.push(JSON.parse(String(init.body)));
        if (commands.length === 1) { if (failure === 'network') throw new TypeError('synthetic lost acknowledgement'); return new Response(JSON.stringify({ error: 'storage_unavailable' }), { status: 503, headers: { 'Content-Type': 'application/json' } }); }
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
    await act(async () => releaseA?.(reply(run({ currentStage: 'A 私有结果' }))));
    expect(screen.queryByText('A 私有结果')).not.toBeInTheDocument();
    expect(screen.queryByText('A 标题')).not.toBeInTheDocument();
  });
  it('shows complete candidate blocks without adopting, including local uncertainty and the untouched raw result', async () => {
    const raw = '{"regions":[{"text":"读🌱叶片边缘清晰。"}]}';
    const image = {
      imageRef: 'image-a', ordinal: 2, role: 'page' as const, state: 'succeeded' as const, slotKey: 'slot-a', aliases: [], originalUrl: '/api/local/media/image-a',
      candidateRevisionRef: 'reading-a', rawResultRef: 'raw-a', rawResultSha256: 'a'.repeat(64), rawJson: raw, invocationRef: 'invocation-a', finishReason: 'stop', usage: null, estimatedCostUsd: null,
      blocks: [{ ref: 'block-a', kind: 'paragraph' as const, text: '读🌱叶片边缘清晰。', speaker: 'unconfirmed' as const }],
      issues: [{ key: 'uncertainty', code: 'glyph', message: '图形或文字待确认', resolution: 'reading_edit' as const, blockRef: 'block-a' }],
    };
    const result = run({ state: 'succeeded', completedImages: 1, totalImages: 1, pendingCount: 1, images: [image] });
    const { rerender } = render(<OcrRecognitionResults run={result}/>);
    expect(screen.getByText('读🌱叶片边缘清晰。')).toBeVisible(); expect(screen.getByText('图形或文字待确认')).toBeVisible();
    expect(screen.getByText('候选读本')).toBeInTheDocument(); expect(screen.queryByText('当前采用，未人工核验')).not.toBeInTheDocument();
    const user = userEvent.setup(); await user.click(screen.getByText('原始识别与运行回执')); await user.click(screen.getByText('第 2 页 · 原始 JSON 与用量'));
    expect(screen.getByLabelText('第 2 页原始 JSON').textContent).toBe(raw);
    expect(screen.getByRole('link', { name: '核对原图' })).toHaveAttribute('href', '/api/local/media/image-a');
    rerender(<OcrRecognitionResults run={{ ...result, images: [{ ...image, originalUrl: null }] }} />);
    expect(screen.getByText('原图当前不可用')).toBeInTheDocument();
  });
  it('rejects invalid coordinates, old region payloads and impossible progress', () => {
    expect(() => parseOcrRecognition(run({ completedImages: 4 }))).toThrow();
    const image = { imageRef: 'image', ordinal: 1, role: 'page', state: 'succeeded', slotKey: null, aliases: [], originalUrl: null, candidateRevisionRef: 'candidate', rawResultRef: null, rawResultSha256: null, rawJson: null, invocationRef: null, finishReason: null, usage: null, estimatedCostUsd: null, blocks: [{ ref: 'r', kind: 'paragraph', text: '字', bbox: [0, 0, 999, 999] }], issues: [] };
    expect(() => parseOcrRecognition({ ...run(), images: [image] })).toThrow();
    expect(() => parseOcrRecognition({ ...run(), images: [{ ...image, blocks: [], regions: [] }] })).toThrow();
  });
  it('sends only explicitly selected slots, preserving original inventory fingerprint', async () => {
    const ready = run({ runRef: null, state: 'ready', completedImages: 0, scope: { sha256: 'a'.repeat(64), imageCount: 3, modelLabel: 'Synthetic vision', endpointUrl: 'http://127.0.0.1:9/v1', images: [1,2,3].map(n => ({ slotKey: `slot-${n}`, ordinal: n, role: 'page' as const, originalUrl: null })) } });
    const commands: Record<string, unknown>[] = []; let accepted = false;
    vi.stubGlobal('fetch', vi.fn(async (_path: string, init?: RequestInit) => { if (init?.method === 'POST') { commands.push(JSON.parse(String(init.body))); accepted = true; return reply({ runRef: 'run-a' }); } return reply(accepted ? run({ state: 'succeeded', completedImages: 3 }) : ready); }));
    const user = userEvent.setup(); render(<OcrRecognition workRef="work-a"/>);
    await user.click(await screen.findByRole('button', { name: 'OCR 识别' })); await user.click(screen.getByRole('checkbox', { name: '第 2 页' }));
    await user.click(screen.getByRole('button', { name: '确认并开始识别' })); await screen.findByRole('button', { name: '查看识别结果' });
    expect(commands[0]).toMatchObject({ expectedScopeSha256: 'a'.repeat(64), slotKeys: ['slot-1','slot-3'] });
  });
  it.each(['succeeded','stopped'] as const)('offers a separate reviewed scope for a new run after %s', async state => {
    const scope = { sha256: 'b'.repeat(64), imageCount: 2, modelLabel: 'New model', endpointUrl: 'http://127.0.0.1:9/v1', images: [1,2].map(n => ({ slotKey: `new-slot-${n}`, ordinal: n, role: 'page' as const, originalUrl: null })) };
    const commands: Record<string, unknown>[] = [];
    vi.stubGlobal('fetch', vi.fn(async (_path: string, init?: RequestInit) => { if (init?.method === 'POST') { commands.push(JSON.parse(String(init.body))); return reply({ runRef: 'run-next' }); } return reply(run({ state, scope })); }));
    const user = userEvent.setup(); render(<OcrRecognition workRef="work-a"/>);
    await user.click(await screen.findByRole('button', { name: '选择图片重新识读' }));
    expect(commands).toHaveLength(0); expect(screen.getByText(/New model/)).toBeVisible();
    await user.click(screen.getByRole('checkbox', { name: '第 1 页' })); await user.click(screen.getByRole('button', { name: '确认并开始识别' }));
    await waitFor(() => expect(commands).toHaveLength(1)); expect(commands[0]).toMatchObject({ expectedScopeSha256: 'b'.repeat(64), slotKeys: ['new-slot-2'] });
  });
});
