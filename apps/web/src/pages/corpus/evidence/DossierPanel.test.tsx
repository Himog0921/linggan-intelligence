import { screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DossierPanel } from './DossierPanel';
import { DOMAIN, dossierJson, workRef } from './evidenceFixtures';
import { createFetchMock, fail, ok, renderWithProviders, type FetchKit } from './testKit';
const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
let kit: FetchKit;
beforeEach(() => { kit = createFetchMock(); kit.install(); });
afterEach(() => vi.unstubAllGlobals());
function mount() { return renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active onOpenImage={vi.fn()} onApplied={vi.fn()} />); }

it.each([401,403,404,410])('刷新得到 %i 后撤下旧内容与导出入口', async status => {
  kit.route('GET', path, ok(dossierJson(1, { bodyText: '已有缓存但随后失去权限的正文' })), fail(status, 'source_restricted'));
  const view = mount(); await screen.findByText('已有缓存但随后失去权限的正文');
  await view.user.click(screen.getByRole('button', { name: '刷新底稿' })); await screen.findByText('研究底稿暂时不可读。');
  expect(screen.queryByText('已有缓存但随后失去权限的正文')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '导出 JSON' })).not.toBeInTheDocument(); expect(screen.queryByRole('button', { name: '导出 Markdown' })).not.toBeInTheDocument();
});
it('临时服务故障保留并标记最后读到的版本，不把故障表示为无内容', async () => {
  kit.route('GET', path, ok(dossierJson(1, { bodyText: '最后成功读取的完整正文' })), fail(503, 'temporarily_unavailable'));
  const view = mount(); await screen.findByText('最后成功读取的完整正文');
  await view.user.click(screen.getByRole('button', { name: '刷新底稿' })); await screen.findByText(/刷新未完成，当前保留上一次读到的版本/);
  expect(screen.getByText('最后成功读取的完整正文')).toBeVisible(); expect(screen.getByRole('button', { name: '导出 JSON' })).toBeEnabled();
});
