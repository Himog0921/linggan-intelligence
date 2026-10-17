import { requestJson } from '../../../shared/api';
import {
  parseCreatorPreview,
  parseDossierComments,
  parseDossierPayload,
  parseRelayerPlanResponse,
  parseRelayerReceipt,
} from './dossierDecode';
import type { CreatorPreview, DossierCommentPage, DossierPayload, RelayerPlan, RelayerReceipt } from './dossierTypes';
import { API_ROOT } from './evidencePaths';

/**
 * 研究底稿读写合同：`apps/api/src/local_web/research_dossier.rs`。读接口都按领域校验作品，不在该领域返回 404
 * `material_not_found`；它们不调用模型、不预留额度、不触发采集。
 */
function base(work: string): string {
  return `${API_ROOT}/${encodeURIComponent(work)}/research-dossier`;
}

function scope(domain: string): string {
  return `domain=${encodeURIComponent(domain)}`;
}

export async function fetchDossier(work: string, domain: string, signal?: AbortSignal): Promise<DossierPayload> {
  return parseDossierPayload(await requestJson(`${base(work)}?${scope(domain)}`, signal ? { signal } : {}));
}

/** 批量整理弹窗用：只读重新分层计划。 */
export async function fetchRelayerPlan(work: string, domain: string, signal?: AbortSignal): Promise<RelayerPlan> {
  return parseRelayerPlanResponse(await requestJson(`${base(work)}/relayer-plan?${scope(domain)}`, signal ? { signal } : {}));
}

/** 「按当前模型预览完整请求」：只读，不调用模型。 */
export async function fetchCreatorPreview(work: string, domain: string, signal?: AbortSignal): Promise<CreatorPreview> {
  return parseCreatorPreview(await requestJson(`${base(work)}/creator-preview?${scope(domain)}`, signal ? { signal } : {}));
}

/** 底稿内的已采评论，每页 30 条。 */
export async function fetchDossierComments(work: string, domain: string, cursor: string | null, signal?: AbortSignal): Promise<DossierCommentPage> {
  const cursorPart = cursor === null ? '' : `&cursor=${encodeURIComponent(cursor)}`;
  return parseDossierComments(await requestJson(`${base(work)}/comments?${scope(domain)}${cursorPart}`, signal ? { signal } : {}));
}

/**
 * 重新分层（写）。没有请求编号，但天然幂等：只追加「（版面，规则版本）」唯一的分层结果，重复提交不会产生第二条。
 * 但第一次真的更新之后底稿修订变了，带着旧 `expectedRevision` 重发得到 409——那不表示第一次失败。
 * 所以未确认之后不盲目重发旧修订：先刷新底稿，再用新读到的修订重新发起。
 */
export async function relayerDossier(work: string, domain: string, expectedRevision: string): Promise<RelayerReceipt> {
  return parseRelayerReceipt(await requestJson(`${base(work)}/relayer`, { method: 'POST', body: { expectedRevision, domain } }));
}
