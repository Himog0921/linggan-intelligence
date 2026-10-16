import { requestJson } from '../../../shared/api';
import { parseCommentPage, parseWorkDetail, parseWorkList } from './evidenceDecode';
import { API_ROOT, apiPath } from './evidencePaths';
import { toApiParams, type EvidenceQuery } from './evidenceQuery';
import type { CommentPage, WorkDetail, WorkList } from './evidenceTypes';

/**
 * 读合同：`GET /api/local/work-resources`（列表）、`…/{ref}`（详情）、`…/{ref}/comments`（评论通道）。
 * 这三个接口都只读：不触发平台访问或采集。列表每页 50 条，扫描预算 200；评论每页 20 条。
 */
export async function fetchWorkList(domain: string, query: EvidenceQuery, cursor: string | null, signal?: AbortSignal): Promise<WorkList> {
  const params = toApiParams(domain, query, cursor);
  return parseWorkList(await requestJson(`${API_ROOT}?${params}`, signal ? { signal } : {}));
}

/** `path` 必须已经过 `detailPath` 或 `directDetailPath` 校验。 */
export async function fetchWorkDetail(path: string, signal?: AbortSignal): Promise<WorkDetail> {
  return parseWorkDetail(await requestJson(path, signal ? { signal } : {}));
}

/**
 * 评论原文只经本机授权通道、按页读取。`channelUrl` 来自详情的 `channels.comments.url`，这里再校验一次同源与前缀；
 * 续页只加一个 `cursor` 参数。
 */
export async function fetchCommentPage(channelUrl: string, cursor: string | null, signal?: AbortSignal): Promise<CommentPage> {
  const base = apiPath(channelUrl);
  if (base === null) throw new Error('评论通道地址不是本页接口');
  const url = new URL(base, 'http://local.invalid');
  if (cursor !== null) url.searchParams.set('cursor', cursor);
  return parseCommentPage(await requestJson(`${url.pathname}${url.search}`, signal ? { signal } : {}));
}
