import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ObservationHistory } from './ObservationHistory';
import { parseWorkDetail } from './evidenceDecode';
import { detailJson } from './evidenceFixtures';

const point = (observedAt: string, like: number | null, comment: number | null, collect: number | null, share: number | null) => ({
  observedAt,
  sourceLane: 'detail',
  likeCount: like,
  likeCountState: like === null ? 'UNKNOWN' : 'KNOWN',
  commentCount: comment,
  commentCountState: comment === null ? 'UNKNOWN' : 'KNOWN',
  collectCount: collect,
  collectCountState: collect === null ? 'UNKNOWN' : 'KNOWN',
  shareCount: share,
  shareCountState: share === null ? 'UNKNOWN' : 'KNOWN',
});

function mount() {
  const { item, inspector } = parseWorkDetail(detailJson(1, {
    timeline: [
      point('2026-10-09T09:00:00Z', 10, 20, 7, null),
      point('2026-10-10T09:00:00Z', 15, 12, 7, null),
    ],
  }));
  return render(<ObservationHistory item={item} inspector={inspector} now={Date.parse('2026-10-11T09:00:00Z')} />);
}

describe('观察增量', () => {
  it('增加用向上的红色语义、减少用绿色、0 写持平；数字与正负号保留，未知不补 0', () => {
    mount();
    const up = screen.getByText('+5');
    expect(up).toHaveAttribute('data-trend', 'up');
    const down = screen.getByText('−8');
    expect(down).toHaveAttribute('data-trend', 'down');
    const flat = screen.getByText('持平');
    expect(flat).toHaveAttribute('data-trend', 'flat');
    expect(document.querySelectorAll('[data-trend]')).toHaveLength(3);
    expect(screen.queryByText('+0')).toBeNull();
    expect(screen.getAllByText('—').length).toBeGreaterThan(0);
  });
});
