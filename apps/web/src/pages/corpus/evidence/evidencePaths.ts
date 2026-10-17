/**
 * 证据库只认两类同源地址：受控本地媒体句柄（`/api/local/media/…`、`/api/local/derivative/…`）和本页自己的
 * 接口（`/api/local/work-resources/…`）。其余一律不当图片用、不当接口用、不跟随——永不回退到平台 CDN，
 * 也不猜后端路由（G08）。
 */
import { UUID_PATTERN } from './evidenceQuery';

export const API_ROOT = '/api/local/work-resources';
export const MEDIA_PREFIX = '/api/local/media/';
export const DERIVATIVE_PREFIX = '/api/local/derivative/';

function currentOrigin(): string {
  return globalThis.location?.origin ?? 'http://localhost';
}

/** 同源且路径以允许的前缀之一开头才返回「路径 + 查询串」；绝对地址只要同源也可以。 */
export function controlledPath(url: unknown, prefixes: readonly string[], origin: string = currentOrigin()): string | null {
  if (typeof url !== 'string' || url === '') return null;
  try {
    const parsed = new URL(url, origin);
    if (parsed.origin !== origin) return null;
    return prefixes.some(prefix => parsed.pathname.startsWith(prefix)) ? `${parsed.pathname}${parsed.search}` : null;
  } catch {
    return null;
  }
}

/** 图片句柄：本机媒体副本或本机派生物。 */
export function imagePath(url: unknown): string | null {
  return controlledPath(url, [MEDIA_PREFIX, DERIVATIVE_PREFIX]);
}

/** 只认本机媒体副本（头像、原图对照、查看器）。 */
export function mediaPath(url: unknown): string | null {
  return controlledPath(url, [MEDIA_PREFIX]);
}

/** 本页接口地址：必须同源、在作品资源根下。 */
export function apiPath(url: unknown): string | null {
  return controlledPath(url, [`${API_ROOT}/`]);
}

/**
 * 列表项给出的详情地址：同源、在作品资源根下，且正好是「根/作品引用」这一层（不是评论、底稿、复观测等
 * 子通道）。否则返回 null，不猜路由。
 */
export function detailPath(url: unknown): string | null {
  const path = apiPath(url);
  return path !== null && /^\/api\/local\/work-resources\/[^/?]+(\?|$)/.test(path) ? path : null;
}

/** 地址里的 `work` 不在已读到的页内时直读详情：保持所选领域与 `collectionRunRef`。 */
export function directDetailPath(ref: string, domain: string, collectionRunRef: string | null): string | null {
  if (!UUID_PATTERN.test(ref)) return null;
  const params = new URLSearchParams({ domain });
  if (collectionRunRef !== null && collectionRunRef !== '') params.set('collectionRunRef', collectionRunRef);
  return `${API_ROOT}/${encodeURIComponent(ref)}?${params}`;
}
