import { ApiError } from '../../../shared/api';
import type { ReobservationOperation } from './reobservationApi';

/**
 * 动作回执的语气。D1 只有三种（已完成／未完成／未确认）；本页的写操作多一种：立即复观测可能「服务器答复了
 * 200，但没有受理」，它既不是成功也不是失败，单列「未受理」。
 *
 * 口径比 D1 更严的一处：得到 2xx 却看不懂内容时，写入可能已经发生，归入「未确认」而不是「未完成」。
 * 写请求从不自动重试；未确认之后只提供「重新核对」，由用户决定下一步。
 */
export type FeedbackTone = 'success' | 'refused' | 'error' | 'unknown';

export interface Feedback {
  tone: FeedbackTone;
  /** 行首的结论词：已完成／已提交／未受理／未完成／未确认。 */
  label: string;
  text: string;
  /** 合同里的错误码或准入结论，作为技术旁注显示；没有时为 null。 */
  code: string | null;
}

const DEFAULT_LABEL: Record<FeedbackTone, string> = { success: '已完成', refused: '未受理', error: '未完成', unknown: '未确认' };

export function feedback(tone: FeedbackTone, text: string, code: string | null = null, label: string = DEFAULT_LABEL[tone]): Feedback {
  return { tone, label, text, code };
}

// ───────────────────────── 读取失败（列表、详情、评论、底稿） ─────────────────────────

const READ_ERROR_TEXT: Readonly<Record<string, string>> = {
  domain_required: '读取需要先选择研究领域。',
  invalid_local_evidence_query: '地址里的筛选条件无效，服务器没有接受它。',
  invalid_material_query_cursor_or_sort: '继续读取的游标已失效或与当前条件不匹配，请重新读取。',
  invalid_material_channel_cursor: '继续读取的游标已失效，请重新读取评论。',
  material_comment_query_too_long: '评论检索词过长。',
  unknown_domain: '所选领域不存在。',
  material_not_found: '这篇作品不在所选领域，或当前缺少可读取的材料。',
  read_model_not_connected: '本机数据库没有连接，页面没有回退到其他数据。',
  domain_read_unavailable: '领域范围当前不可读，页面没有把未知结果显示成空列表。',
  domain_material_read_unavailable: '领域范围当前不可读，没有读取这篇作品。',
  material_projection_schema_unavailable: '本机材料读模型当前不可用。',
  material_read_projection_unavailable: '本机材料读模型当前不可用。',
  material_detail_unavailable: '作品详情当前不可读。',
  reobservation_eligibility_unavailable: '复观测资格暂时无法判断。',
  run_material_read_unavailable: '本次采集的作品范围当前不可读。',
  material_comment_channel_unavailable: '评论通道当前不可读。',
  dossier_read_unavailable: '研究底稿当前不可读。',
  creator_preview_unavailable: '完整请求预览暂不可用。',
  local_comment_study_origin_rejected: '这个页面只能在本机访问。',
};

/** 读取失败的中文说明；码不认识时返回 null，由调用方给通用说明。 */
export function readErrorText(error: unknown): string | null {
  const code = errorCode(error);
  return code ? (READ_ERROR_TEXT[code] ?? null) : null;
}

export function errorCode(error: unknown): string | null {
  return error instanceof ApiError && error.failure.kind === 'http' ? error.failure.code : null;
}

// ───────────────────────── 立即复观测（写） ─────────────────────────

const ADMISSION_COPY: Readonly<Record<string, string>> = {
  ADMITTED: '已按关联授权准入',
  MERGE: '已合并到覆盖当前作品的在途工作',
  REUSE: '既有材料已满足请求',
  DEFERRED: '请求已记录，等待可用资源',
  REFUSED: '请求已记录，但不在当前可执行范围',
  DECISION_REQUIRED: '请求已记录，尚需决定',
};

export function admissionCopy(admission: string | null): string {
  return (admission ? ADMISSION_COPY[admission] : undefined) ?? '准入结论当前未知';
}

export type AdmissionClass = 'queued' | 'reused' | 'declined';

/**
 * 回执分类。只有带着工单（或租约）引用的 `ADMITTED`／`MERGE` 才算「已提交」；`REUSE` 是既有材料已满足、
 * 没有新建工作；其余（包括 200 却被拒的 `DEFERRED`／`REFUSED`／`DECISION_REQUIRED`／`UNKNOWN`，以及缺少
 * 工单引用的异常回执）都是「未受理」——不得写成已排队。
 */
export function classifyAdmission(operation: ReobservationOperation): AdmissionClass {
  const hasWork = operation.workOrderRef !== null || operation.leaseRef !== null;
  if ((operation.admission === 'ADMITTED' || operation.admission === 'MERGE') && hasWork) return 'queued';
  return operation.admission === 'REUSE' ? 'reused' : 'declined';
}

export function reobservationReceipt(operation: ReobservationOperation): Feedback {
  switch (classifyAdmission(operation)) {
    case 'queued':
      return feedback(
        'success',
        operation.admission === 'MERGE'
          ? '已合并到覆盖当前作品的在途工单，尚未由工位认领。这不是已读取，也不是已刷新。'
          : '已入队：复观测工单已形成，尚未由工位认领。这不是已读取，也不是已刷新。',
        operation.admission,
        '已提交',
      );
    case 'reused':
      return feedback('success', '既有材料已满足请求，没有新建工作。', operation.admission);
    case 'declined': {
      const reason = operation.admissionReason ? ` 准入说明：${operation.admissionReason}` : '';
      return feedback('refused', `${admissionCopy(operation.admission)}；没有形成工单，页面不会把它写成已排队。${reason}`, operation.admission);
    }
  }
}

const REOBSERVE_ERROR_TEXT: Readonly<Record<string, string>> = {
  material_not_found: '这篇作品不在所选领域，或当前材料不可读取。',
  reobservation_platform_not_supported: '该作品的平台当前不支持复观测。',
  reobservation_authorization_not_linked: '该作品当前尚未具备监控目标关联的有效深度归档授权。',
  reobservation_admission_unavailable: '准入暂时无法完成，请稍后重试。',
  reobservation_lease_unavailable: '工单暂时无法建立，请稍后重试。',
  reobservation_unavailable: '本机数据库暂时无法完成这次请求。',
  read_model_not_connected: '本机数据库没有连接。',
  local_comment_study_origin_rejected: '这个页面只能在本机访问。',
};

export function reobservationFailure(error: unknown): Feedback {
  if (error instanceof ApiError) {
    const failure = error.failure;
    if (failure.kind === 'network') {
      return feedback('unknown', '请求没有得到答复，结果未确认，可能已经入队。页面不会自动重发：可以先“重新核对”，或到“采集任务”查看；确需再提交时会再次确认。');
    }
    if (failure.kind === 'invalid_response') {
      return feedback('unknown', '服务返回的内容无法识别，结果未确认，可能已经入队。页面不会自动重发：可以先“重新核对”，或到“采集任务”查看；确需再提交时会再次确认。');
    }
    const reason = (failure.code ? REOBSERVE_ERROR_TEXT[failure.code] : undefined) ?? '服务器没有接受这次请求。';
    return feedback('error', `本次没有形成工单。${reason}`, failure.code);
  }
  return feedback('error', '本次没有形成工单。请求没有完成。');
}
