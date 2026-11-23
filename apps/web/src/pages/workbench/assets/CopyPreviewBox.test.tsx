import { fireEvent, render as rtlRender, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement, ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import * as api from './assetsApi';
import { CopyPreviewBox } from './CopyPreviewBox';

vi.mock('./assetsApi', async importOriginal => ({ ...await importOriginal<typeof import('./assetsApi')>(), copyPreview: vi.fn() }));
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{children}</QueryClientProvider>;
const render = (ui: ReactElement) => rtlRender(ui, { wrapper });
beforeEach(() => { vi.mocked(api.copyPreview).mockReset(); });

const WORK = '00000000-0000-4000-8000-000000000010';
const REV = '00000000-0000-4000-8000-000000000014';
const preview = (patch = {}) => ({ workId: WORK, revisionId: REV, rulesetVersion: 'creation-checks.v1', title: '标题', body: '第一段\nㅤ\n第二段', tags: ['#ADHD'], text: '第一段\nㅤ\n第二段\n#ADHD', warnCount: 2, hintCount: 1, ...patch });
const must = (message: string) => ({ rule: 'R-M1', level: 'must' as const, message, location: { kind: 'slot' as const }, fixes: [] });
const blocked = (messages: string[]) => new ApiError({ kind: 'http', status: 409, code: 'creation_assets_blocked' }, { code: 'creation_assets_blocked', findings: messages.map(must) });

describe('the text the copy would give', () => {
  it('shows it read-only, with the placeholder note, and says how many reminders the copy rules gave', async () => {
    vi.mocked(api.copyPreview).mockResolvedValue(preview());
    render(<CopyPreviewBox workId={WORK} revisionId={REV} unsaved={false} />);
    const box = await screen.findByLabelText('复制出去的文字（空行里是ㅤ占位符）');
    expect(box).toHaveValue('第一段\nㅤ\n第二段\n#ADHD');
    expect(box).toHaveAttribute('readonly');
    expect(screen.getByText(/提醒 2 条、提示 1 条，都不挡复制/)).toBeVisible();
    expect(screen.getByText(/这是已保存版本要复制的文字。/)).toBeVisible();
    expect(api.copyPreview).toHaveBeenCalledWith(WORK, expect.any(AbortSignal));
  });
  it('says plainly that unsaved edits are not in it', async () => {
    vi.mocked(api.copyPreview).mockResolvedValue(preview());
    render(<CopyPreviewBox workId={WORK} revisionId={REV} unsaved />);
    expect(await screen.findByText('这是上次保存的版本，编辑中还没保存的改动不在这里。')).toBeVisible();
  });
  it('shows a progress line while it is being read', () => {
    vi.mocked(api.copyPreview).mockReturnValue(new Promise(() => undefined));
    render(<CopyPreviewBox workId={WORK} revisionId={REV} unsaved={false} />);
    expect(screen.getByRole('status')).toHaveTextContent('正在按复制的规则生成');
    expect(screen.queryByLabelText(/复制出去的文字/)).toBeNull();
  });
  it('reads again when the saved revision changes, not on every render', async () => {
    vi.mocked(api.copyPreview).mockResolvedValue(preview());
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { rerender } = rtlRender(<QueryClientProvider client={client}><CopyPreviewBox workId={WORK} revisionId={REV} unsaved={false} /></QueryClientProvider>);
    await screen.findByLabelText(/复制出去的文字/);
    rerender(<QueryClientProvider client={client}><CopyPreviewBox workId={WORK} revisionId={REV} unsaved /></QueryClientProvider>);
    expect(api.copyPreview).toHaveBeenCalledTimes(1);
    rerender(<QueryClientProvider client={client}><CopyPreviewBox workId={WORK} revisionId="00000000-0000-4000-8000-000000000099" unsaved={false} /></QueryClientProvider>);
    await screen.findByLabelText(/复制出去的文字/);
    expect(api.copyPreview).toHaveBeenCalledTimes(2);
  });
});

describe('when there is nothing to preview', () => {
  it('lists the must-fix items that block the copy instead of an empty box, at most five', async () => {
    vi.mocked(api.copyPreview).mockRejectedValue(blocked(['一', '二', '三', '四', '五', '六', '七'].map(item => `问题${item}`)));
    render(<CopyPreviewBox workId={WORK} revisionId={REV} unsaved={false} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('这个版本还有必改项，改完才能复制');
    expect(screen.getAllByText(/^问题/).map(node => node.textContent?.trim())).toEqual(['问题一', '问题二', '问题三', '问题四', '问题五']);
    expect(screen.getByText('还有 2 条，在检查里查看。')).toBeVisible();
    expect(screen.queryByLabelText(/复制出去的文字/)).toBeNull();
    expect(screen.queryByText(/这是已保存版本要复制的文字/)).toBeNull();
  });
  it('still says it is about the saved version when the writer has unsaved edits that may already fix it', async () => {
    vi.mocked(api.copyPreview).mockRejectedValue(blocked(['问题一']));
    render(<CopyPreviewBox workId={WORK} revisionId={REV} unsaved />);
    expect(await screen.findByRole('alert')).toHaveTextContent('这个版本还有必改项');
    expect(screen.getByText('这是上次保存的版本，编辑中还没保存的改动不在这里。')).toBeVisible();
  });
  it('says the saved version has no body yet when the server finds it empty', async () => {
    vi.mocked(api.copyPreview).mockRejectedValue(new ApiError({ kind: 'http', status: 400, code: 'creation_assets_parameter_invalid' }, { code: 'creation_assets_parameter_invalid' }));
    render(<CopyPreviewBox workId={WORK} revisionId={REV} unsaved={false} />);
    expect(await screen.findByText('已保存的版本还没有正文，保存后这里会显示。')).toBeVisible();
  });
  it('explains an unavailable source in plain words, and retries on request', async () => {
    vi.mocked(api.copyPreview).mockRejectedValueOnce(new ApiError({ kind: 'http', status: 409, code: 'creation_assets_source_unavailable' }, { code: 'creation_assets_source_unavailable' })).mockResolvedValueOnce(preview());
    render(<CopyPreviewBox workId={WORK} revisionId={REV} unsaved={false} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('稿件引用的来源当前不可使用');
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    expect(await screen.findByLabelText(/复制出去的文字/)).toHaveValue('第一段\nㅤ\n第二段\n#ADHD');
  });
});
