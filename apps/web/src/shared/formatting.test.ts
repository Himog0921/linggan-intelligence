import { describe, expect, it } from 'vitest';
import { countOrUnknown, humanMoment } from './formatting';

describe('humanMoment（与 linggan_human_moment 同一写法）', () => {
  it('把 ISO 时间换成北京时间的 YYYY-MM-DD HH:MM', () => {
    expect(humanMoment('2026-10-15T06:05:09.916Z')).toBe('2026-10-15 14:05');
  });

  it('认得 PostgreSQL timestamptz::text 的写法', () => {
    expect(humanMoment('2026-10-15 14:30:00.123456+08')).toBe('2026-10-15 14:30');
    expect(humanMoment('2026-10-14 23:59:59+00')).toBe('2026-10-15 07:59');
    expect(humanMoment('2026-10-15 01:02:03-05:30')).toBe('2026-10-15 14:32');
  });

  it('跨日按北京时间计算', () => {
    expect(humanMoment('2026-12-31T16:00:00Z')).toBe('2027-01-01 00:00');
  });

  it('空值显示未知，不编造时间', () => {
    expect(humanMoment(null)).toBe('未知');
    expect(humanMoment(undefined)).toBe('未知');
    expect(humanMoment('  ')).toBe('未知');
  });

  it('认不出来就原样返回', () => {
    expect(humanMoment('昨天下午')).toBe('昨天下午');
    expect(humanMoment('2026-10-15 14:30')).toBe('2026-10-15 14:30');
  });
});

describe('countOrUnknown', () => {
  it('未知不是 0', () => {
    expect(countOrUnknown(null)).toBe('未知');
    expect(countOrUnknown(undefined)).toBe('未知');
    expect(countOrUnknown(0)).toBe('0');
    expect(countOrUnknown(1024)).toBe('1024');
  });
});
