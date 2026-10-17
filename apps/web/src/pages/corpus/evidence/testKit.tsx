/**
 * 证据库组件与页面测试用的假 fetch 和渲染辅助。所有内容都是合成的，形状见 evidenceFixtures。
 * 假 fetch 按「方法 + 路径」登记响应；多个响应依次使用（最后一个重复使用）；没登记的请求直接抛错，
 * 这样页面意外多发的请求会让测试失败，而不是被悄悄吞掉。
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { RouterProvider, createMemoryRouter } from 'react-router';
import { vi } from 'vitest';
import { shellJson } from './evidenceFixtures';

export interface Call {
  method: string;
  path: string;
  search: URLSearchParams;
  body: unknown;
}

export type Reply = { status: number; body: unknown } | 'network';
export type Responder = (call: Call) => Reply | Promise<Reply>;

export function ok(body: unknown): Responder {
  return () => ({ status: 200, body });
}

export function fail(status: number, code: string | null): Responder {
  return () => ({ status, body: code === null ? 'plain text error' : { operation: 'local_read', outcome: 'unavailable', code } });
}

export function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => {
    resolve = done;
  });
  return { promise, resolve };
}

export function createFetchMock() {
  const calls: Call[] = [];
  const handlers = new Map<string, Responder[]>();
  const fetchMock = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
    const url = new URL(String(input), 'http://127.0.0.1:3000');
    const method = init?.method ?? 'GET';
    const call: Call = { method, path: url.pathname, search: url.searchParams, body: typeof init?.body === 'string' ? (JSON.parse(init.body) as unknown) : undefined };
    calls.push(call);
    const queue = handlers.get(`${method} ${url.pathname}`);
    const responder = queue && queue.length > 1 ? queue.shift() : queue?.[0];
    if (!responder) throw new Error(`no handler for ${method} ${input}`);
    const reply = await responder(call);
    if (reply === 'network') throw new TypeError('Failed to fetch');
    const text = typeof reply.body === 'string' ? reply.body : JSON.stringify(reply.body);
    return new Response(text, { status: reply.status, headers: { 'Content-Type': typeof reply.body === 'string' ? 'text/plain' : 'application/json' } });
  });
  return {
    calls,
    fetchMock,
    route(method: string, path: string, ...responders: Responder[]) {
      handlers.set(`${method} ${path}`, responders);
    },
    of(method: string, path: string): Call[] {
      return calls.filter(call => call.method === method && call.path === path);
    },
    install() {
      vi.stubGlobal('fetch', fetchMock);
    },
  };
}

export type FetchKit = ReturnType<typeof createFetchMock>;

export function newClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false }, mutations: { retry: false } } });
}

/** 渲染任意元素，带查询客户端和路由（组件需要 Link 或 QueryClient 时用）。 */
export function renderWithProviders(element: ReactElement, url = '/') {
  const client = newClient();
  const router = createMemoryRouter([{ path: '*', element }], { initialEntries: [url] });
  const view = render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { ...view, client, user: userEvent.setup(), location: () => router.state.location.pathname + router.state.location.search };
}

/** 默认的只读路由：外壳。列表、详情等由各测试自己登记。 */
export function defaultRoutes(kit: FetchKit) {
  kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
}
