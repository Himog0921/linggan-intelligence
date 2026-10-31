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
  const [draftInputs, setDraftInputs] = useState({ cover: "", note: "", idea: "" });
  const [client] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: false } } }));
  return <QueryClientProvider client={client}>
    <ManuscriptEditor draftInputs={draftInputs} onDraftInputs={setDraftInputs} document={document} edit={setDocument} compose={() => {}} isComposing={() => false}
      textSelection={() => {}} tagsDraft="" onTagsChange={() => {}} />
    <output aria-label="AI来源记录">{JSON.stringify(document.derivedSources)}</output>
    <output aria-label="来源记录">{JSON.stringify(document.experienceSlots)}</output>
    <output aria-label="标题候选记录">{JSON.stringify(document.noteTitleCandidates)}</output>
    <output aria-label="封面记录">{JSON.stringify(document.pages.filter(page => page.kind === 'cover'))}</output>
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

describe('标题候选', () => {
  it('writes the cover headline directly and creates a cover only when the user types', () => {
    render(<Editor initial={newDocument('笔记标题')} />);
    expect(JSON.parse(screen.getByLabelText('封面记录').textContent ?? '[]')).toEqual([]);
    fireEvent.change(screen.getByLabelText('封面大字'), { target: { value: '合成封面' } });
    expect(JSON.parse(screen.getByLabelText('封面记录').textContent ?? '[]')[0]).toMatchObject({ kind: 'cover', title: '合成封面', assetId: null });
    expect(screen.getByLabelText('正文标题')).toHaveValue('笔记标题');
  });
  it('saves manually entered alternatives with the document and applies the selected title', () => {
    render(<Editor initial={newDocument('原笔记标题')} />);
    fireEvent.click(screen.getByText('添加笔记标题候选'));
    fireEvent.change(screen.getByLabelText('新标题候选'), { target: { value: '合成候选标题' } });
    const area = screen.getByLabelText('笔记标题候选');
    fireEvent.click(within(area).getByRole('button', { name: '添加候选' }));
    expect(JSON.parse(screen.getByLabelText('标题候选记录').textContent ?? '[]')).toEqual(['合成候选标题']);
    fireEvent.click(within(area).getByRole('button', { name: '合成候选标题' }));
    expect(screen.getByLabelText('正文标题')).toHaveValue('合成候选标题');
    fireEvent.click(within(area).getByRole('button', { name: '删除标题候选 合成候选标题' }));
    expect(JSON.parse(screen.getByLabelText('标题候选记录').textContent ?? '[]')).toEqual([]);
  });
});


it('preserves AI source lineage when splitting a paragraph', () => {
  const initial = { ...newDocument(), paragraphs: [{ id: first, text: '合成改写内容' }], derivedSources: [{
    target: { kind: 'paragraph' as const, paragraphId: first },
    citation: { asOf: '2026-10-29', blockRef: 'body', sourceKind: 'comment' as const, sourceRef: experience, workRef: first, field: 'comment_body' as const, sourceVersion: 'v1', start: 0, end: 1, textHash: 'a'.repeat(64), location: null },
  }] };
  render(<Editor initial={initial}/>);
  const input = screen.getByLabelText('正文第 1 段') as HTMLTextAreaElement;
  input.setSelectionRange(2, 2); fireEvent.keyDown(input, { key: 'Enter' });
  expect(screen.getByLabelText('正文第 2 段')).toHaveValue('改写内容');
  const sources = JSON.parse(screen.getByLabelText('AI来源记录').textContent ?? '[]');
  expect(sources).toHaveLength(2);
  expect(sources[1].citation).toEqual(sources[0].citation);
  expect(sources[1].target.paragraphId).not.toBe(first);
});

it('retains sources on both halves when Enter replaces a selected range', () => {
  const initial = { ...newDocument(), paragraphs: [{ id: first, text: '合成改写内容' }], derivedSources: [{
    target: { kind: 'paragraph' as const, paragraphId: first },
    citation: { asOf: '2026-10-29', blockRef: 'body', sourceKind: 'comment' as const, sourceRef: experience, workRef: first, field: 'comment_body' as const, sourceVersion: 'v1', start: 0, end: 1, textHash: 'a'.repeat(64), location: null },
  }] };
  render(<Editor initial={initial}/>);
  const input = screen.getByLabelText('正文第 1 段') as HTMLTextAreaElement;
  input.setSelectionRange(2, 4); fireEvent.keyDown(input, { key: 'Enter' });
  expect(screen.getByLabelText('正文第 1 段')).toHaveValue('合成');
  expect(screen.getByLabelText('正文第 2 段')).toHaveValue('内容');
  const sources = JSON.parse(screen.getByLabelText('AI来源记录').textContent ?? '[]');
  expect(sources).toHaveLength(2);
  expect(sources[1].citation).toEqual(sources[0].citation);
  expect(sources[1].target.paragraphId).not.toBe(first);
});
