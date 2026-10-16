import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { lazy, Suspense } from 'react';
import { createBrowserRouter } from 'react-router';
import { RouterProvider } from 'react-router/dom';
import { ModelSettingsPage } from '../pages/settings/ModelSettingsPage';
import { NotFoundPage } from './NotFoundPage';
import { OPS_PATHS, COLLECTION_PATHS, CREATORS_PATH, MODEL_SETTINGS_PATH, WORKBENCH_PATH, TOPICS_PATH } from './pages';

const WorkbenchPage = lazy(() => import('../pages/workbench/WorkbenchPage').then(module => ({ default: module.WorkbenchPage })));
// 语料页按需加载：打开设置页或工作台时不下载创作者页的代码。
const CreatorsPage = lazy(() => import('../pages/corpus/creators/CreatorsPage').then(module => ({ default: module.CreatorsPage })));


const OpsPage = lazy(() => import('../pages/ops/OpsPage').then(m => ({ default: m.OpsPage })));

const collectionPages = ['TargetsPage', 'TasksPage', 'DomainsPage', 'RuntimePage'] as const;
const TargetsPage = lazy(() => import('../pages/collection/targets/TargetsPage').then(m => ({ default: m.TargetsPage })));
const TasksPage = lazy(() => import('../pages/collection/tasks/TasksPage').then(m => ({ default: m.TasksPage })));
const DomainsPage = lazy(() => import('../pages/collection/management/DomainsPage').then(m => ({ default: m.DomainsPage })));
const RuntimePage = lazy(() => import('../pages/collection/management/RuntimePage').then(m => ({ default: m.RuntimePage })));

// 读取失败立即如实显示，由用户决定是否刷新；写操作永远不自动重试（重试必须带原请求身份）。
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false },
    mutations: { retry: false },
  },
});

const router = createBrowserRouter([
  { path: MODEL_SETTINGS_PATH, element: <ModelSettingsPage /> },
  { path: WORKBENCH_PATH, element: <Suspense fallback={<p role="status">正在打开工作台…</p>}><WorkbenchPage /></Suspense> },
  { path: CREATORS_PATH, element: <Suspense fallback={<p role="status">正在打开创作者…</p>}><CreatorsPage /></Suspense> },
  ...[TargetsPage, TasksPage, DomainsPage, RuntimePage].map((Page, i) => ({ path: COLLECTION_PATHS[i], element: <Suspense fallback={<p role="status">正在打开采集页面…</p>}><Page key={collectionPages[i]} /></Suspense> })),
  ...OPS_PATHS.map(path => ({ path, element: <Suspense fallback={<p role="status">正在打开运营页面…</p>}><OpsPage /></Suspense> })),
  { path: TOPICS_PATH, HydrateFallback: () => <p role="status">正在打开主题图谱…</p>, lazy: async () => ({ Component: (await import('../pages/topics/TopicsPage')).TopicsPage }) },
  { path: '/topics/:canonical_key', HydrateFallback: () => <p role="status">正在打开 Topic 工作区…</p>, lazy: async () => ({ Component: (await import('../pages/topics/WorkspacePage')).WorkspacePage }) },
  { path: '*', element: <NotFoundPage /> },
]);

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
