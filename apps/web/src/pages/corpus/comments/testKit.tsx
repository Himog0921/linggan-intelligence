import { businessSettingsFixture } from '../../settings/aiSettingsFixture.test-support';
import { StrictMode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { vi } from 'vitest';
import { CommentsPage } from './CommentsPage';
export const DOMAIN = '00000000-0000-4000-8000-000000000001';
export const OTHER = '00000000-0000-4000-8000-000000000009';
export const WORK = '00000000-0000-4000-8000-000000000002';
export const POLICY = '00000000-0000-4000-8000-000000000003';
export const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });
export function comment(id = 'c1', eligible = true) { return { sourceRef: 'source-' + id, commentKey: { workRef: WORK, commentExternalId: id }, commentText: 'SYNTHETIC 原声 <script>不执行</script>', researchText: 'SYNTHETIC 清洗文本', sourceState: 'known', observationRole: 'primary', authorDisplayName: null, observedAt: '2026-10-18T01:00:00Z', likeCount: id === 'c1' ? 0 : null, likeCountAsOf: null, cleanState: 'direct', cleanReasons: [], signalKinds: ['question'], workTitle: '合成作品', workTitleSource: 'platform_title', studyEligibility: { eligible, reasons: eligible ? [] : ['commentAuthorUnknown'] }, latestStudy: null, effectiveStudy: null, effectiveState: 'none', postUsage: { state: 'unused', markedAt: null } }; }
export function wordCloud(domain = DOMAIN, days = 56) {
 return { domainRef: domain, days, timezone: 'Asia/Shanghai', asOf: '2026-10-18T01:00:00Z',
  algorithmVersion: 'comment-phrase-frequency.v1', eligibleCommentCount: 3, processedCommentCount: 3,
  truncated: false, textPrefixCharacters: 800, items: [
   { text: '作业拖拉', count: 3, workCount: 1, category: 'scenario',
     examples: [{ workRef: WORK, commentExternalId: 'c1' }] },
   { text: '情绪失控', count: 2, workCount: 1, category: 'emotion', examples: [] },
   { text: '三分钟关卡', count: 2, workCount: 1, category: 'approach', examples: [] },
  ] };
}
export const calls: { url: URL; method: string; body: Record<string, unknown> | null; signal: AbortSignal | null }[] = [];
export function defaultReply(url: URL) {
 if (url.pathname === '/api/local/ai-settings') return businessSettingsFixture();
 const domain = url.searchParams.get('domain') || DOMAIN, endpoint = url.pathname.split('/').at(-1);
 if (url.pathname === '/api/local/corpus/shell') return { domains: [{ domainRef: DOMAIN, name: '合成领域一', status: 'active', targetCount: 2, sampleCount: 3 }, { domainRef: OTHER, name: '合成领域二', status: 'active', targetCount: 0, sampleCount: null }], selectedDomain: domain, collectionState: '观察中' };
 if (endpoint === 'word-cloud') return wordCloud(domain, Number(url.searchParams.get('days') || 56));
 if (endpoint === 'comments') return { domainRef: domain, items: [comment(), comment('c2', false)], summary: { displayableCommentCount: 200, eligibleCommentCount: 100 }, indexCoverage: { indexedCount: 200, pendingCount: null }, page: { nextCursor: null } };
 if (endpoint === 'works') return { domainRef: domain, items: [{ workRef: WORK, displayTitle: '合成作品', displayTitleSource: 'platform_title', observationRole: 'primary', eligibleCommentCount: 100 }], totalWorkCount: 1, indexCoverage: { indexedCount: 100, pendingCount: null }, page: { nextCursor: null } };
 if (url.pathname === '/api/local/comment-research/runs') return { domainRef: domain, runs: [], limit: 100 };
 if (url.pathname === '/api/local/comment-research/problems') return { domainRef: domain, problems: [] };
 throw new Error('Unexpected test endpoint ' + url.pathname);
}
export function installFetch(custom?: (url: URL, options: RequestInit) => Promise<Response> | Response | undefined) {
 calls.length = 0;
 vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }));
 HTMLElement.prototype.scrollIntoView = vi.fn();
 vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL, options: RequestInit = {}) => {
  const url = new URL(String(input), 'http://localhost');
  calls.push({ url, method: options.method || 'GET', body: options.body ? JSON.parse(String(options.body)) as Record<string, unknown> : null, signal: options.signal ?? null });
  return custom?.(url, options) ?? json(defaultReply(url));
 }));
}
export function renderPage(search = `domain=${DOMAIN}&view=overview`, strict = false) {
 const router = createMemoryRouter([{ path: '/corpus/comments', element: <CommentsPage /> }], { initialEntries: ['/corpus/comments?' + search] });
 const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false }, mutations: { retry: false } } });
 const tree = <QueryClientProvider client={queryClient}><RouterProvider router={router} /></QueryClientProvider>;
 const view = render(strict ? <StrictMode>{tree}</StrictMode> : tree);
 return { ...view, router, queryClient };
}
