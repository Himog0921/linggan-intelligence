/**
 * 证据库整页测试的渲染与路由登记。数据全是合成的（形状见 evidenceFixtures）；假 fetch 对没登记的请求直接抛错，
 * 页面意外多发的请求会让测试失败。
 */
import { QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router';
import { vi } from 'vitest';
import { EvidencePage } from './EvidencePage';
import { DOMAIN, detailJson, listItemJson, listJson, shellJson, workRef, type DetailOptions, type ItemOptions } from './evidenceFixtures';
import { DRAWER_QUERY, NO_HOVER_QUERY } from './useMediaQuery';
import { newClient, ok, type FetchKit, type Responder } from './testKit';

export const LIST_PATH = '/api/local/work-resources';
export const SHELL_PATH = '/api/local/corpus/shell';
export const pageUrl = (search = `domain=${DOMAIN}`) => `/corpus/evidence${search ? `?${search}` : ''}`;
export const detailPathOf = (n: number) => `${LIST_PATH}/${workRef(n)}`;

export function renderEvidencePage(url = pageUrl()) {
  const client = newClient();
  const router = createMemoryRouter([{ path: '/corpus/evidence', element: <EvidencePage /> }, { path: '*', element: <p>其他页面</p> }], { initialEntries: [url] });
  const view = render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { ...view, client, router, user: userEvent.setup(), location: () => router.state.location.pathname + router.state.location.search };
}

/** jsdom 没有 matchMedia：按需要打开抽屉模式（≤1180px）或触屏（无悬停）。 */
export function stubViewport(options: { drawer?: boolean; noHover?: boolean } = {}) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: (query === DRAWER_QUERY && options.drawer === true) || (query === NO_HOVER_QUERY && options.noHover === true),
    media: query,
    onchange: null,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => false,
  }));
}

export function items(count: number, options: (n: number) => ItemOptions = () => ({}), start = 1) {
  return Array.from({ length: count }, (_, index) => listItemJson(start + index, options(start + index)));
}

interface Routes {
  /** 列表响应；缺省按 `items(count)` 一页给全。 */
  list?: Responder;
  count?: number;
  /** 每件作品详情的选项。 */
  detail?: (n: number) => DetailOptions;
  shell?: Responder;
}

/** 外壳、列表与前 count 件作品的详情。写接口一律不登记：页面打开时不该发任何写请求。 */
export function pageRoutes(kit: FetchKit, routes: Routes = {}) {
  const count = routes.count ?? 3;
  kit.route('GET', SHELL_PATH, routes.shell ?? ok(shellJson()));
  kit.route('GET', LIST_PATH, routes.list ?? ok(listJson(items(count))));
  for (let n = 1; n <= count; n += 1) kit.route('GET', detailPathOf(n), ok(detailJson(n, routes.detail?.(n) ?? {})));
}
