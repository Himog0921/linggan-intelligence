import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ManualOcrAdoption } from './ManualOcrAdoption';
import { adoptManualOcr } from './ocrRecognitionApi';
import type { OcrRecognitionImage } from './ocrRecognitionTypes';

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), {
  status, headers: { 'Content-Type': 'application/json' },
});
function image(change: Partial<OcrRecognitionImage> = {}): OcrRecognitionImage {
  return {
    imageRef: 'image-a', ordinal: 1, role: 'cover', state: 'succeeded', slotKey: 'slot-a', aliases: [],
    originalUrl: '/api/local/media/synthetic-original', rawResultRef: 'raw-a', rawResultSha256: 'a'.repeat(64),
    rawJson: '{"synthetic":true}', invocationRef: 'invocation-a', finishReason: 'stop', usage: null,
    estimatedCostUsd: null, regions: [], adoptionRef: 'adoption-a', reviewRevision: 0, machineText: '机器预览',
    humanBody: null, ...change,
  };
}
async function completeForm(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: '人工核对并采用' }));
  await user.clear(screen.getByRole('textbox', { name: '对照原图后的完整正文' }));
  await user.type(screen.getByRole('textbox', { name: '对照原图后的完整正文' }), '人工确认正文');
  await user.type(screen.getByRole('textbox', { name: '审核人' }), 'synthetic-reviewer');
  await user.type(screen.getByRole('textbox', { name: '对照依据与原因' }), 'synthetic-original');
}

describe('manual cloud OCR adoption', () => {
  it.each([
    ['没有已保存 adoption', image({ adoptionRef: null })],
    ['原图不可读', image({ originalUrl: null })],
    ['识别结果未保存', image({ state: 'running' })],
  ])('does not show an adoption entry when %s', (_case, candidate) => {
    render(<ManualOcrAdoption workRef="work-a" image={candidate} onSaved={vi.fn()}/>);
    expect(screen.queryByRole('button', { name: '人工核对并采用' })).toBeNull();
  });

  it('sends only adopted_body, waits for its receipt, then asks the parent to reread', async () => {
    const posted: unknown[] = []; const onSaved = vi.fn();
    vi.stubGlobal('fetch', vi.fn(async (_url: string, init?: RequestInit) => {
      posted.push(JSON.parse(String(init?.body)));
      return json({ reviewRef: 'review-a', revision: 1 });
    }));
    render(<ManualOcrAdoption workRef="work-a" image={image()} onSaved={onSaved}/>);
    const user = userEvent.setup(); await completeForm(user);
    await user.click(screen.getByRole('button', { name: '保存人工采用' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('人工采用已保存，第 1 版；正在重读当前结果。'));
    expect(posted).toHaveLength(1);
    expect(posted[0]).toMatchObject({ adoptionRef: 'adoption-a', expectedRevision: 0, decision: 'adopt', field: 'adopted_body', text: '人工确认正文', actor: 'synthetic-reviewer', reason: 'synthetic-original' });
    expect(onSaved).toHaveBeenCalledOnce();
  });

  it('reuses the same command after an unknown acknowledgement instead of making a new adoption', async () => {
    const posted: unknown[] = []; let attempts = 0;
    vi.stubGlobal('fetch', vi.fn(async (_url: string, init?: RequestInit) => {
      posted.push(JSON.parse(String(init?.body))); attempts += 1;
      if (attempts === 1) throw new TypeError('synthetic lost acknowledgement');
      return json({ reviewRef: 'review-a', revision: 1 });
    }));
    render(<ManualOcrAdoption workRef="work-a" image={image()} onSaved={vi.fn()}/>);
    const user = userEvent.setup(); await completeForm(user);
    await user.click(screen.getByRole('button', { name: '保存人工采用' }));
    await screen.findByText('保存回执暂未确认；请核对同一采用请求，不会创建第二版。');
    expect(screen.getByRole('textbox', { name: '对照原图后的完整正文' })).toBeDisabled();
    expect(screen.getByRole('textbox', { name: '审核人' })).toBeDisabled();
    expect(screen.getByRole('textbox', { name: '对照依据与原因' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: '核对同一采用请求' }));
    await screen.findByText('人工采用已保存，第 1 版；正在重读当前结果。');
    expect(posted).toHaveLength(2); expect(posted[0]).toEqual(posted[1]);
  });

  it('rereads on a definite conflict, closes the stale form, and never claims success', async () => {
    const onSaved = vi.fn();
    vi.stubGlobal('fetch', vi.fn(async () => json({ code: 'manual_ocr_adoption_conflict' }, 409)));
    const view = render(<ManualOcrAdoption workRef="work-a" image={image()} onSaved={onSaved}/>);
    const user = userEvent.setup(); await completeForm(user);
    await user.click(screen.getByRole('button', { name: '保存人工采用' }));
    await screen.findByText('当前版本或来源已变化，请重新读取后再决定。');
    expect(screen.queryByText(/人工采用已保存/)).toBeNull(); expect(onSaved).toHaveBeenCalledOnce();
    expect(screen.queryByRole('button', { name: '保存人工采用' })).toBeNull();
    view.rerender(<ManualOcrAdoption workRef="work-a" image={image({ reviewRevision: 1, machineText: '刷新后的机器预览' })} onSaved={onSaved}/>);
    await user.click(screen.getByRole('button', { name: '人工核对并采用' }));
    expect(screen.getByRole('textbox', { name: '对照原图后的完整正文' })).toHaveValue('刷新后的机器预览');
    expect(screen.getByRole('textbox', { name: '审核人' })).toHaveValue('');
  });

  it('does not label a non-conflict HTTP failure as a stale source', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => json({ code: 'manual_ocr_adoption_refused' }, 400)));
    render(<ManualOcrAdoption workRef="work-a" image={image()} onSaved={vi.fn()}/>);
    const user = userEvent.setup(); await completeForm(user);
    await user.click(screen.getByRole('button', { name: '保存人工采用' }));
    await screen.findByText('保存未被接受（HTTP 400 manual_ocr_adoption_refused），请核对填写内容后重新提交。');
    expect(screen.queryByText('当前版本或来源已变化，请重新读取后再决定。')).toBeNull();
  });
});

it('does not expose the unimplemented cover_headline command field to Web callers', () => {
  type Payload = Parameters<typeof adoptManualOcr>[1];
  // @ts-expect-error The manual cloud-adoption API implements adopted_body only.
  const unsupported: Payload = { commandRef: 'command-a', adoptionRef: 'adoption-a', expectedRevision: 0, decision: 'adopt', field: 'cover_headline', text: '标题', actor: 'synthetic-reviewer', reason: 'synthetic-original' };
  expect(unsupported).toBeDefined();
});
