import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { lazy, Suspense } from 'react';
import { createBrowserRouter } from 'react-router';
import { RouterProvider } from 'react-router/dom';
import { ModelSettingsPage } from '../pages/settings/ModelSettingsPage';
import { NotFoundPage } from './NotFoundPage';
import { CREATORS_PATH, EVIDENCE_PATH, MODEL_SETTINGS_PATH, WORKBENCH_PATH } from './pages';

const WorkbenchPage = lazy(() => import('../pages/workbench/WorkbenchPage').then(module => ({ default: module.WorkbenchPage })));
// 语料页按需加载：打开设置页或工作台时不下载创作者页的代码。
const CreatorsPage = lazy(() => import('../pages/corpus/creators/CreatorsPage').then(module => ({ default: module.CreatorsPage })));
const EvidencePage = lazy(() => import('../pages/corpus/evidence/EvidencePage').then(module => ({ default: module.EvidencePage })));

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
  { path: EVIDENCE_PATH, element: <Suspense fallback={<p role="status">正在打开证据库…</p>}><EvidencePage /></Suspense> },
  { path: '*', element: <NotFoundPage /> },
]);

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
