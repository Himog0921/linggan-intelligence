import { describe, expect, it } from 'vitest';
import {
  CRITERIA,
  COEXISTENCE,
  analysisStatus,
  canRetry,
  effectiveJudgement,
  evidenceFor,
  failureReason,
  observedText,
  preservedLifecycle,
  provenanceText,
  relevanceLabel,
  sampleText,
  valueLabel,
  workTitle,
} from './creatorsPresent';
import { parseAnalysis, parseCreatorList } from './creatorsApi';
import { itemJson, listJson, workJson } from './testKit';
import type { Analysis, Observation } from './creatorsTypes';

const empty = parseAnalysis({});
const analysis = (patch: Partial<Analysis>): Analysis => ({ ...empty, ...patch });

describe('分析状态：五种结局含义不同，分别显示（#384 发现 2）', () => {
  it('排队与执行中是「在路上」，不是失败也不是无关', () => {
    expect(analysisStatus(analysis({ state: 'queued' }), 'unknown', true)).toEqual({ kind: 'queued', text: '排队待分析' });
    expect(analysisStatus(analysis({ state: 'running' }), 'unknown', true)).toEqual({ kind: 'running', text: '分析中' });
  });

  it('失败与暂停写明原因（中文），认不得的原因码不猜', () => {
    expect(analysisStatus(analysis({ state: 'failed', lastErrorCode: 'model_budget_exhausted' }), 'unknown', true).text).toBe('分析失败 · 本日额度已用尽');
    expect(analysisStatus(analysis({ state: 'paused', lastErrorCode: 'model_disabled' }), 'unknown', true).text).toBe('分析已暂停 · 模型配置未启用');
    expect(analysisStatus(analysis({ state: 'failed', lastErrorCode: 'something_new' }), 'unknown', true).text).toBe('分析失败 · 查看处理信息，确认配置后重试');
    expect(failureReason(null)).toBe('查看处理信息，确认配置后重试');
  });

  it('已分析但依据不足：分析做过了、结论仍是未知，不是失败', () => {
    const done = analysis({ state: 'idle', resultAt: '2026-10-14 10:00:00+08' });
    expect(analysisStatus(done, 'unknown', true)).toEqual({ kind: 'insufficient', text: '已分析，依据不足' });
    expect(analysisStatus(done, undefined, true).kind).toBe('insufficient');
    expect(analysisStatus(done, 'vertical_tendency', true)).toEqual({ kind: 'done', text: '已有分析结果' });
  });

  it('没有结果时区分「还没轮到」与「自动分析没开」', () => {
    expect(analysisStatus(empty, 'unknown', true)).toEqual({ kind: 'pending', text: '尚未取得当前分析结果' });
    expect(analysisStatus(empty, 'unknown', false)).toEqual({ kind: 'off', text: '自动分析未开启' });
  });

  it('只有分析已开启、且失败或暂停时才可重试', () => {
    expect(canRetry(analysis({ state: 'failed' }), true)).toBe(true);
    expect(canRetry(analysis({ state: 'paused' }), true)).toBe(true);
    expect(canRetry(analysis({ state: 'failed' }), false)).toBe(false);
    expect(canRetry(analysis({ state: 'queued' }), true)).toBe(false);
  });
});

describe('实际取样篇数：字段有就显示，缺失如实说明，绝不显示成 0 或候选数', () => {
  it('主线 worker 尚未写入这两个字段（依赖 #384 后台）：说「未记录」', () => {
    expect(sampleText(empty)).toBe('本次实际取样篇数未记录。');
  });

  it('字段在时显示实际数与当时已有数；只有实际数时不编另一个', () => {
    expect(sampleText(analysis({ sampleWorkCount: 12, availableWorkCount: 24 }))).toBe('本次实际使用 12 篇作品摘要；分析当时已有 24 篇。');
    expect(sampleText(analysis({ sampleWorkCount: 6 }))).toBe('本次实际使用 6 篇作品摘要。');
    expect(sampleText(analysis({ sampleWorkCount: 0 }))).toBe('本次实际使用 0 篇作品摘要。');
  });

  it('解码时 worker 写入的字段保持原值，没写就是 null', () => {
    const withCounts = parseCreatorList(listJson({ items: [itemJson(1, { sampleWorkCount: 12, availableWorkCount: 24 })] }));
    expect(withCounts.items[0]?.authorAnalysis.sampleWorkCount).toBe(12);
    const without = parseCreatorList(listJson({ items: [itemJson(1)] }));
    expect(without.items[0]?.authorAnalysis.sampleWorkCount).toBeNull();
  });
});

describe('观察关系的说法', () => {
  const base: Observation = {
    lookup: 'available', inCurrentDomain: false, monitoringState: 'not_enabled', lifecycleState: null, usageRole: null, targetRef: null,
    existsInOtherDomains: false, hasOtherPrimaryDomain: false,
  };
  it('关系没读到是「暂不可用」，不是「未加入」', () => {
    expect(observedText({ ...base, lookup: 'unavailable', inCurrentDomain: null })).toBe('观察关系暂不可用');
    expect(observedText({ ...base, inCurrentDomain: null })).toBe('观察关系暂不可用');
  });
  it('已加入：巡查中 / 观察已暂停 / 已加入·巡查未开启；参照用途单独标注', () => {
    expect(observedText({ ...base, inCurrentDomain: true, monitoringState: 'monitoring', usageRole: 'primary' })).toBe('巡查中');
    expect(observedText({ ...base, inCurrentDomain: true, monitoringState: 'paused', usageRole: 'reference' })).toBe('观察已暂停 · 参照');
    expect(observedText({ ...base, inCurrentDomain: true, monitoringState: 'not_enabled' })).toBe('已加入 · 巡查未开启');
  });
  it('未加入本领域：暂停 / 忽略的目标仍写明；仅其他领域有目标；完全没有', () => {
    expect(observedText({ ...base, lifecycleState: 'paused' })).toBe('观察已暂停 · 未加入本领域');
    expect(observedText({ ...base, lifecycleState: 'dismissed' })).toBe('已忽略 · 未加入本领域');
    expect(observedText({ ...base, existsInOtherDomains: true })).toBe('仅其他领域已观察');
    expect(observedText(base)).toBe('未加入观察');
    expect(preservedLifecycle({ ...base, lifecycleState: 'dismissed' })).toBe('已忽略');
    expect(preservedLifecycle(base)).toBeNull();
  });
});

describe('判断与依据：作品表达与作者倾向分层，门槛与并存关系可见（#384 发现 1）', () => {
  it('六项判断都有门槛；持续创作的门槛写明「至少两篇独立相关作品」', () => {
    for (const key of ['personal_experience', 'professional_output', 'explicit_promotion', 'institution_or_brand', 'relevance', 'focus']) {
      expect(CRITERIA[key], key).toBeTruthy();
    }
    expect(CRITERIA.focus).toContain('至少两篇独立相关作品');
    expect(CRITERIA.personal_experience).toContain('不认证经历真实性');
    expect(COEXISTENCE).toContain('作品');
    expect(COEXISTENCE).toContain('作者');
    expect(COEXISTENCE).toContain('并存');
  });

  it('判断值的中文：未见只说本次材料没看到', () => {
    expect(valueLabel('yes')).toBe('有依据');
    expect(valueLabel('no')).toBe('本次材料未见');
    expect(valueLabel('vertical_tendency')).toBe('本领域持续创作');
    expect(valueLabel(undefined)).toBe('尚不能判定');
    expect(relevanceLabel('unknown')).toBe('待判断');
    expect(relevanceLabel('related')).toBe('已判相关');
  });

  it('人工调整优先于自动结果；引用的片段随之换成人工选择的那些', () => {
    const work = parseCreatorList(listJson({ items: [itemJson(1, { works: [workJson(11)] })] })).items[0]?.representatives[0];
    expect(work).toBeDefined();
    const auto = effectiveJudgement(work!.analysis, 'personal_experience');
    expect(auto.value).toBe('yes');
    expect(evidenceFor(work!.analysis, 'personal_experience', auto).map(fragment => fragment.field)).toEqual(['body']);
    const manual = {
      ...work!.analysis,
      manual: {
        personal_experience: {
          judgement: { value: 'no', reason: '我的判断', evidenceFragmentIds: [] },
          supportFragmentIds: work!.analysis.fragments.filter(fragment => fragment.field === 'title').map(fragment => fragment.fragmentId),
          at: '2026-10-15 14:30:00.5+08',
          supportUnavailable: false,
        },
      },
    };
    expect(effectiveJudgement(manual, 'personal_experience').value).toBe('no');
    expect(evidenceFor(manual, 'personal_experience', effectiveJudgement(manual, 'personal_experience')).map(fragment => fragment.field)).toEqual(['title']);
    expect(provenanceText(manual, 'personal_experience')).toBe('人工调整 · 2026-10-15 14:30');
    expect(provenanceText(work!.analysis, 'personal_experience')).toBe('自动依据更新于 2026-10-14 10:00');
    expect(provenanceText(empty, 'relevance')).toBe('自动依据更新时间未记录');
  });

  it('没有任何判断时是「尚不能判定」，不是别的默认值', () => {
    expect(effectiveJudgement(empty, 'relevance').value).toBe('unknown');
    expect(effectiveJudgement(empty, 'focus').value).toBe('unknown');
    expect(effectiveJudgement(empty, 'explicit_promotion').value).toBe('unknown');
  });
});

describe('作品标题', () => {
  it('作品资源给出的展示标题优先，其次作品自带标题，都没有就明说', () => {
    const work = parseCreatorList(listJson({ items: [itemJson(1, { works: [workJson(11)] })] })).items[0]?.representatives[0];
    const resources = (title: string | null) => ({
      byRef: new Map([[work!.workRef, { publicRef: work!.workRef, title, avatarUrl: null, coverUrl: null }]]),
      pending: false,
      failed: false,
    });
    expect(workTitle(work!, resources('资源标题（封面识别补位）'))).toBe('资源标题（封面识别补位）');
    expect(workTitle(work!, resources(null))).toBe('合成作品 11');
    expect(workTitle({ ...work!, title: null }, resources(null))).toBe('标题未取得');
  });
});
