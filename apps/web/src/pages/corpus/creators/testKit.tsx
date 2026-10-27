import { businessSettingsFixture } from '../../settings/aiSettingsFixture.test-support';
/**
 * 创作者页测试用的合成数据与假 fetch。所有内容都是合成的，不是任何真实作者、作品或原声。
 * 形状照 `GET /api/local/creators` 的真实输出（`discovery::aggregate`），不是自己想象的结构。
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router';
import { vi } from 'vitest';
import { CreatorsPage } from './CreatorsPage';

export const DOMAIN = '00000000-0000-4000-8000-000000000001';
export const OTHER_DOMAIN = '00000000-0000-4000-8000-000000000002';

export function creatorKeyFor(id: string): string {
  return `xhs:${[...new TextEncoder().encode(id)].map(byte => byte.toString(16).padStart(2, '0')).join('')}`;
}

export function workRef(n: number): string {
  return `10000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
}

export const judgement = (value: string, reason: string | null = '依据：当前可读材料支持这项判断', ids: string[] = []) => ({
  value,
  reason,
  evidenceFragmentIds: ids,
});

export function fragmentJson(id: string, field: string, text: string, ref: string | null, title: string | null = null) {
  return {
    fragmentId: id,
    field,
    sourceType: field === 'biography' ? 'author_profile' : 'work_material',
    ...(ref ? { workRef: ref } : {}),
    ...(title ? { workTitle: title } : {}),
    sourceRef: '20000000-0000-4000-8000-000000000001',
    text,
  };
}

interface WorkOptions {
  relevance?: string;
  likes?: number | null;
  state?: string | null;
  lastErrorCode?: string | null;
  withResult?: boolean;
  manual?: Record<string, unknown>;
}

export function workJson(n: number, options: WorkOptions = {}) {
  const ref = workRef(n);
  const body = fragmentJson(`${ref}.body`, 'body', `合成正文片段 ${n}：先做两分钟，再决定要不要继续。`, ref);
  const title = fragmentJson(`${ref}.title`, 'title', `合成作品 ${n}`, ref);
  const state = options.state === undefined ? 'idle' : options.state;
  const withResult = options.withResult ?? state === 'idle';
  return {
    workRef: ref,
    title: `合成作品 ${n}`,
    relevance: options.relevance ?? 'related',
    likes: options.likes === undefined ? 1200 + n : options.likes,
    collects: 30,
    comments: 8,
    acquisitionKind: n % 2 === 0 ? 'limited_domain_sample' : 'profile_discovery',
    firstAdded: '2026-10-10 08:00:00+08',
    firstAddedDisplay: '2026-10-10 08:00',
    likesObservedAt: '2026-10-12 09:30:00+08',
    likesObservedDisplay: '2026-10-12 09:30',
    analysis: {
      state,
      lastErrorCode: options.lastErrorCode ?? null,
      resultAt: withResult ? '2026-10-14 10:00:00.123456+08' : null,
      resultAtDisplay: withResult ? '2026-10-14 10:00' : null,
      automatic: withResult
        ? {
            relevance: judgement(options.relevance ?? 'related', '与当前领域的研究目标相关', [body.fragmentId]),
            traits: {
              personal_experience: judgement('yes', '记录本人具体行动与变化', [body.fragmentId]),
              professional_output: judgement('no', '没有看到概念或方法讲解', []),
              explicit_promotion: judgement('unknown', '材料不足以判断', []),
            },
          }
        : {},
      manual: options.manual ?? {},
      evidence: withResult ? [body] : [],
      supportFragments: [title, body],
    },
  };
}

interface ItemOptions {
  name?: string | null;
  followers?: number | null;
  inDomain?: boolean | null;
  monitoringState?: string | null;
  lifecycleState?: string | null;
  hasOtherPrimaryDomain?: boolean;
  existsInOtherDomains?: boolean;
  targetRef?: string | null;
  focusValue?: string;
  authorState?: string | null;
  authorErrorCode?: string | null;
  authorResult?: boolean;
  sampleWorkCount?: number | null;
  availableWorkCount?: number | null;
  works?: ReturnType<typeof workJson>[];
  traits?: string[];
  related?: number;
  unknown?: number;
  maxLikes?: number | null;
  authorManual?: Record<string, unknown>;
  displayNameKind?: string;
}

export function itemJson(n: number, options: ItemOptions = {}) {
  const externalId = `author-${n}`;
  const works = options.works ?? [workJson(n * 10 + 1), workJson(n * 10 + 2)];
  const bio = fragmentJson(`author.${n}.biography`, 'biography', '合成公开简介：记录学习方法与日常实践。', null);
  const focusValue = options.focusValue ?? 'vertical_tendency';
  const authorResult = options.authorResult ?? (options.authorState ?? 'idle') === 'idle';
  const focus = judgement(focusValue, '简介与已有作品呈现相近的内容方向', [bio.fragmentId, `${works[0]?.workRef}.body`]);
  const inDomain = options.inDomain === undefined ? false : options.inDomain;
  const unavailable = inDomain === null;
  return {
    creatorKey: creatorKeyFor(externalId),
    displayName: options.name === undefined ? `合成作者 ${n}` : options.name,
    displayNameSource: { kind: options.displayNameKind ?? 'accepted_work', workRef: works[0]?.workRef },
    authorExternalId: externalId,
    platform: 'xhs',
    profile: { displayName: `合成作者 ${n}`, biography: '合成公开简介：记录学习方法与日常实践。', followerCount: options.followers === undefined ? 2000 + n : options.followers },
    focus,
    authorAnalysis: {
      automatic: authorResult
        ? {
            focus,
            institution_or_brand: judgement('no', '简介没有机构自述', [bio.fragmentId]),
            evidence: [bio],
            sampleBasis: 'profile_discovery_sample',
            sampleLimitations: ['仅使用当前领域已取得材料，不代表账号全部作品'],
            ...(options.sampleWorkCount === undefined || options.sampleWorkCount === null ? {} : { sampleWorkCount: options.sampleWorkCount }),
            ...(options.availableWorkCount === undefined || options.availableWorkCount === null ? {} : { availableWorkCount: options.availableWorkCount }),
          }
        : {},
      manual: options.authorManual ?? {},
      institution_or_brand: authorResult ? judgement('no', '简介没有机构自述', [bio.fragmentId]) : { value: 'unknown' },
      supportFragments: [bio, ...works.map(work => fragmentJson(`${work.workRef}.body`, 'body', '合成正文片段，用于支持作者倾向。', work.workRef))],
      identitySupportFragments: [bio],
      state: options.authorState ?? 'idle',
      lastErrorCode: options.authorErrorCode ?? null,
      resultAt: authorResult ? '2026-10-14 10:00:00+08' : null,
      resultAtDisplay: authorResult ? '2026-10-14 10:00' : null,
    },
    traits: options.traits ?? ['personal_experience'],
    topicHints: ['任务启动', '晚间练习'],
    observation: unavailable
      ? { inCurrentDomain: null, monitoringState: null, existsInOtherDomains: null, hasOtherPrimaryDomain: null, targetRef: null, usageRole: null, lookupState: 'unavailable' }
      : {
          targetRef: options.targetRef ?? (inDomain ? `30000000-0000-4000-8000-${String(n).padStart(12, '0')}` : null),
          inCurrentDomain: inDomain,
          usageRole: inDomain ? 'primary' : null,
          lifecycleState: options.lifecycleState ?? (inDomain ? 'active' : null),
          monitoringState: options.monitoringState ?? (inDomain ? 'monitoring' : 'not_enabled'),
          existsInOtherDomains: options.existsInOtherDomains ?? false,
          hasOtherPrimaryDomain: options.hasOtherPrimaryDomain ?? false,
          lookupState: 'available',
        },
    collectedWorkCount: 5,
    relatedWorkCount: options.related ?? 3,
    matchedRelatedWorkCount: options.related ?? 3,
    matchedViralWorkCount: null,
    unrelatedWorkCount: 0,
    unknownRelevanceWorkCount: options.unknown ?? 2,
    knownLikeWorkCount: 3,
    viralWorkCount: null,
    maxRelatedLikeCount: options.maxLikes === undefined ? 1500 : options.maxLikes,
    candidateHighLikeCount: 1500,
    matchedWorkRefs: works.map(work => work.workRef),
    representatives: works,
    firstAdded: '2026-10-10 08:00:00+08',
    firstAddedDisplay: '2026-10-10 08:00',
  };
}

interface ListOptions {
  items?: ReturnType<typeof itemJson>[];
  total?: number;
  page?: number;
  pageSize?: number;
  domainStatus?: string;
  threshold?: number | null;
  analysisEnabled?: boolean;
  outside?: number | null;
  description?: string | null;
  researchGoal?: string | null;
  revision?: number;
  configRef?: string | null;
  dailyTokenLimit?: number | null;
}

export function listJson(options: ListOptions = {}) {
  const items = options.items ?? [itemJson(1), itemJson(2)];
  return {
    items,
    total: options.total ?? items.length,
    stats: {
      authors: 27,
      works: 80,
      unknownAuthorWorks: 4,
      relatedAuthors: 20,
      vertical: 8,
      personal: 12,
      outside: options.outside === undefined ? 20 : options.outside,
      outsideViral: options.threshold === null || options.threshold === undefined ? null : 6,
      unknownDateWorks: 3,
    },
    policy: {
      likeThreshold: options.threshold === undefined ? null : options.threshold,
      revision: options.revision ?? 0,
      analysisEnabled: options.analysisEnabled ?? false,
      configRef: options.configRef ?? null,
      dailyTokenLimit: options.dailyTokenLimit ?? null,
    },
    domain: {
      domainRef: DOMAIN,
      name: '合成领域一',
      description: options.description === undefined ? '合成领域说明' : options.description,
      researchGoal: options.researchGoal === undefined ? '合成研究目标' : options.researchGoal,
      status: options.domainStatus ?? 'active',
    },
    asOf: '2026-10-15 14:30:00.123456+08',
    displayAsOf: '2026-10-15 14:30',
    observationLookupState: options.outside === null ? 'unavailable' : 'available',
    page: options.page ?? 0,
    pageSize: options.pageSize ?? 50,
  };
}

export function shellJson(overrides: Record<string, unknown> = {}) {
  return {
    domains: [
      { domainRef: DOMAIN, name: '合成领域一', status: 'active', targetCount: 3, sampleCount: 80 },
      { domainRef: OTHER_DOMAIN, name: '合成领域二', status: 'active', targetCount: 1, sampleCount: null },
    ],
    selectedDomain: DOMAIN,
    collectionState: '观察中',
    ...overrides,
  };
}

// ───────────────────────── 假 fetch ─────────────────────────

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
  const fetchMock = vi.fn(async (input: string, init?: RequestInit) => {
    const url = new URL(input, 'http://127.0.0.1:3000');
    const method = init?.method ?? 'GET';
    const call: Call = {
      method,
      path: url.pathname,
      search: url.searchParams,
      body: typeof init?.body === 'string' ? (JSON.parse(init.body) as unknown) : undefined,
    };
    calls.push(call);
    const queue = handlers.get(`${method} ${url.pathname}`);
    const responder = queue && queue.length > 1 ? queue.shift() : queue?.[0];
    if (!responder) throw new Error(`no handler for ${method} ${input}`);
    const reply = await responder(call);
    if (reply === 'network') throw new TypeError('Failed to fetch');
    return new Response(JSON.stringify(reply.body), { status: reply.status, headers: { 'Content-Type': 'application/json' } });
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

/** 默认的只读路由：外壳、列表、作品资源、模型设置。写接口由各测试自己登记。 */
export function defaultRoutes(kit: FetchKit, list: Responder = ok(listJson())) {
  kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
  kit.route('GET', '/api/local/creators', list);
  kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
  for (let n=1; n<=40; n++) {
    kit.route('GET', `/api/local/creators/${creatorKeyFor(`author-${n}`)}/preview`,
      ok({state:'not_requested',runRef:null,requestedLimit:30,sampleCount:0,works:[]}));
  }
  kit.route('GET', '/api/local/ai-settings', ok(businessSettingsFixture()));
}

export function renderCreatorsPage(url = `/corpus/creators?domain=${DOMAIN}`) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false }, mutations: { retry: false } } });
  const router = createMemoryRouter([{ path: '/corpus/creators', element: <CreatorsPage /> }], { initialEntries: [url] });
  const view = render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { ...view, user: userEvent.setup(), client, router, location: () => router.state.location.pathname + router.state.location.search };
}
