import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, it, vi } from 'vitest';
import { requestJson } from '../../shared/api';
import { createMemoryRouter, Link, RouterProvider } from 'react-router';
import { CreativeBudget } from './CreativeBudget';
vi.mock('../../shared/api', async importOriginal => ({ ...await importOriginal<typeof import('../../shared/api')>(), requestJson: vi.fn() }));
const data = { capCents: 10000, revision: 1, month: '2026-10', knownPicoUsd: '1000000000000', reservedPicoUsd: '500000000000', unknownCalls: 0 };
beforeEach(() => { vi.resetAllMocks(); });
it('shows actual and reserved costs and retries an unknown update with the same identity', async () => {
  let write = 0;
  vi.mocked(requestJson).mockImplementation(async (_url, options) => {
    if (options?.method === 'PUT') { write++; if (write === 1) throw new Error('offline'); return { capCents: 12550, revision: 2, replayed: true }; }
    return data;
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: '/', element: <><Link to='/elsewhere'>离开设置</Link><CreativeBudget/></> }, { path: '/elsewhere', element: <p>其他页面</p> }]);
  render(<QueryClientProvider client={client}><RouterProvider router={router}/></QueryClientProvider>);
  expect(await screen.findByText(/已知费用 \$1.00/)).toHaveTextContent('待结算预留 $0.50');
  fireEvent.change(screen.getByLabelText('每月上限（美元）'), { target: { value: '125.50' } });
  fireEvent.click(screen.getByRole('button', { name: '保存月度上限' }));
  expect(await screen.findByText('结果未确认，请沿用原请求重试。')).toBeInTheDocument();
  expect(screen.getByLabelText('每月上限（美元）')).toBeDisabled();
  fireEvent.click(screen.getByRole('link', { name: '离开设置' }));
  expect(await screen.findByText(/额度设置结果尚未确认/)).toBeInTheDocument();
  expect(screen.queryByText('其他页面')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '返回额度设置' }));
  fireEvent.click(screen.getByRole('button', { name: '沿用原请求重试' }));
  await waitFor(() => expect(screen.getByText('月度额度已更新。')).toBeInTheDocument());
  const calls = vi.mocked(requestJson).mock.calls.filter(call => call[1]?.method === 'PUT');
  expect(calls).toHaveLength(2);
  expect(calls[1]?.[1]?.body).toEqual(calls[0]?.[1]?.body);
  expect(calls[0]?.[1]?.body).toMatchObject({ expectedRevision: 1, capCents: 12550 });
});
it('does not present unknown historical cost as zero', async () => {
  vi.mocked(requestJson).mockResolvedValue({ ...data, unknownCalls: 2 });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: '/', element: <><Link to='/elsewhere'>离开设置</Link><CreativeBudget/></> }, { path: '/elsewhere', element: <p>其他页面</p> }]);
  render(<QueryClientProvider client={client}><RouterProvider router={router}/></QueryClientProvider>);
  expect(await screen.findByText(/2 次历史费用未知，已暂停新调用/)).toBeInTheDocument();
});
it('preserves the edit base if another page changes the cap', async () => {
  vi.mocked(requestJson).mockImplementation(async (_url, options) => options?.method === 'PUT' ? { capCents: 12550, revision: 3, replayed: false } : data);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: '/', element: <><Link to='/elsewhere'>离开设置</Link><CreativeBudget/></> }, { path: '/elsewhere', element: <p>其他页面</p> }]);
  render(<QueryClientProvider client={client}><RouterProvider router={router}/></QueryClientProvider>);
  const input = await screen.findByLabelText('每月上限（美元）');
  fireEvent.change(input, { target: { value: '125.50' } });
  act(() => client.setQueryData(['creation-ai-month-budget'], { ...data, capCents: 9000, revision: 2 }));
  expect(await screen.findByText(/当前上限已变为 \$90.00/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '保存月度上限' }));
  expect(vi.mocked(requestJson).mock.calls.filter(call => call[1]?.method === 'PUT')).toHaveLength(0);
  fireEvent.click(screen.getByRole('button', { name: '基于当前额度继续编辑' }));
  expect(input).toHaveValue('125.50');
  fireEvent.click(screen.getByRole('button', { name: '保存月度上限' }));
  await waitFor(() => expect(vi.mocked(requestJson).mock.calls.some(call => call[1]?.method === 'PUT')).toBe(true));
  expect(vi.mocked(requestJson).mock.calls.find(call => call[1]?.method === 'PUT')?.[1]?.body).toMatchObject({ expectedRevision: 2, capCents: 12550 });
});
