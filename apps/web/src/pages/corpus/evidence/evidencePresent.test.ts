import { describe, expect, it } from 'vitest';
import { parseWorkList } from './evidenceDecode';
import { listItemJson, listJson } from './evidenceFixtures';
import {
  LANE_ORDER,
  STATE_LABELS,
  compactCount,
  compactDay,
  compactMoment,
  coverObservationMoment,
  coverPublishedCopy,
  effectiveAuthor,
  knownText,
  limitationCopy,
  materialFill,
  materialSummary,
  metricReading,
  momentText,
  monitoringOriginCopy,
  publishedCopy,
  purposeLabel,
  quoteParts,
  readoutFacts,
  readoutTitle,
  segmentTitle,
  stateMeta,
  stoppedReasonMeaning,
} from './evidencePresent';
import type { WorkItem } from './evidenceTypes';

const item = (options: Parameters<typeof listItemJson>[1] = {}, n = 1): WorkItem => parseWorkList(listJson([listItemJson(n, options)])).items[0]!;

describe('状态词表（N01）', () => {
  it('每个状态码都有中文词与色调；词表外写「来源未完整表达」', () => {
    const expected: Record<string, string> = {
      UNKNOWN: '当前未知', SOURCE_TEXT_ONLY: '仅有来源时间文本', NOT_REQUESTED: '尚未请求', QUEUED: '已排队', LEASED: '租约已签发', ACTIVE: '租约有效',
      MERGED: '已合并到在途工作', RELEASED: '租约已释放', CLAIMED: '工位已认领', RUNNING: '执行中', ACCEPTED: '已接纳回执', EXPIRED: '租约已过期',
      EXPIRED_WITHOUT_RECEIPT: '租约结束，未见回执', COMPLETED_WITHOUT_RECEIPT: '任务结束，未见回执', INPUT_BLOCKED: '输入不可执行，已停止',
      NOT_OBSERVED: '尚未形成观察', OBSERVED: '已观察', PARTIAL: '部分取得', ACQUIRED: '已取得', PROCESSING: '处理中', NOT_ENABLED: '处理器未启用',
      SEARCHABLE: '可检索', KNOWN_EMPTY: '已处理，未识别到内容', FAILED: '执行失败', RISK_CONTROL: '风险控制停止', BYTES_CLEANED: '字节已清理',
      RETIRED: '旧 OCR 已停用', WITHDRAWN_OR_RESTRICTED: '已撤回或限制读取', AVAILABLE: '本地副本可用',
    };
    for (const [code, label] of Object.entries(expected)) expect(stateMeta(code)[0], code).toBe(label);
    expect(stateMeta('SOMETHING_NEW')).toEqual(['来源未完整表达', 'unknown']);
    expect(stateMeta(null)).toEqual(['来源未完整表达', 'unknown']);
  });

  it('DR-3 补的三条文案取自后端含义：页面不在／详情页没读成／尚未开始', () => {
    expect(stateMeta('PAGE_UNAVAILABLE')).toEqual(['平台页面已不存在（采集端确认）', 'warning']);
    expect(stateMeta('DETAIL_READ_BLOCKED')).toEqual(['详情页已打开，但没有读成', 'danger']);
    expect(stateMeta('NOT_STARTED')).toEqual(['尚未开始', 'unknown']);
    expect(Object.keys(STATE_LABELS)).toHaveLength(32);
  });

  it('失败与受限不共用一个色调：失败 danger、部分 warning、撤回 danger', () => {
    expect(stateMeta('FAILED')[1]).toBe('danger');
    expect(stateMeta('PARTIAL')[1]).toBe('warning');
    expect(stateMeta('WITHDRAWN_OR_RESTRICTED')[1]).toBe('danger');
    expect(LANE_ORDER).toHaveLength(9);
  });
});

describe('材料完整度（N02）', () => {
  it('状态归类：就绪／部分／失败／待处理／未知', () => {
    for (const state of ['ACQUIRED', 'SEARCHABLE', 'COMPLETE', 'KNOWN_EMPTY']) expect(materialFill(state)).toBe('ready');
    for (const state of ['PARTIAL', 'BYTES_CLEANED']) expect(materialFill(state)).toBe('partial');
    for (const state of ['FAILED', 'RISK_CONTROL', 'WITHDRAWN_OR_RESTRICTED']) expect(materialFill(state)).toBe('failed');
    for (const state of ['QUEUED', 'PROCESSING', 'OBSERVED', 'NOT_OBSERVED', 'NOT_REQUESTED', 'NOT_ENABLED']) expect(materialFill(state)).toBe('pending');
    expect(materialFill('UNKNOWN')).toBe('unknown');
    expect(materialFill('SOMETHING_NEW')).toBe('unknown');
  });

  it('图文作品没有视频：转录不适用、不进分母（读 4/4 不是 4/5）', () => {
    const summary = materialSummary(item({ lanes: { comments: 'SEARCHABLE', media_bytes: 'ACQUIRED', ocr: 'ACQUIRED' }, media: { images: 2 } }));
    expect(summary.segments.find(segment => segment.key === 'asr')).toMatchObject({ applicable: false, fill: 'not-applicable' });
    expect(summary.applicable).toBe(4);
    expect(summary.ready).toBe(4);
    expect(summary.hasGap).toBe(false);
  });

  it('尚未看过媒体：转录与图片文字仍按「未知」计入——没检查不等于不适用', () => {
    const summary = materialSummary(item());
    expect(summary.applicable).toBe(5);
    expect(summary.segments.find(segment => segment.key === 'ocr')).toMatchObject({ applicable: true, fill: 'unknown' });
    expect(summary.ready).toBe(1);
  });

  it('全部未知时 ready 是 null，不是系统无法担保的 0', () => {
    const summary = materialSummary(item({ lanes: { detail: 'UNKNOWN' } }));
    expect(summary.ready).toBeNull();
    expect(summary.hasGap).toBe(false);
  });

  it('失败与部分被标出；提示写明哪个维度、什么状态', () => {
    const summary = materialSummary(item({ lanes: { comments: 'RISK_CONTROL', media_bytes: 'BYTES_CLEANED' }, media: { images: 1 } }));
    expect(summary.failed).toBe(true);
    expect(summary.partial).toBe(true);
    expect(segmentTitle(summary.segments[1]!)).toBe('评论：风险控制停止');
    expect(segmentTitle(summary.segments[4]!)).toBe('转录：本作品不适用，不计入分母');
  });

  it('侧栏徽标的读数：本次读取 N 个作品，不编造总数；未知评论数写「未知」', () => {
    const items = [item({ lanes: { comments: ['SEARCHABLE', 1500] }, media: { images: 1, cover: 'available' } }, 1), item({ lanes: { media_bytes: 'FAILED' } }, 2)];
    const facts = readoutFacts(items);
    expect(facts.works).toBe(2);
    expect(facts.commentsRetained).toBe(1500);
    expect(facts.localObjects).toBeGreaterThan(0);
    expect(facts.gaps).toBeGreaterThanOrEqual(1);
    expect(readoutTitle(facts)).toMatch(/^本次读取 2 个作品 · 已留存评论 1\.5K · 本地材料 \d+ · 部分或缺口 \d+$/);
    expect(readoutTitle(readoutFacts([item()]))).toContain('已留存评论 未知');
  });
});

describe('作者规则（作者合并决定 2026-09-04）', () => {
  it('详情里的作者名优先', () => {
    expect(effectiveAuthor(item({ author: '平台作者', context: { targetKind: 'creator', targetDisplayState: 'KNOWN', targetDisplayName: '监控目标' } }))).toBe('平台作者');
  });

  it('没有作者名时回退到 creator 型监控目标的名称', () => {
    expect(effectiveAuthor(item({ author: null, context: { targetKind: 'creator', targetDisplayState: 'KNOWN', targetDisplayName: '监控目标' } }))).toBe('监控目标');
  });

  it('身份比对为 MISMATCH 时不回退：不断言平台否认的作者归属', () => {
    expect(effectiveAuthor(item({ author: null, context: { authorIdentityMatchState: 'MISMATCH', targetKind: 'creator', targetDisplayState: 'KNOWN', targetDisplayName: '监控目标' } }))).toBe('当前未知');
  });

  it('关键词型目标是搜索词不是人，永不填作者；都没有写「当前未知」', () => {
    expect(effectiveAuthor(item({ author: null, context: { targetKind: 'keyword', targetDisplayState: 'KNOWN', targetDisplayName: '考研' } }))).toBe('当前未知');
    expect(effectiveAuthor(item({ author: null }))).toBe('当前未知');
  });

  it('来源监控留在检查器：创作者／关键词各带类型；未知写「当前未知」', () => {
    expect(monitoringOriginCopy(item({ context: { targetKind: 'creator', targetDisplayState: 'KNOWN', targetDisplayName: '博主甲' } }))).toBe('博主甲（博主监控）');
    expect(monitoringOriginCopy(item({ context: { targetKind: 'keyword', targetDisplayState: 'KNOWN', targetDisplayName: '考研' } }))).toBe('考研（关键词监控）');
    expect(monitoringOriginCopy(item({ context: { targetKind: 'other', targetDisplayState: 'KNOWN', targetDisplayName: '某目标' } }))).toBe('某目标');
    expect(monitoringOriginCopy(item())).toBe('当前未知');
  });
});

describe('时间与数量的写法', () => {
  it('发布时间：已知／仅有来源文本／未知；精度为「日」只显示日期，不补成整分钟（H14）', () => {
    expect(publishedCopy(item().display)).toBe('2026-10-13 13:28');
    expect(publishedCopy(item().display, { compact: true })).toBe('2026-10-13');
    expect(publishedCopy(item({ publishedPrecision: 'day' }).display)).toBe('2026-10-13');
    expect(publishedCopy(item({ published: null, publishedState: 'SOURCE_TEXT_ONLY', publishedSourceText: '3小时前' }).display)).toBe('来源时间：3小时前');
    expect(publishedCopy(item({ published: null, publishedState: 'SOURCE_TEXT_ONLY', publishedSourceText: null }).display)).toBe('来源时间：已观察');
    expect(publishedCopy(item({ published: null }).display)).toBe('发布时间当前未知');
    expect(publishedCopy(item({ published: null }).display, { withFieldName: false })).toBe('当前未知');
    expect(coverPublishedCopy(item({ published: null, publishedState: 'SOURCE_TEXT_ONLY' }).display)).toBe('来源时间');
    expect(coverPublishedCopy(item().display)).toBe('2026-10-13');
  });

  it('紧凑时间：行内到分钟，铭牌写 MM-DD HH:MM；认不出的原样保留', () => {
    expect(compactMoment('2026-10-10 17:00:12.5+08')).toBe('2026-10-10 17:00');
    expect(compactMoment('2026-10-10T09:00:00Z')).toBe('2026-10-10 09:00');
    expect(compactMoment('昨天')).toBe('昨天');
    expect(compactMoment(null)).toBeNull();
    expect(compactDay('2026-10-10 17:00')).toBe('2026-10-10');
    expect(coverObservationMoment('2026-10-10 17:00')).toBe('10-10 17:00');
    expect(coverObservationMoment(null)).toBe('未知');
  });

  it('数据库原串统一成北京时间 YYYY-MM-DD HH:MM；空值写当前未知', () => {
    expect(momentText('2026-10-10T09:00:00Z')).toBe('2026-10-10 17:00');
    expect(momentText('2026-10-16 13:28:09.519695+08')).toBe('2026-10-16 13:28');
    expect(momentText('2026-10-16 13:28')).toBe('2026-10-16 13:28');
    expect(momentText(null)).toBe('当前未知');
    expect(momentText('  ', '尚未形成')).toBe('尚未形成');
  });

  it('未知不是 0：互动未知写「未知」；已知按千分位', () => {
    expect(metricReading({ value: 12345, state: 'KNOWN' })).toBe('12,345');
    expect(metricReading({ value: 0, state: 'KNOWN' })).toBe('0');
    expect(metricReading({ value: null, state: 'UNKNOWN' })).toBe('未知');
    expect(metricReading({ value: 5, state: 'UNKNOWN' })).toBe('未知');
    expect(knownText('x', 'KNOWN')).toBe('x');
    expect(knownText('x', 'UNKNOWN')).toBe('当前未知');
    expect(knownText(null, 'KNOWN', '标题当前未知')).toBe('标题当前未知');
    expect(compactCount(999)).toBe('999');
    expect(compactCount(1500)).toBe('1.5K');
    expect(compactCount(25_000)).toBe('25K');
    expect(compactCount(2_500_000)).toBe('2.5M');
  });

  it('限制码、停止原因与用途的中文说明（H12）', () => {
    expect(limitationCopy('OTHER_LANES_NOT_EVALUATED')).toBe('其他材料通道尚未评估');
    expect(limitationCopy('NEW_CODE')).toBe('来源未完整表达');
    const expected: Record<string, string> = {
      quota_reached: '已达到本次配额', maximum_quota: '已达到本次配额', surface_ended: '当前页面内容已结束', surface_read_complete: '当前页面读取完成',
      risk_control: '平台风险控制导致停止', manual_stop: '用户手动停止', unknown: '停止原因未知', future_reason: '停止原因未归类',
    };
    for (const [code, meaning] of Object.entries(expected)) expect(stoppedReasonMeaning(code), code).toBe(meaning);
    expect(purposeLabel('body_image')).toBe('正文图片');
    expect(purposeLabel('mystery')).toBe('mystery');
  });
});

describe('原声高亮（E04）', () => {
  it('按字符偏移切片，兼容中文与表情（一个表情不止一个 JavaScript 下标）', () => {
    expect(quoteParts({ text: '先做两分钟再决定', matchOffset: 2, matchLength: 3 })).toEqual({ before: '先做', match: '两分钟', after: '再决定' });
    expect(quoteParts({ text: '😀😀考研资料', matchOffset: 2, matchLength: 2 })).toEqual({ before: '😀😀', match: '考研', after: '资料' });
  });

  it('没有命中范围或越界：整段原样，不高亮', () => {
    expect(quoteParts({ text: '合成', matchOffset: null, matchLength: null })).toBeNull();
    expect(quoteParts({ text: '合成', matchOffset: 1, matchLength: 5 })).toBeNull();
    expect(quoteParts({ text: '合成', matchOffset: 0, matchLength: 0 })).toBeNull();
  });
});
