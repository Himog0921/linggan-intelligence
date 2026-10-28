import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OcrRecognition } from './OcrRecognition';
import { parseOcrRecognition, type OcrRecognitionRun } from './ocrRecognitionTypes';

afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllGlobals(); });
const reply = (value: unknown) => new Response(JSON.stringify(value), { status: 200, headers: { 'Content-Type': 'application/json' } });
function run(change: Partial<OcrRecognitionRun> = {}): OcrRecognitionRun {
  return { runRef: 'run-a', workRef: 'work-a', state: 'running', completedImages: 1, totalImages: 3, currentStage: '正在识别内页。',
    modelLabel: 'Synthetic vision', unavailableReason: null, images: [], pendingCount: 0,
    failureCode: null, batchRef: null, batchRevision: null, cancelRequested: false, scope: null, ...change };
}
async function clickButton(name: string) { await act(async () => { fireEvent.click(screen.getByRole('button', { name })); }); }
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
  it.each([['running', 1, 33, '识别中 33%'], ['finalizing', 3, 99, '整理结果 99%'], ['succeeded', 3, 100, '识别完成 100%']] as const)(
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
    await screen.findByRole('button', { name: '识别完成 100%' });
    expect(commands).toHaveLength(2); expect(commands[0]).toEqual(commands[1]);
  });
  it.each(['network', 503])('retries the exact stop request after polling revisions and terminal state change (%s), then gives a new run its own stop command', async failure => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const scope = { sha256: 'b'.repeat(64), imageCount: 1, modelLabel: 'Synthetic vision', endpointUrl: 'http://127.0.0.1:9/v1', images: [{ slotKey: 'slot-1', ordinal: 1, role: 'page' as const, originalUrl: null }] };
    let current = run({ batchRef: 'batch-a', batchRevision: 4 });
    const stops: Record<string, unknown>[] = []; const starts: Record<string, unknown>[] = [];
    vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
      if (init?.method !== 'POST') return reply(current);
      const body = JSON.parse(String(init.body));
      if (path.endsWith('/batches/cancel')) {
        stops.push(body);
        if (stops.length === 1) {
          current = run({ batchRef: 'batch-a', batchRevision: 5, cancelRequested: true });
          if (failure === 'network') throw new TypeError('synthetic lost stop receipt');
          return new Response(JSON.stringify({ error: 'storage_unavailable' }), { status: 503, headers: { 'Content-Type': 'application/json' } });
        }
        if (body.batchRef === 'batch-b') current = run({ runRef: 'run-b', state: 'stopped', batchRef: 'batch-b', batchRevision: 2, cancelRequested: true, scope });
        return reply({ batchRef: body.batchRef, cancelRequested: true });
      }
      starts.push(body); current = run({ runRef: 'run-b', batchRef: 'batch-b', batchRevision: 1 });
      return reply({ runRef: 'run-b' });
    }));
    await act(async () => { render(<OcrRecognition workRef="work-a"/>); });
    await clickButton('停止本次识别');
    expect(screen.getByRole('button', { name: '核对本次停止请求' })).toBeEnabled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
    expect(screen.getByRole('button', { name: '正在停止' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '核对本次停止请求' })).toBeEnabled();
    current = run({ state: 'stopped', batchRef: 'batch-a', batchRevision: 6, cancelRequested: true, scope });
    await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
    expect(screen.getByRole('button', { name: '已停止 33%' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '选择图片重新识读' })).toBeDisabled();
    await clickButton('选择图片重新识读'); expect(starts).toHaveLength(0);
    await clickButton('核对本次停止请求');
    expect(stops).toHaveLength(2); expect(stops[1]).toEqual(stops[0]);
    expect(stops[1]).toMatchObject({ batchRef: 'batch-a', expectedRevision: 4 });
    expect(screen.queryByRole('button', { name: '核对本次停止请求' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '选择图片重新识读' })).toBeEnabled();
    await clickButton('选择图片重新识读'); await clickButton('确认并开始识别');
    expect(starts).toHaveLength(1);
    await clickButton('停止本次识别');
    expect(stops).toHaveLength(3); expect(stops[2]).toMatchObject({ batchRef: 'batch-b', expectedRevision: 1 });
    expect(stops[2]?.commandRef).not.toEqual(stops[0]?.commandRef);
  });
  it('clears a rejected stop and waits for a fresh revision before a new command', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    let current = run({ batchRef: 'batch-a', batchRevision: 2 });
    let waitingRead: Promise<Response> | null = null; let resolveRead!: (response: Response) => void;
    const stops: Record<string, unknown>[] = [];
    vi.stubGlobal('fetch', vi.fn(async (_path: string, init?: RequestInit) => {
      if (init?.method !== 'POST') return waitingRead ?? reply(current);
      const body = JSON.parse(String(init.body)); stops.push(body);
      if (stops.length === 1) {
        waitingRead = new Promise(resolve => { resolveRead = resolve; });
        return new Response(JSON.stringify({ error: 'batch_revision_conflict' }), { status: 409, headers: { 'Content-Type': 'application/json' } });
      }
      current = run({ state: 'stopped', batchRef: 'batch-a', batchRevision: 10, cancelRequested: true });
      return reply({ batchRef: body.batchRef, cancelRequested: true });
    }));
    await act(async () => { render(<OcrRecognition workRef="work-a"/>); });
    await clickButton('停止本次识别');
    expect(screen.getByText(/停止请求未被接纳，任务版本已变化/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '核对本次停止请求' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '停止本次识别' })).toBeDisabled();
    await clickButton('停止本次识别'); expect(stops).toHaveLength(1);
    current = run({ batchRef: 'batch-a', batchRevision: 9 }); waitingRead = null;
    await act(async () => { resolveRead(reply(current)); });
    expect(screen.getByRole('button', { name: '停止本次识别' })).toBeEnabled();
    await clickButton('停止本次识别');
    expect(stops).toHaveLength(2); expect(stops[0]).toMatchObject({ batchRef: 'batch-a', expectedRevision: 2 });
    expect(stops[1]).toMatchObject({ batchRef: 'batch-a', expectedRevision: 9 });
    expect(stops[1]?.commandRef).not.toEqual(stops[0]?.commandRef);
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
  it.each([true, false])('canonical publication is based on the actual reading pointer: %s', async isCurrent => {
    const image = { imageRef: 'image-a', ordinal: 1, role: 'page' as const, state: 'succeeded' as const, slotKey: 'slot-a', aliases: [], originalUrl: null, readingRef: 'reading-a', isCurrent, rawResultRef: null, rawResultSha256: null, rawJson: null, invocationRef: null, finishReason: null, usage: null, estimatedCostUsd: null, blocks: [{ ref: 'block-a', kind: 'paragraph' as const, text: '只可在 canonical 阅读的文字' }], issues: [] };
    vi.stubGlobal('fetch', vi.fn(async () => reply(run({ state: 'succeeded', completedImages: 1, totalImages: 1, images: [image] }))));
    const applied = vi.fn(); render(<OcrRecognition workRef="work-a" onApplied={applied} />);
    expect(await screen.findByText(isCurrent ? '识别已完成，1 张图片已进入当前底稿。' : '识别已完成，当前底稿未替换。')).toBeVisible();
    expect(screen.queryByText('只可在 canonical 阅读的文字')).not.toBeInTheDocument();
    expect(screen.queryByText(/候选|确认采用|待核验/)).not.toBeInTheDocument();
    await waitFor(() => expect(applied).toHaveBeenCalledTimes(1));
  });
  it('rejects invalid coordinates, old region payloads and impossible progress', () => {
    expect(() => parseOcrRecognition(run({ completedImages: 4 }))).toThrow();
    const image = { imageRef: 'image', ordinal: 1, role: 'page', state: 'succeeded', slotKey: null, aliases: [], originalUrl: null, readingRef: 'reading', isCurrent: true, rawResultRef: null, rawResultSha256: null, rawJson: null, invocationRef: null, finishReason: null, usage: null, estimatedCostUsd: null, blocks: [{ ref: 'r', kind: 'paragraph', text: '字', bbox: [0, 0, 999, 999] }], issues: [] };
    expect(() => parseOcrRecognition({ ...run(), images: [image] })).toThrow();
    expect(() => parseOcrRecognition({ ...run(), images: [{ ...image, blocks: [], regions: [] }] })).toThrow();
  });
  it('refreshes actual publication status after a canonical edit without dispatching again', async () => {
    let current=true;
    const fetch=vi.fn(async (_path:string,init?:RequestInit)=>{expect(init?.method).not.toBe('POST');return reply(run({state:'succeeded',completedImages:1,totalImages:1,images:[{imageRef:'image-a',ordinal:1,role:'page',state:'succeeded',slotKey:'slot-a',aliases:[],originalUrl:null,readingRef:'reading-a',isCurrent:current,rawResultRef:null,rawResultSha256:null,rawJson:null,invocationRef:null,finishReason:null,usage:null,estimatedCostUsd:null,blocks:[],issues:[]}]}));});
    vi.stubGlobal('fetch',fetch);
    const view=render(<OcrRecognition workRef="work-a" readingRevision="before-edit"/>);
    await screen.findByText('识别已完成，1 张图片已进入当前底稿。');
    current=false;view.rerender(<OcrRecognition workRef="work-a" readingRevision="after-edit"/>);
    await screen.findByText('识别已完成，当前底稿未替换。');expect(fetch).toHaveBeenCalledTimes(2);
  });
  it('sends only explicitly selected slots, preserving original inventory fingerprint', async () => {
    const ready = run({ runRef: null, state: 'ready', completedImages: 0, scope: { sha256: 'a'.repeat(64), imageCount: 3, modelLabel: 'Synthetic vision', endpointUrl: 'http://127.0.0.1:9/v1', images: [1,2,3].map(n => ({ slotKey: `slot-${n}`, ordinal: n, role: 'page' as const, originalUrl: null })) } });
    const commands: Record<string, unknown>[] = []; let accepted = false;
    vi.stubGlobal('fetch', vi.fn(async (_path: string, init?: RequestInit) => { if (init?.method === 'POST') { commands.push(JSON.parse(String(init.body))); accepted = true; return reply({ runRef: 'run-a' }); } return reply(accepted ? run({ state: 'succeeded', completedImages: 3 }) : ready); }));
    const user = userEvent.setup(); render(<OcrRecognition workRef="work-a"/>);
    await user.click(await screen.findByRole('button', { name: 'OCR 识别' })); await user.click(screen.getByRole('checkbox', { name: '第 2 页' }));
    await user.click(screen.getByRole('button', { name: '确认并开始识别' })); await screen.findByRole('button', { name: '识别完成 100%' });
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

it.each([
  ['ocr_visual_coordinates_invalid', '文字位置无效'],
  ['ocr_visual_title_mismatch', '标题与识别原文不一致'],
  ['ocr_output_truncated', '识别输出不完整'],
])('停止原因可读且不触发重发：%s', async (failureCode, message) => {
  const fetch = vi.fn(async () => reply(run({ state: 'stopped', failureCode })));
  vi.stubGlobal('fetch', fetch);
  render(<OcrRecognition workRef="work-a"/>);
  expect(await screen.findByText(`${message}，本次已停止。后续图片未发送。`)).toBeVisible();
  expect(fetch).toHaveBeenCalledTimes(1);
});
