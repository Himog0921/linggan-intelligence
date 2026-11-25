import { focusManager, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { previewTemplateReplication } from '../../workbench/assets/structuralTemplatesApi';
import { EvidenceTemplateAction } from './EvidenceTemplateAction';

vi.mock('../../workbench/assets/structuralTemplatesApi', async original => ({
  ...await original<typeof import('../../workbench/assets/structuralTemplatesApi')>(),
  previewTemplateReplication: vi.fn(),
}));

it('hides cached source text while focus revalidates and after a stale response', async () => {
  let rejectRefresh: (reason: unknown) => void = () => {};
  vi.mocked(previewTemplateReplication)
    .mockResolvedValueOnce({ system: '合成指令', prompt: '曾经有效的底稿正文' })
    .mockReturnValueOnce(new Promise((_, reject) => { rejectRefresh = reject; }));
  const client = new QueryClient({ defaultOptions: { queries: { refetchOnWindowFocus: false } } });
  const view = render(<QueryClientProvider client={client}>
    <EvidenceTemplateAction workRef="work" domainRef="domain" materialRevision="revision" hasAuthorBody />
  </QueryClientProvider>);
  try {
    fireEvent.click(screen.getByRole('button', { name: '复刻成样板' }));
    fireEvent.click(screen.getByRole('button', { name: '查看自动复刻输入' }));
    expect(await screen.findByText('曾经有效的底稿正文')).toBeVisible();
    act(() => { focusManager.setFocused(false); focusManager.setFocused(true); });
    await waitFor(() => expect(screen.queryByText('曾经有效的底稿正文')).not.toBeInTheDocument());
    expect(previewTemplateReplication).toHaveBeenCalledTimes(2);
    act(() => rejectRefresh(new ApiError({ kind: 'http', status: 409, code: 'creation_assets_stale' })));
    expect(await screen.findByRole('alert')).toHaveTextContent('无法预览');
    expect(screen.queryByText('曾经有效的底稿正文')).not.toBeInTheDocument();
  } finally {
    view.unmount();
    focusManager.setFocused(undefined);
  }
});
