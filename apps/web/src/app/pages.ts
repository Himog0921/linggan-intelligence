/**
 * 由本应用提供的正式页面路径。服务器只为登记过的路径返回本应用，因此每一项都必须同时出现在
 * `apps/api/src/local_web/web_app.rs` 的 `REACT_PAGES` 里；`pageRegistry.test.ts` 逐项对照。
 */
export const MODEL_SETTINGS_PATH = '/settings/models';
export const WORKBENCH_PATH = '/ops/creation';
export const CREATORS_PATH = '/corpus/creators';

export const COLLECTION_PATHS = ['/collection/targets', '/collection/tasks', '/collection/domains', '/collection/runtime'] as const;

export const REACT_PAGE_PATHS: readonly string[] = [MODEL_SETTINGS_PATH, WORKBENCH_PATH, CREATORS_PATH, ...COLLECTION_PATHS];
