import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it } from 'vitest';
import spec from '../../../../docs/design/lids/iconography.json';
import { IntelligenceIcon, type IntelligenceIconName } from './IntelligenceIcon';

it('renders every approved icon as a local decorative SVG', () => {
  const { container } = render(<>{spec.icons.map(icon => <IntelligenceIcon key={icon.name} name={icon.name as IntelligenceIconName}/>)}</>);
  expect([...container.querySelectorAll('svg')].map(svg => svg.getAttribute('data-icon')))
    .toEqual(spec.icons.map(icon => icon.name));
  for (const svg of container.querySelectorAll('svg')) {
    expect(svg).toHaveAttribute('viewBox', '0 0 24 24');
    expect(svg).toHaveAttribute('aria-hidden', 'true');
    expect(svg).toHaveAttribute('focusable', 'false');
    expect(svg.childElementCount).toBeGreaterThan(0);
  }
  expect(container.querySelector('image,use,script')).toBeNull();
});

it('keeps keyboard focus and accessible naming on the owning control', async () => {
  const user = userEvent.setup();
  render(<><button type="button" aria-label="刷新底稿"><IntelligenceIcon name="refresh"/></button><button type="button" disabled aria-label="当前不能导出"><IntelligenceIcon name="download"/></button><button type="button" aria-label="下一步">下一步</button></>);
  await user.tab(); expect(screen.getByRole('button', { name: '刷新底稿' })).toHaveFocus();
  await user.tab(); expect(screen.getByRole('button', { name: '下一步' })).toHaveFocus();
  expect(screen.queryByRole('img')).not.toBeInTheDocument();
});

it('binds chapter direction to its explicit expanded state', () => {
  const { container, rerender } = render(<IntelligenceIcon name="chevron" expanded/>);
  expect(container.querySelector('svg')).toHaveAttribute('data-expanded', 'true');
  rerender(<IntelligenceIcon name="chevron" expanded={false}/>);
  expect(container.querySelector('svg')).not.toHaveAttribute('data-expanded');
});
