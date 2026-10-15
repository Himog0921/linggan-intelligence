import { describe, expect, it } from 'vitest';
import { ApiError } from '../../shared/api';
import { parseModelSettings, readProbeOutcome, type ModelEntry } from './modelSettingsApi';
import { feedbackForError, feedbackForProbe, isCallable, modelStateLabel } from './modelSettingsModel';

const base: ModelEntry = {
  modelRef: 'm1',
  modelId: 'model-a',
  connectionVersionRef: 'v1',
  connectionName: '连接 A',
  semanticQualified: true,
  modelCallable: true,
  enabled: true,
};

describe('模型状态', () => {
  it('只有启用、可调用且通过 V1 JSON 测试的模型可选为默认', () => {
    expect(isCallable(base)).toBe(true);
    expect(isCallable({ ...base, enabled: false })).toBe(false);
    expect(isCallable({ ...base, semanticQualified: false })).toBe(false);
    expect(isCallable({ ...base, modelCallable: false })).toBe(false);
    expect(isCallable(undefined)).toBe(false);
  });

  it('状态说明与旧页面口径一致', () => {
    expect(modelStateLabel(base)).toBe('可用于评论研究 V1');
    expect(modelStateLabel({ ...base, enabled: false })).toBe('连接已停用');
    expect(modelStateLabel({ ...base, semanticQualified: false })).toBe('模型已响应，但 V1 JSON 测试未通过');
    expect(modelStateLabel({ ...base, semanticQualified: false, modelCallable: false })).toBe('需要完成模型测试');
  });
});

describe('错误说明', () => {
  it('合同错误码给出中文说明并保留错误码', () => {
    expect(feedbackForError(new ApiError({ kind: 'http', status: 409, code: 'model_revision_conflict' }))).toEqual({
      tone: 'error',
      text: '设置已变化，请刷新后重试。',
      code: 'model_revision_conflict',
    });
  });

  it('不认识的错误码不编造原因', () => {
    expect(feedbackForError(new ApiError({ kind: 'http', status: 400, code: 'something_new' }))).toEqual({
      tone: 'error',
      text: '请求没有完成，请保留当前输入后重试。',
      code: 'something_new',
    });
  });

  it('没有得到答复时说明结果未确认，而不是失败', () => {
    const feedback = feedbackForError(new ApiError({ kind: 'network' }));
    expect(feedback.tone).toBe('unknown');
    expect(feedback.text).toContain('结果未确认');
  });
});

describe('测试回执', () => {
  it('首次测试：按 semanticQualified / modelCallable 判断', () => {
    expect(readProbeOutcome({ ok: true, semanticQualified: true, modelCallable: true })).toEqual({ kind: 'qualified' });
    expect(readProbeOutcome({ ok: true, semanticQualified: false, modelCallable: true })).toEqual({ kind: 'callable_not_qualified' });
    expect(readProbeOutcome({ ok: false, failureCode: 'provider_timeout' })).toEqual({ kind: 'failed', failureCode: 'provider_timeout' });
  });

  it('重放：结论在 result 里，不能误报未通过', () => {
    expect(readProbeOutcome({ invocationRef: 'i', state: 'succeeded', replayed: true, result: { semanticQualified: true } })).toEqual({
      kind: 'qualified',
    });
  });

  it('重放时上一次测试仍在进行：如实说明未确认', () => {
    const outcome = readProbeOutcome({ invocationRef: 'i', state: 'running', replayed: true, result: null });
    expect(outcome).toEqual({ kind: 'still_running' });
    expect(feedbackForProbe(outcome).tone).toBe('unknown');
  });

  it('回执不是对象时不当成成功', () => {
    expect(() => readProbeOutcome('ok')).toThrow(ApiError);
  });
});

describe('读取结果校验', () => {
  const valid = {
    worker: { lastSeenAt: null, recent: false, lastError: null },
    connections: [],
    models: [],
    config: null,
    invocations: [],
  };

  it('合法结果原样采用', () => {
    expect(parseModelSettings(valid).connections).toEqual([]);
  });

  it('缺字段或类型不对时报无法解析，不拿残缺数据渲染', () => {
    expect(() => parseModelSettings({ ...valid, connections: 'none' })).toThrow(ApiError);
    expect(() => parseModelSettings({ ...valid, worker: undefined })).toThrow(ApiError);
    expect(() =>
      parseModelSettings({ ...valid, invocations: [{ invocationRef: 'i', operation: 'probe', state: 'failed', chargedTokens: '1', createdAt: 'x' }] }),
    ).toThrow(ApiError);
  });
});
