import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { newDocument, type WorkDocument } from '../workbenchModel';
import { newImagePage } from '../imagePageFactory';
import { PreviewTab } from './PreviewTab';

vi.mock('./CopyPreviewBox', () => ({ CopyPreviewBox: ({ revisionId, unsaved }: { revisionId: string; unsaved: boolean }) => <div data-testid="copy-box">{revisionId}:{String(unsaved)}</div> }));
vi.mock('./CoverCanvas', () => ({ CoverCanvas: ({ page, workId }: { page: { title: string }; workId: string }) => <div data-testid="cover-canvas">{page.title}@{workId}</div> }));

const doc = (patch: Partial<WorkDocument> = {}): WorkDocument => ({
  ...newDocument('孩子迟迟不开始写作业'), paragraphs: [{ id: 'a', text: '第一段' }, { id: 'b', text: '收尾' }], ...patch,
});

describe('phone preview', () => {
  it('lays out the title, the paragraphs with a blank line between, and the tags once each', () => {
    const { container } = render(<PreviewTab document={doc()} workId="w1" tagsDraft="#A娃 #陪跑 #a娃" />);
    expect(screen.getByText('孩子迟迟不开始写作业')).toHaveClass('phone-title');
    expect(container.querySelector('.phone-body')).toHaveTextContent('第一段ㅤ收尾');
    expect(container.querySelectorAll('.phone-body p[aria-hidden="true"]')).toHaveLength(1);
    expect(container.querySelector('.phone-tags')).toHaveTextContent('#A娃 #陪跑');
  });
  it('shows the bridge sentence where the slot puts it', () => {
    const { container } = render(<PreviewTab workId="w1" tagsDraft="" document={doc({ bridge: { skip: false, afterParagraph: null, sentence: { sentenceCode: 'XJ-1', sentenceRevision: 1, group: 'parent', text: '我也是学员家长', edited: false } } })} />);
    expect([...container.querySelectorAll('.phone-body > div > p:not([aria-hidden])')].map(node => node.textContent)).toEqual(['第一段', '我也是学员家长', '收尾']);
    expect(container.querySelector('.phone-bridge')).toHaveTextContent('我也是学员家长');
  });
  it('draws the cover with the export renderer, and tags it only when it is known whether AI made it', () => {
    const cover = { ...newImagePage('cover'), title: '先做一步' };
    const { container, rerender } = render(<PreviewTab document={doc({ pages: [cover] })} workId="w1" tagsDraft="" enabled />);
    expect(screen.getByTestId('cover-canvas')).toHaveTextContent('先做一步@w1');
    expect(container.querySelector('.cover-tag')).toBeNull();
    rerender(<PreviewTab document={doc({ pages: [cover], coverAi: true })} workId="w1" tagsDraft="" enabled />);
    expect(container.querySelector('.cover-tag')).toHaveTextContent('AI 作图');
    rerender(<PreviewTab document={doc({ pages: [cover], coverAi: false })} workId="w1" tagsDraft="" enabled />);
    expect(container.querySelector('.cover-tag')).toHaveTextContent('实拍');
  });
  it('does not run the export renderer while the preview tab is hidden', () => {
    const cover = { ...newImagePage('cover'), title: '先做一步' };
    const { rerender } = render(<PreviewTab document={doc({ pages: [cover] })} workId="w1" tagsDraft="" />);
    expect(screen.queryByTestId('cover-canvas')).toBeNull();
    rerender(<PreviewTab document={doc({ pages: [cover] })} workId="w1" tagsDraft="" enabled />);
    expect(screen.getByTestId('cover-canvas')).toBeInTheDocument();
  });
  it('shows an empty cover frame when there is no cover page yet, and says so when there is no body', () => {
    render(<PreviewTab document={doc({ paragraphs: [], title: '' })} workId="w1" tagsDraft="" />);
    expect(screen.queryByTestId('cover-canvas')).toBeNull();
    expect(screen.getByText('未命名稿件')).toBeVisible();
    expect(screen.getByText('还没有正文。')).toBeVisible();
  });
  it('adds the copy text box only while the tab is showing, for a saved revision, and when there is a body', () => {
    const { rerender } = render(<PreviewTab document={doc()} workId="w1" tagsDraft="" savedRevisionId="r1" />);
    expect(screen.queryByTestId('copy-box')).toBeNull();
    rerender(<PreviewTab document={doc()} workId="w1" tagsDraft="" enabled />);
    expect(screen.queryByTestId('copy-box')).toBeNull();
    rerender(<PreviewTab document={doc({ paragraphs: [] })} workId="w1" tagsDraft="" savedRevisionId="r1" enabled />);
    expect(screen.queryByTestId('copy-box')).toBeNull();
    rerender(<PreviewTab document={doc()} workId="w1" tagsDraft="" savedRevisionId="r1" unsaved enabled />);
    expect(screen.getByTestId('copy-box')).toHaveTextContent('r1:true');
  });
  it('says the preview is of the draft being edited, not of the copied text', () => {
    render(<PreviewTab document={doc()} workId="w1" tagsDraft="" />);
    expect(screen.getByText(/复制时会使用保存并检查后的稿件/)).toBeVisible();
  });
});
