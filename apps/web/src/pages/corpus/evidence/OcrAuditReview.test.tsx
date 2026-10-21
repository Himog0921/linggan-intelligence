import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import { OcrAuditReview } from './OcrAuditReview';
afterEach(() => vi.unstubAllGlobals());
it('人工采用显式填写，确认回执后才展示版本，原始数据不由模型覆盖', async () => {
  const posted: unknown[] = [];
  vi.stubGlobal('fetch', vi.fn(async (_url: unknown, options?: RequestInit) => {
    if (options?.method === 'POST') { posted.push(JSON.parse(String(options.body))); return new Response(JSON.stringify({ reviewRef: 'synthetic-review', revision: 1, sourceKind: 'manual_transcription' }), { headers: { 'Content-Type': 'application/json' } }); }
    return new Response(JSON.stringify({ sampleRef: 'synthetic-sample', modelId: 'synthetic-model', modelOpinion: { regions: [{ text: '42' }] }, reviews: [], issues: [] }), { headers: { 'Content-Type': 'application/json' } });
  }));
  render(<MemoryRouter><QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><OcrAuditReview adoptionRef="synthetic-adoption" body="" /></QueryClientProvider></MemoryRouter>);
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '查看质量抽检' }));
  const select = await screen.findByRole('combobox', { name: '审阅决定' }); await user.selectOptions(select, 'adopt');
  await user.type(screen.getByRole('textbox', { name: '修正后本图完整采用文字' }), '42');
  await user.type(screen.getByRole('textbox', { name: '审核人' }), 'synthetic-reviewer'); await user.type(screen.getByRole('textbox', { name: '对照依据与原因' }), 'synthetic-pixels');
  await user.click(screen.getByRole('button', { name: '保存审阅决定' }));
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('审阅已保存，第 1 版'));
  expect(posted).toHaveLength(1); expect(posted[0]).toMatchObject({ decision: 'adopt', field: 'adopted_body', text: '42', actor: 'synthetic-reviewer', expectedRevision: 0 });
});
