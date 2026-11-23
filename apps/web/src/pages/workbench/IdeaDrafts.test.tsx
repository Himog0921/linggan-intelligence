import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IdeaDraftsAction, IdeaDraftsBody, type IdeaDraftRow } from './IdeaDrafts';
import { openedText } from './ideaDraftsText';
import type { IdeaBoxSummary, OpenedIdeaDrafts } from './workbenchModel';

const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
const accounts = [{ ref: id(11), name: '甲号' }, { ref: id(12), name: '乙号' }, { ref: id(13), name: '丙号' }, { ref: id(14), name: '丁号' }];
const idea: IdeaBoxSummary = { ideaRef: id(1), anchorWorkId: id(2), anchorRevisionId: id(3), savedAt: '2026-11-23T00:00:00Z', name: '放手也是爱', intent: '' };
const opened = (over: Partial<OpenedIdeaDrafts>): OpenedIdeaDrafts => ({ requestId: id(5), ideaRef: id(1), drafts: [], skippedExcerpts: 0, replayed: false, ...over });
const nameOf = (ref: string) => accounts.find(account => account.ref === ref)?.name ?? '账号未知';

function body(over: Partial<Parameters<typeof IdeaDraftsBody>[0]> = {}, drafted: IdeaDraftRow[] = [], picked: string[] = []) {
  const props = { onPick: vi.fn(), onOpenWork: vi.fn(), onOpenAnchor: vi.fn() };
  render(<IdeaDraftsBody idea={idea} ideaState="success" hasIdea isAnchor accounts={accounts} drafted={drafted} picked={picked} reading="ready" opening={false} outcome={null} {...props} {...over} />);
  return props;
}

describe('what the desk says after opening drafts for several accounts', () => {
  it('names the accounts that got a draft and the ones that already had one', () => {
    expect(openedText(opened({ drafts: [
      { workId: id(21), accountRef: id(11), created: true, revisionId: id(31) },
      { workId: id(22), accountRef: id(12), created: true, revisionId: id(32) },
      { workId: id(23), accountRef: id(13), created: false, revisionId: id(33) },
    ] }), nameOf)).toBe('已开稿 2 篇：甲号、乙号；丙号原来就有稿，没有重复开');
  });
  it('says so when nothing new was opened, when sources were left out and when the request had been answered before', () => {
    expect(openedText(opened({ drafts: [{ workId: id(23), accountRef: id(13), created: false, revisionId: id(33) }] }), nameOf)).toBe('选中的账号都开过稿了，没有再开');
    expect(openedText(opened({ skippedExcerpts: 2, replayed: true, drafts: [{ workId: id(21), accountRef: id(11), created: true, revisionId: id(31) }] }), nameOf))
      .toBe('已开稿 1 篇：甲号；这些稿是上一次同一个请求已经开好的；2 段来源现在读不到，没有放进新稿');
  });
});

describe('the account picker of an idea', () => {
  it('lets the idea’s own draft pick accounts, and marks an account that already writes under it', () => {
    const { onPick } = body({}, [{ workId: id(2), accountRef: id(11), title: '放手也是爱' }]);
    const taken = screen.getByRole('button', { name: '甲号 · 已开稿' });
    expect(taken).toBeDisabled();
    expect(taken).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: '乙号' }));
    expect(onPick).toHaveBeenCalledWith(id(12));
  });
  it('disables the accounts that would be the fourth', () => {
    body({}, [{ workId: id(2), accountRef: id(11), title: '甲稿' }], [id(12), id(13)]);
    expect(screen.getByRole('button', { name: '丁号' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '丁号' })).toHaveAttribute('title', '一个选题最多 3 个账号');
    expect(screen.getByRole('button', { name: '乙号' })).toBeEnabled();
  });
  it('lists the drafts under the idea and opens one', () => {
    const { onOpenWork } = body({}, [{ workId: id(2), accountRef: id(11), title: '放手也是爱' }]);
    fireEvent.click(screen.getByRole('button', { name: /放手也是爱/ }));
    expect(onOpenWork).toHaveBeenCalledWith(id(2));
  });
  it('sends a draft that is not the idea’s own to the idea’s own draft instead of picking from here', () => {
    const { onOpenAnchor } = body({ isAnchor: false });
    expect(screen.queryByRole('button', { name: '乙号' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '切到选题框原稿' }));
    expect(onOpenAnchor).toHaveBeenCalled();
  });
  it('explains why there is nothing to pick before the idea exists or is readable', () => {
    const view = render(<IdeaDraftsBody idea={null} ideaState="success" hasIdea={false} isAnchor={false} accounts={accounts} drafted={[]} picked={[]} reading="ready" opening={false} outcome={null} onPick={vi.fn()} onOpenWork={vi.fn()} onOpenAnchor={vi.fn()} />);
    expect(screen.getByText(/这篇还没有选题框/)).toBeInTheDocument();
    view.rerender(<IdeaDraftsBody idea={null} ideaState="pending" hasIdea isAnchor={false} accounts={accounts} drafted={[]} picked={[]} reading="ready" opening={false} outcome={null} onPick={vi.fn()} onOpenWork={vi.fn()} onOpenAnchor={vi.fn()} />);
    expect(screen.getByText('正在读取选题框…')).toBeInTheDocument();
    view.rerender(<IdeaDraftsBody idea={null} ideaState="success" hasIdea isAnchor={false} accounts={accounts} drafted={[]} picked={[]} reading="ready" opening={false} outcome={null} onPick={vi.fn()} onOpenWork={vi.fn()} onOpenAnchor={vi.fn()} />);
    expect(screen.getByText(/先保存这篇/)).toBeInTheDocument();
  });
  it('does not offer accounts until every draft of the idea has been read', () => {
    const view = render(<IdeaDraftsBody idea={idea} ideaState="success" hasIdea isAnchor accounts={accounts} drafted={[]} picked={[]} reading="pending" opening={false} outcome={null} onPick={vi.fn()} onOpenWork={vi.fn()} onOpenAnchor={vi.fn()} />);
    expect(screen.getByText(/正在读取这个选题下的稿件/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '乙号' })).toBeNull();
    view.rerender(<IdeaDraftsBody idea={idea} ideaState="success" hasIdea isAnchor accounts={accounts} drafted={[]} picked={[]} reading="failed" opening={false} outcome={null} onPick={vi.fn()} onOpenWork={vi.fn()} onOpenAnchor={vi.fn()} />);
    expect(screen.getByText(/稿件读取失败/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '乙号' })).toBeNull();
  });
  it('shows the result of the last opening', () => {
    body({ outcome: { state: 'applied', text: '已开稿 1 篇：乙号' } });
    expect(screen.getByRole('status')).toHaveTextContent('已开稿 1 篇：乙号');
  });
});

describe('the open-drafts button', () => {
  it('counts the accounts once there is more than one', () => {
    const view = render(<IdeaDraftsAction count={1} cards={2} opening={false} onOpen={vi.fn()} />);
    expect(screen.getByRole('button', { name: '用这组材料开稿' })).toBeEnabled();
    view.rerender(<IdeaDraftsAction count={2} cards={2} opening={false} onOpen={vi.fn()} />);
    expect(screen.getByRole('button', { name: '用这组材料开稿 · 2 个账号' })).toBeEnabled();
  });
  it('waits for an account and for material, and shows that it is working', () => {
    const view = render(<IdeaDraftsAction count={0} cards={2} opening={false} onOpen={vi.fn()} />);
    expect(screen.getByRole('button', { name: '用这组材料开稿' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '用这组材料开稿' })).toHaveAttribute('title', '先选择写作账号');
    view.rerender(<IdeaDraftsAction count={1} cards={0} opening={false} onOpen={vi.fn()} />);
    expect(screen.getByRole('button', { name: '用这组材料开稿' })).toHaveAttribute('title', '先放入材料');
    view.rerender(<IdeaDraftsAction count={1} cards={2} opening onOpen={vi.fn()} />);
    expect(screen.getByRole('button', { name: '起稿中…' })).toBeDisabled();
  });
});
