import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { describe, expect, it, vi } from 'vitest';
import type { ShellDomain } from '../../../corpus/corpusShellApi';
import { DomainChoiceDialog } from './DomainChoiceDialog';
import { DOMAIN, OTHER_DOMAIN, workRef } from './evidenceFixtures';

const domains: ShellDomain[] = [
  { domainRef: DOMAIN, name: '合成领域一', status: 'active', targetCount: 3, sampleCount: 80 },
  { domainRef: OTHER_DOMAIN, name: '合成领域二', status: 'paused', targetCount: 1, sampleCount: null },
  { domainRef: '00000000-0000-4000-8000-000000000003', name: '合成领域三', status: 'active', targetCount: 0, sampleCount: null },
];

function setup(list: ShellDomain[] = domains, pendingWork: string | null = null) {
  const onClose = vi.fn();
  render(
    <MemoryRouter>
      <DomainChoiceDialog domains={list} pendingWork={pendingWork} onClose={onClose} />
    </MemoryRouter>,
  );
  return { onClose, user: userEvent.setup() };
}

describe('选择研究领域', () => {
  it('弹出模态，说明与每个领域的状态：运行中／已暂停／作品数未知', () => {
    setup();
    const dialog = screen.getByRole('dialog', { name: '选择研究领域' });
    expect(within(dialog).getByText('选择后读取该领域的作品材料；也可随后在页头切换。')).toBeInTheDocument();
    const nav = within(dialog).getByRole('navigation', { name: '选择语料领域' });
    const links = within(nav).getAllByRole('link');
    expect(links.map(link => link.getAttribute('aria-label'))).toEqual([
      '合成领域一，运行中 · 3 个目标 · 80 篇作品',
      '合成领域二，已暂停 · 历史材料可读',
      '合成领域三，运行中 · 0 个目标 · 作品数未知',
    ]);
    expect(links[0]).toHaveAttribute('href', `/corpus/evidence?domain=${DOMAIN}`);
  });

  it('地址带着合法 work 但没选领域：每个领域链接都带上它，选定后继续定位这篇作品', () => {
    setup(domains, workRef(7));
    for (const link of within(screen.getByRole('navigation', { name: '选择语料领域' })).getAllByRole('link')) {
      expect(link.getAttribute('href')).toMatch(new RegExp(`&work=${workRef(7)}$`));
    }
  });

  it('一个领域也没有：写明，并给出前往领域管理的出口', () => {
    setup([]);
    expect(screen.getByText('还没有可选择的研究领域。')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '前往领域管理 →' })).toHaveAttribute('href', '/collection/domains');
    expect(screen.queryByRole('navigation', { name: '选择语料领域' })).toBeNull();
  });

  it('关闭只回到空态，不代选：关闭按钮与 Esc 都只调用 onClose', async () => {
    const { user, onClose } = setup();
    await user.click(screen.getByRole('button', { name: '关闭领域选择' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    const dialog = screen.getByRole('dialog', { name: '选择研究领域' });
    dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
