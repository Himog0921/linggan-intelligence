import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { newDocument, type WorkDocument } from '../workbenchModel';
import * as api from './assetsApi';
import { ExperiencePicker } from './ExperiencePicker';
import type { Experience } from './assetsModel';

vi.mock('./assetsApi', () => ({ listExperiences: vi.fn() }));
const account = '00000000-0000-4000-8000-000000000001';
const id = '00000000-0000-4000-8000-000000000002';
const item: Experience = { id, accountRef: account, code: 'JL-0001', currentPerson: true, state: 'confirmed', revision: 1,
  title: '合成经历', body: '合成正文', tags: [], origin: 'oral', sendToModel: null, confirmedAt: 't', confirmedSubjectRole: '妈妈' };
function Harness() {
  const [doc, setDoc] = useState<WorkDocument>({ ...newDocument(), accountRef: account });
  const [client] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: false } } }));
  return <QueryClientProvider client={client}><ExperiencePicker document={doc} edit={setDoc} /><output aria-label="稿件状态">{JSON.stringify(doc)}</output></QueryClientProvider>;
}
describe('经历位', () => {
  it('fills a gap only with an account experience that is confirmed and carries its revision', async () => {
    vi.mocked(api.listExperiences).mockResolvedValue([item, { ...item, id: crypto.randomUUID(), state: 'awaiting_subject', title: '未确认' }]);
    render(<Harness />);
    fireEvent.click(screen.getByRole('button', { name: '留一个经历位' }));
    fireEvent.click(screen.getByRole('button', { name: '从经历库选' }));
    const select = await screen.findByLabelText('选择本人已确认的经历');
    expect(screen.queryByRole('option', { name: /未确认/ })).not.toBeInTheDocument();
    fireEvent.change(select, { target: { value: id } });
    fireEvent.click(screen.getByRole('button', { name: '放入正文' }));
    await waitFor(() => expect(JSON.parse(screen.getByLabelText('稿件状态').textContent ?? '{}')).toMatchObject({
      experienceGaps: [], experienceSlots: [{ experienceRef: id, experienceRevision: 1 }], paragraphs: [{ text: '合成正文' }],
    }));
    fireEvent.click(screen.getByRole('button', { name: '从经历库选' }));
    expect(await screen.findByText(/已放入本稿的同一版本不重复插入/)).toBeInTheDocument();
  });
});
