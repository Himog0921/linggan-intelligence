import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { IdeaBoxLink } from './IdeaBoxLink';
import { newDocument, type IdeaBoxSummary, type WorkDocument } from './workbenchModel';

const idea: IdeaBoxSummary = { ideaRef: '00000000-0000-4000-8000-000000000001', anchorWorkId: '00000000-0000-4000-8000-000000000002', anchorRevisionId: '00000000-0000-4000-8000-000000000003', savedAt: '2026-10-29T00:00:00Z', name: '合成选题', intent: '合成意图' };
function Harness({ initial, open }: { initial: WorkDocument; open: (id: string) => void }) {
  const [document, edit] = useState(initial);
  const [name, setName] = useState('');
  return <><IdeaBoxLink name={name} onNameChange={setName} compose={() => {}} document={document} ideas={[idea]} state="success" edit={edit} openTopic={open} /><output aria-label="文档快照">{JSON.stringify(document)}</output></>;
}
describe('选题框关联', () => {
  it('associates a manual draft to a real existing idea and opens the anchor', () => {
    const open = vi.fn();
    render(<Harness initial={newDocument('笔记标题')} open={open} />);
    fireEvent.click(screen.getByText('补一个选题框'));
    fireEvent.change(screen.getByLabelText('关联已有选题框'), { target: { value: idea.ideaRef } });
    expect(JSON.parse(screen.getByLabelText('文档快照').textContent ?? '{}')).toMatchObject({ ideaRef: idea.ideaRef, ideaName: null, topicRef: null, title: '笔记标题' });
    fireEvent.click(screen.getByRole('button', { name: '打开选题框' }));
    expect(open).toHaveBeenCalledWith(idea.anchorWorkId);
  });
  it('creates a separate idea name without rewriting the note title', () => {
    render(<Harness initial={newDocument('笔记标题')} open={() => {}} />);
    fireEvent.click(screen.getByText('补一个选题框'));
    fireEvent.change(screen.getByLabelText('新选题框名称'), { target: { value: '新合成选题' } });
    fireEvent.click(screen.getByRole('button', { name: '建立并关联' }));
    expect(JSON.parse(screen.getByLabelText('文档快照').textContent ?? '{}')).toMatchObject({ ideaName: '新合成选题', title: '笔记标题', topicRef: null });
    fireEvent.click(screen.getByText('编辑选题意图'));
    fireEvent.change(screen.getByLabelText('选题意图'), { target: { value: '这篇希望说清楚的事情' } });
    expect(JSON.parse(screen.getByLabelText('文档快照').textContent ?? '{}').angle).toBe('这篇希望说清楚的事情');
  });
});
