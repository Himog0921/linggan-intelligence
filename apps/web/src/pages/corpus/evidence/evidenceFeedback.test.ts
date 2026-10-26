import { describe, expect, it } from 'vitest';
import { ApiError } from '../../../shared/api';
import { admissionCopy, classifyAdmission, errorCode, readErrorText, relayerFailure, relayerSuccess, relayerSummary, reobservationFailure, reobservationReceipt } from './evidenceFeedback';
import { operationJson } from './evidenceFixtures';
import { parseOperation } from './reobservationApi';

const operation = (overrides: Partial<Record<string, unknown>> = {}) => parseOperation(operationJson(overrides));
const http = (status: number, code: string | null) => new ApiError({ kind: 'http', status, code });

describe('重新分层的回执（W1）', () => {
  it('成功文案：本次 N 个版面：更新、已是当前版本、跳过', () => {
    const feedback = relayerSuccess({ scopeCount: 3, updated: 1, alreadyCurrent: 1, skipped: 1, needsOcr: 2, needsAcquisition: 1 });
    expect(feedback).toMatchObject({ tone: 'success', label: '已完成' });
    expect(feedback.text).toContain('本次 3 个版面：更新 1 个，已是当前版本 1 个，跳过 1 个。');
    expect(feedback.text).toContain('跳过的版面缺少可用识别行，没有处理');
    expect(feedback.text).toContain('待识字 2 张图片。');
    expect(feedback.text).toContain('待取得 1 张图片。');
    expect(feedback.text).not.toContain('识字或补采');
  });

  it('更新 0 且已是当前版本：无变化也算完成，明说已是当前版本', () => {
    expect(relayerSummary({ scopeCount: 1, updated: 0, alreadyCurrent: 1, skipped: 0, needsOcr: 0, needsAcquisition: 0 })).toMatch(/^已是当前版本，无需更新。本次 1 个版面/);
  });

  it('有明确答复的拒绝是「未完成」，一律声明没有写入；409 解释旧修订并提示可能已生效', () => {
    const stale = relayerFailure(http(409, 'dossier_changed_or_scope_unavailable'));
    expect(stale).toMatchObject({ tone: 'error', label: '未完成', code: 'dossier_changed_or_scope_unavailable' });
    expect(stale.text).toContain('本次没有写入');
    expect(stale.text).toContain('刷新后会显示已是当前版本');
    expect(relayerFailure(http(404, 'material_not_found')).text).toContain('本次没有写入');
    expect(relayerFailure(http(503, 'dossier_relayer_failed')).text).toBe('暂时无法完成，已取得的原始材料仍保留。本次没有写入。');
    expect(relayerFailure(http(422, null))).toMatchObject({ tone: 'error', code: null });
    expect(relayerFailure(new Error('x')).tone).toBe('error');
  });

  it('没有得到答复，或得到 2xx 却看不懂：是「未确认」，写入可能已发生，不当成失败', () => {
    const silent = relayerFailure(new ApiError({ kind: 'network' }));
    expect(silent).toMatchObject({ tone: 'unknown', label: '未确认' });
    expect(silent.text).toContain('可能已经生效');
    expect(silent.text).toContain('刷新底稿');
    const garbled = relayerFailure(new ApiError({ kind: 'invalid_response', status: 200 }));
    expect(garbled).toMatchObject({ tone: 'unknown', label: '未确认' });
    expect(garbled.text).toContain('可能已经生效');
  });
});

describe('立即复观测的回执（W3 / DR-3）：四种结局分开', () => {
  it('已提交：ADMITTED 带工单引用——不是已读取，也不是已刷新', () => {
    const feedback = reobservationReceipt(operation());
    expect(feedback).toMatchObject({ tone: 'success', label: '已提交', code: 'ADMITTED' });
    expect(feedback.text).toContain('复观测工单已形成，尚未由工位认领');
    expect(feedback.text).toContain('不是已读取，也不是已刷新');
  });

  it('已提交：MERGE 合并到在途工单', () => {
    const feedback = reobservationReceipt(operation({ admission: 'MERGE' }));
    expect(feedback).toMatchObject({ tone: 'success', label: '已提交' });
    expect(feedback.text).toContain('合并到覆盖当前作品的在途工单');
  });

  it('REUSE：既有材料已满足，没有新建工作——不写成已入队', () => {
    const feedback = reobservationReceipt(operation({ admission: 'REUSE', workOrderRef: null, execution: 'NOT_STARTED' }));
    expect(feedback).toMatchObject({ tone: 'success', label: '已完成' });
    expect(feedback.text).toBe('既有材料已满足请求，没有新建工作。');
  });

  it('未受理：200 却被拒（REFUSED／DEFERRED／DECISION_REQUIRED／UNKNOWN）——显示原因，绝不是成功', () => {
    for (const admission of ['REFUSED', 'DEFERRED', 'DECISION_REQUIRED', 'UNKNOWN']) {
      const result = operation({ admission, workOrderRef: null, execution: 'NOT_STARTED', admissionReason: '不在当前可执行范围的原因' });
      expect(classifyAdmission(result), admission).toBe('declined');
      const feedback = reobservationReceipt(result);
      expect(feedback, admission).toMatchObject({ tone: 'refused', label: '未受理', code: admission });
      expect(feedback.text, admission).toContain('没有形成工单');
      expect(feedback.text, admission).toContain('不在当前可执行范围的原因');
      expect(feedback.text, admission).not.toContain('已入队');
    }
  });

  it('ADMITTED／MERGE 却没有工单也没有租约引用（异常回执）：不能写成已提交', () => {
    expect(classifyAdmission(operation({ workOrderRef: null }))).toBe('declined');
    expect(classifyAdmission(operation({ admission: 'MERGE', workOrderRef: null }))).toBe('declined');
    expect(classifyAdmission(operation({ workOrderRef: null, leaseRef: 'lease-1' }))).toBe('queued');
  });

  it('准入结论文案表', () => {
    expect(admissionCopy('ADMITTED')).toBe('已按关联授权准入');
    expect(admissionCopy('MERGE')).toBe('已合并到覆盖当前作品的在途工作');
    expect(admissionCopy('REUSE')).toBe('既有材料已满足请求');
    expect(admissionCopy('DEFERRED')).toBe('请求已记录，等待可用资源');
    expect(admissionCopy('REFUSED')).toBe('请求已记录，但不在当前可执行范围');
    expect(admissionCopy('DECISION_REQUIRED')).toBe('请求已记录，尚需决定');
    expect(admissionCopy('UNKNOWN')).toBe('准入结论当前未知');
    expect(admissionCopy(null)).toBe('准入结论当前未知');
  });

  it('未完成：明确的非 2xx 答复，本次没有形成工单，按码给原因', () => {
    const cases: Record<string, string> = {
      reobservation_authorization_not_linked: '尚未具备监控目标关联的有效深度归档授权',
      reobservation_platform_not_supported: '平台当前不支持复观测',
      material_not_found: '不在所选领域',
      reobservation_admission_unavailable: '准入暂时无法完成',
      reobservation_unavailable: '本机数据库暂时无法完成',
    };
    for (const [code, text] of Object.entries(cases)) {
      const feedback = reobservationFailure(http(409, code));
      expect(feedback, code).toMatchObject({ tone: 'error', label: '未完成', code });
      expect(feedback.text, code).toContain('本次没有形成工单');
      expect(feedback.text, code).toContain(text);
    }
    expect(reobservationFailure(http(500, null)).text).toContain('服务器没有接受这次请求');
  });

  it('未确认：没有得到答复，或 2xx 却看不懂——可能已入队；页面不自动重发，给出核对与查看采集任务的路', () => {
    for (const failure of [new ApiError({ kind: 'network' }), new ApiError({ kind: 'invalid_response', status: 200 })]) {
      const feedback = reobservationFailure(failure);
      expect(feedback).toMatchObject({ tone: 'unknown', label: '未确认' });
      expect(feedback.text).toContain('可能已经入队');
      expect(feedback.text).toContain('不会自动重发');
      expect(feedback.text).toContain('重新核对');
      expect(feedback.text).toContain('采集任务');
    }
  });
});

describe('读取失败的说明', () => {
  it('按错误码给中文说明；错误码另作旁注；不认识的码返回 null', () => {
    expect(errorCode(http(503, 'read_model_not_connected'))).toBe('read_model_not_connected');
    expect(readErrorText(http(503, 'read_model_not_connected'))).toContain('本机数据库没有连接');
    expect(readErrorText(http(400, 'invalid_local_evidence_query'))).toContain('筛选条件无效');
    expect(readErrorText(http(400, 'something_new'))).toBeNull();
    expect(readErrorText(new ApiError({ kind: 'network' }))).toBeNull();
    expect(errorCode(new Error('x'))).toBeNull();
  });
});
