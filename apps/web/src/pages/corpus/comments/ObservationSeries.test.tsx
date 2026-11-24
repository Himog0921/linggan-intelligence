import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ObservationSeries } from './ObservationSeries';
import type { ObservationPoint } from './overviewDtos';
import { rangePoints } from './researchModel';

const day = (i: number) => `2026-10-${String(i + 1).padStart(2, '0')}`;
const points = (max: number, length = 7): ObservationPoint[] => Array.from({ length }, (_, i) => ({ date: day(i), newObservedCommentCount: i === 0 ? max : 0, acceptedSignalCount: 0, studiedCommentCount: null, coveredWorkCount: null, observationCoverage: i === 0 ? 'partial' : 'none' }));
const series = (list: ObservationPoint[]) => ({ timezone: 'Asia/Shanghai', points: list });
const noop = () => {};

describe('B-04 trend chart', () => {
  it('renders a zero series without invalid geometry and keeps missing coverage explicit', () => {
    render(<ObservationSeries series={series(points(0))} range={7} view="chart" onRange={noop} onView={noop} />);
    const svg = screen.getByRole('img', { name: /近7天，首次观察0条评论/ });
    expect(svg.innerHTML).not.toMatch(/NaN|Infinity/);
    expect([...svg.querySelectorAll('title')].filter(t => t.textContent?.endsWith('无记录'))).toHaveLength(6);
    expect([...svg.querySelectorAll('title')].filter(t => t.textContent?.endsWith('部分'))).toHaveLength(1);
  });
  it('labels every day within 14 days and keeps independent integer axes', () => {
    render(<ObservationSeries series={series(points(1, 14))} range={56} view="chart" onRange={noop} onView={noop} />);
    const svg = screen.getByRole('img');
    const dates = [...svg.querySelectorAll('text[text-anchor="middle"]')].map(e => e.textContent);
    expect(dates).toHaveLength(14);
    const left = [...svg.querySelectorAll('text[text-anchor="end"]')].map(e => e.textContent).filter(text => /^\d+$/.test(text ?? ''));
    expect(left).toEqual(['0', '1', '2']);
    expect(svg.querySelector('rect title')?.textContent).toBe('2026-10-01 · 首次观察 1 条评论');
  });
  it('draws at the container pixel size instead of stretching a fixed viewBox', () => {
    render(<ObservationSeries series={series(points(2))} range={7} view="chart" onRange={noop} onView={noop} />);
    const svg = screen.getByRole('img');
    expect(svg.getAttribute('viewBox')).toBe(`0 0 ${svg.getAttribute('width')} ${svg.getAttribute('height')}`);
  });
  it('starts "since data" two days before the first observation, with at least seven days', () => {
    const list = Array.from({ length: 30 }, (_, i) => ({ ...points(0, 30)[i]!, newObservedCommentCount: i >= 20 ? 1 : 0 }));
    expect(rangePoints(list, 'fit')[0]?.date).toBe(list[18]!.date);
    expect(rangePoints(points(0, 30), 'fit')).toHaveLength(7);
    expect(rangePoints(list.slice(0, 22), 'fit')).toHaveLength(7);
  });
  it('shows unobserved days as unknown in the table without hiding independent signal counts', () => {
    render(<ObservationSeries series={series(points(0))} range={7} view="table" onRange={noop} onView={noop} />);
    const cells = screen.getAllByRole('row')[1]!.querySelectorAll('td');
    expect(cells[1]).toHaveTextContent('—');
    expect(cells[2]).toHaveTextContent('0');
    expect(cells[3]).toHaveTextContent('无记录');
  });
});
