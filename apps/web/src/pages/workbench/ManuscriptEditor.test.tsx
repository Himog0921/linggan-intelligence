import { fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import { ManuscriptEditor } from './ManuscriptEditor';
import { newDocument, type WorkDocument } from './workbenchModel';

const first = '00000000-0000-4000-8000-000000000001';
const experience = '00000000-0000-4000-8000-000000000002';

function Editor({ initial }: { initial: WorkDocument }) {
  const [document, setDocument] = useState(initial);
  const [client] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: false } } }));
  return <QueryClientProvider client={client}>
    <ManuscriptEditor document={document} edit={setDocument} compose={() => {}} isComposing={() => false}
      textSelection={() => {}} tagsDraft="" onTagsChange={() => {}} />
    <output aria-label="来源记录">{JSON.stringify(document.experienceSlots)}</output>
  </QueryClientProvider>;
}

describe('经历来源', () => {
  it('starts a separate authored paragraph without attaching it to an experience', () => {
    const initial = { ...newDocument(), paragraphs: [{ id: first, text: '合成经历正文' }],
      experienceSlots: [{ paragraphId: first, experienceRef: experience, experienceRevision: 1 }] };
    render(<Editor initial={initial} />);
    fireEvent.click(screen.getByRole('button', { name: /继续写一段/ }));
    fireEvent.change(screen.getByLabelText('正文第 2 段'), { target: { value: '独立原创' } });
    expect(JSON.parse(screen.getByLabelText('来源记录').textContent ?? '[]')).toEqual(initial.experienceSlots);
    expect(screen.getAllByText('来源：经历库 · 使用前重新核验')).toHaveLength(1);
  });

  it('keeps line breaks in the same sourced paragraph and removes its reference when deleted', () => {
    const initial = { ...newDocument(), paragraphs: [{ id: first, text: '合成经历正文' }],
      experienceSlots: [{ paragraphId: first, experienceRef: experience, experienceRevision: 1 }] };
    render(<Editor initial={initial} />);
    const input = screen.getByLabelText('正文第 1 段') as HTMLTextAreaElement;
    input.setSelectionRange(4, 4);
    fireEvent.keyDown(input, { key: 'Enter' });
    const output = screen.getByLabelText('来源记录');
    const references = JSON.parse(output.textContent ?? '[]');
    expect(references).toHaveLength(1);
    expect(references[0]).toMatchObject({ paragraphId: first, experienceRef: experience, experienceRevision: 1 });
    expect(screen.getAllByText('来源：经历库 · 使用前重新核验')).toHaveLength(1);
    const firstParagraph = screen.getByLabelText('正文第 1 段').parentElement!;
    fireEvent.click(within(firstParagraph).getByText('段落与来源'));
    fireEvent.click(within(firstParagraph).getByRole('button', { name: '删除这一段' }));
    expect(JSON.parse(output.textContent ?? '[]')).toEqual([]);
  });
});
