import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { describe, expect, it } from 'vitest';
import type { ShellDomain } from './corpusShellApi';
import { DomainPicker } from './DomainPicker';

const A = '00000000-0000-4000-8000-00000000000a';
const B = '00000000-0000-4000-8000-00000000000b';
const domains: ShellDomain[] = [
  { domainRef: A, name: '领域甲', status: 'active', targetCount: 3, sampleCount: 12 },
  { domainRef: B, name: '领域乙', status: 'paused', targetCount: 1, sampleCount: null },
];

function renderPicker(list: ShellDomain[], selected: string | null) {
  return render(
    <MemoryRouter initialEntries={['/corpus/creators?domain=' + A + '&sort=high_likes']}>
      <DomainPicker domains={list} selected={selected} action="/corpus/creators" />
    </MemoryRouter>,
  );
}

describe('领域选择器（既有领域状态与数量规则）', () => {
  it('一个领域也没有，或只有一个且已选中时整个控件不渲染', () => {
    const { container, unmount } = renderPicker([], null);
    expect(container).toBeEmptyDOMElement();
    unmount();
    const one = renderPicker([domains[0] as ShellDomain], A);
    expect(one.container).toBeEmptyDOMElement();
  });

  it('只有一个领域但还没选：显示「选择领域」，不替用户挑', () => {
    renderPicker([domains[0] as ShellDomain], null);
    const summary = screen.getByLabelText('切换当前观察领域');
    expect(summary).toHaveTextContent('选择领域');
    expect(summary).toHaveTextContent('请选择一个领域');
    expect(screen.getByRole('link', { name: /领域甲/ })).not.toHaveAttribute('aria-current');
  });

  it('已选领域写在摘要里；每项带名称与说明；当前项 aria-current', () => {
    renderPicker(domains, A);
    expect(screen.getByLabelText('切换当前观察领域')).toHaveTextContent('领域甲');
    expect(screen.getByLabelText('切换当前观察领域')).toHaveTextContent('3 个目标 · 12 篇作品');
    const nav = screen.getByRole('navigation', { name: '可选观察领域', hidden: true });
    const links = within(nav).getAllByRole('link', { hidden: true });
    expect(links).toHaveLength(2);
    expect(links[0]).toHaveAttribute('aria-current', 'page');
    expect(links[1]).not.toHaveAttribute('aria-current');
    expect(links[1]).toHaveAccessibleName('领域乙，已暂停');
  });

  it('切换领域只带 domain，停留在本页；此前领域下的检索与筛选不跟去', () => {
    renderPicker(domains, A);
    const nav = screen.getByRole('navigation', { name: '可选观察领域', hidden: true });
    const hrefs = within(nav).getAllByRole('link', { hidden: true }).map(link => link.getAttribute('href'));
    expect(hrefs).toEqual([`/corpus/creators?domain=${A}`, `/corpus/creators?domain=${B}`]);
  });

  it('选了一项之后收起；Esc 收起并把焦点还给摘要', async () => {
    const user = userEvent.setup();
    renderPicker(domains, A);
    const summary = screen.getByLabelText('切换当前观察领域');
    const details = summary.closest('details') as HTMLDetailsElement;
    await user.click(summary);
    await waitFor(() => expect(details.open).toBe(true));
    await user.keyboard('{Escape}');
    await waitFor(() => expect(details.open).toBe(false));
    expect(summary).toHaveFocus();
    await user.click(summary);
    await waitFor(() => expect(details.open).toBe(true));
    await user.click(screen.getByRole('link', { name: '领域乙，已暂停' }));
    await waitFor(() => expect(details.open).toBe(false));
  });
});
