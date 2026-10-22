import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ObservationChart, ObservationSeries } from './ObservationSeries';
import type { ObservationPoint } from './overviewDtos';
const points = (max: number): ObservationPoint[] => Array.from({ length: 7 }, (_, i) => ({ date: `2026-10-${String(i + 1).padStart(2, '0')}`, newObservedCommentCount: i === 0 ? max : 0, acceptedSignalCount: 0, studiedCommentCount: null, coveredWorkCount: null, observationCoverage: i === 0 ? 'partial' : 'none' }));
describe('D3 observation counts and coverage', () => {
  it('renders a zero series without invalid geometry and keeps missing coverage explicit', () => {
    render(<ObservationChart points={points(0)} />);
    const svg = screen.getByRole('img', { name: '评论柱状与研究信号渐变曲线，共同横轴及采集覆盖' });
    expect(svg.innerHTML).not.toMatch(/NaN|Infinity/);
    expect(screen.getAllByRole('img', { name: /无观察记录/ })).toHaveLength(6);
    expect(svg.querySelectorAll('rect[data-state="partial"]')).toHaveLength(1);
    expect(svg.querySelectorAll('rect[data-state="none"]')).toHaveLength(6);
  });
  it('uses unique integer axis labels for a single observed comment', () => {
    render(<ObservationChart points={points(1)} />);
    const svg = screen.getByRole('img', { name: '评论柱状与研究信号渐变曲线，共同横轴及采集覆盖' });
    const axis = [...svg.querySelectorAll('text[text-anchor="end"]')].filter(e => Number(e.getAttribute('x')) === 40 && Number(e.getAttribute('y')) < 400).map(e => e.textContent);
    expect(axis).toEqual(['1', '0']);
    expect(screen.getByRole('img', { name: /2026-10-01：首次观察 1 条/ })).toBeInTheDocument();
  });
  it('shows unobserved days as unknown without hiding independent signal counts', () => {
    render(<ObservationSeries series={{ timezone: 'Asia/Shanghai', points: points(0) } as never}
      days={7} view="table" onDays={() => {}} onView={() => {}} />);
    const cells = screen.getAllByRole('row')[1]!.querySelectorAll('td');
    expect(cells[1]).toHaveTextContent('—');
    expect(cells[3]).toHaveTextContent('0');
  });
});
