import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { RevisionHistory } from './RevisionHistory';
import { newDocument, parseHistory, type Revision, type RevisionSummary } from './workbenchModel';
import * as api from './workbenchApi';
vi.mock('./workbenchApi', () => ({ readRevision: vi.fn() }));
const work = '00000000-0000-4000-8000-000000000001';
const current: RevisionSummary = { id: '00000000-0000-4000-8000-000000000002', number: 92, savedAt: '2026-10-31T00:00:00Z', referenceFormat: 'current' };
const legacy: RevisionSummary = { ...current, id: '00000000-0000-4000-8000-000000000003', number: 91, referenceFormat: 'legacy_reference' };
function detail(text: string): Revision { return { ...current, document: { ...newDocument(), paragraphs: [{ id: work, text }] } }; }
beforeEach(() => vi.resetAllMocks());
describe('history metadata and qualified lazy reads', () => {
  it('accepts metadata only and rejects unknown reference formats', () => {
    expect(parseHistory({ items: [current, legacy] }).items).toHaveLength(2);
    expect(() => parseHistory({ items: [{ ...current, referenceFormat: 'guessed' }] })).toThrow();
    expect(() => parseHistory({ items: [detail('unexpected prose')] })).toThrow();
  });
  it('keeps all legacy rows without fetching or offering restore', () => {
    render(<RevisionHistory items={[legacy]} workId={work} currentRevisionId={current.id} disabled={false} onRestore={vi.fn()} />);
    expect(screen.getByText('版本 91 · 旧版引用')).toBeInTheDocument();
    expect(screen.queryByText('查看正文')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '恢复为新版本' })).not.toBeInTheDocument();
    expect(api.readRevision).not.toHaveBeenCalled();
  });
  it('loads only expanded detail, clears on close, and requalifies on reopen', async () => {
    vi.mocked(api.readRevision).mockResolvedValueOnce(detail('可读正文')).mockRejectedValueOnce(new Error('restricted'));
    render(<RevisionHistory items={[current, legacy]} workId={work} currentRevisionId={work} disabled={false} onRestore={vi.fn()} />);
    expect(api.readRevision).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText('查看正文'));
    expect(await screen.findByText('可读正文')).toBeInTheDocument();
    fireEvent.click(screen.getByText('查看正文'));
    await waitFor(() => expect(screen.queryByText('可读正文')).not.toBeInTheDocument());
    fireEvent.click(screen.getByText('查看正文'));
    expect(await screen.findByRole('alert')).toHaveTextContent('暂时无法安全读取');
    expect(screen.queryByText('可读正文')).not.toBeInTheDocument();
    expect(screen.getByText('版本 91 · 旧版引用')).toBeInTheDocument();
    expect(api.readRevision).toHaveBeenCalledTimes(2);
  });
});
