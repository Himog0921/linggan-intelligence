/**
 * 由本应用提供的正式页面路径。服务器只为登记过的路径返回本应用，因此每一项都必须同时出现在
 * `apps/api/src/local_web/web_app.rs` 的 `REACT_PAGES` 里；`pageRegistry.test.ts` 逐项对照。
 */
export const LOGIN_PATH = '/login';
export const INVITATION_ACCEPT_PATH = '/invite/accept';
export const MODEL_SETTINGS_PATH = '/settings/models';
export const WORKBENCH_PATH = '/ops/creation';
export const CREATORS_PATH = '/corpus/creators';
export const EVIDENCE_PATH = '/corpus/evidence';
export const COMMENTS_PATH = '/corpus/comments';

export const COLLECTION_PATHS = ['/collection/targets', '/collection/tasks', '/collection/domains', '/collection/runtime'] as const;

export const TOPICS_PATH = '/topics';
export const OFFLINE_REVIEW_PATH = '/topics/offline-review';
export const TOPIC_WORKSPACE_PATH = '/topics/{canonical_key}';

export const OPS_PATHS = ['/ops/overview', '/ops/publications', '/ops/accounts', '/ops/daily', '/ops/io'] as const;

export const REACT_PAGE_PATHS: readonly string[] = [LOGIN_PATH, INVITATION_ACCEPT_PATH, MODEL_SETTINGS_PATH, WORKBENCH_PATH, CREATORS_PATH, EVIDENCE_PATH, COMMENTS_PATH, ...COLLECTION_PATHS, ...OPS_PATHS, TOPICS_PATH, OFFLINE_REVIEW_PATH, TOPIC_WORKSPACE_PATH];
