import { render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { describe, expect, it } from 'vitest';
import { CorpusFrame } from './CorpusFrame';
import type { CorpusShell } from './corpusShellApi';

const A = '00000000-0000-4000-8000-00000000000a';
const B = '00000000-0000-4000-8000-00000000000b';
const shell = (selected: string | null): CorpusShell => ({
  domains: [
    { domainRef: A, name: '领域甲', status: 'active', targetCount: 2, sampleCount: 5 },
    { domainRef: B, name: '领域乙', status: 'active', targetCount: 1, sampleCount: 3 },
  ],
  selectedDomain: selected,
  collectionState: '观察中',
});

function renderFrame(value: CorpusShell | null) {
  return render(
    <MemoryRouter>
      <CorpusFrame
        page="creators"
        pageLabel="创作者"
        action="/corpus/creators"
        shell={value}
        meta={<span>读取于 2026-10-15 14:30</span>}
        foot="作者来自当前领域已有作品"
      >
        <p>页面内容</p>
      </CorpusFrame>
    </MemoryRouter>,
  );
}

describe('语料页面框架（三页共用）', () => {
  it('页头 + 侧栏 + 内容区；页面标题给读屏，文档标题带「语料」', () => {
    renderFrame(shell(A));
    expect(screen.getByRole('heading', { level: 1, name: '创作者' })).toHaveClass('v7-sr-only');
    expect(screen.getByRole('main')).toHaveAccessibleName('创作者');
    expect(screen.getByText('页面内容')).toBeInTheDocument();
    expect(document.title).toBe('创作者 · 语料');
    expect(screen.getByRole('navigation', { name: '一级导航' })).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: '语料导航' })).toHaveTextContent('作者来自当前领域已有作品');
    expect(screen.getByText('读取于 2026-10-15 14:30')).toBeInTheDocument();
  });

  it('面包屑里是领域选择器；侧栏每一项带着已确认的领域', () => {
    const { container } = renderFrame(shell(A));
    const crumb = container.querySelector('.v7-context-crumb') as HTMLElement;
    expect(within(crumb).getByLabelText('切换当前观察领域')).toHaveTextContent('领域甲');
    expect(crumb.querySelector('b')).toHaveTextContent('创作者');
    expect(screen.getAllByRole('link', { name: /证据库|评论研究|创作者/ }).map(link => link.getAttribute('href'))).toEqual(
      expect.arrayContaining([`/corpus/evidence?domain=${A}`, `/corpus/comments?domain=${A}`, `/corpus/creators?domain=${A}`]),
    );
  });

  it('外壳还没读到（读取中或读取失败）：没有选择器，也不出现空的面包屑段，侧栏是裸路径', () => {
    const { container } = renderFrame(null);
    const crumb = container.querySelector('.v7-context-crumb') as HTMLElement;
    expect(crumb.textContent?.replace(/\s+/g, ' ').trim()).toBe('语料 / 创作者');
    expect(crumb.querySelectorAll('.v7-slash')).toHaveLength(1);
    expect(screen.queryByLabelText('切换当前观察领域')).toBeNull();
    expect(screen.getAllByRole('link', { name: /证据库/ })[0]).toHaveAttribute('href', '/corpus/evidence');
  });

  it('未选领域时选择器显示「选择领域」，侧栏不传入虚假的读取范围', () => {
    renderFrame(shell(null));
    expect(screen.getByLabelText('切换当前观察领域')).toHaveTextContent('选择领域');
    expect(screen.getAllByRole('link', { name: /证据库/ })[0]).toHaveAttribute('href', '/corpus/evidence');
  });

  it('证据库页的侧栏计数经框架传到徽标；其他页不传时隐藏', () => {
    const { container } = render(
      <MemoryRouter>
        <CorpusFrame page="evidence" pageLabel="证据库" action="/corpus/evidence" shell={shell(A)} meta={null} foot="" evidenceCount={{ count: 3, title: '本次读取 3 个作品' }}>
          <p>页面内容</p>
        </CorpusFrame>
      </MemoryRouter>,
    );
    expect(container.querySelector('#ev-rail-count')).toHaveTextContent('3');
    renderFrame(shell(A));
    expect(document.querySelectorAll('#ev-rail-count[hidden]')).toHaveLength(1);
  });

  it('语料是当前一级入口', () => {
    renderFrame(shell(A));
    expect(screen.getByRole('link', { name: /语料/ })).toHaveAttribute('aria-current', 'page');
  });
  it('创作者启用黑底材质；方格预览由 URL 选择，其他语料页面不改变', () => {
    const first = renderFrame(shell(A));
    const row = first.container.querySelector('.v7-context-row');
    expect(row).toHaveAttribute('data-context-texture', 'dark');
    expect(row?.querySelector('[data-pattern]')).toHaveAttribute('data-pattern', 'dot');
    first.unmount();

    window.history.replaceState({}, '', '/?breadcrumbPattern=square');
    try {
      const square = renderFrame(shell(A));
      expect(square.container.querySelector('[data-pattern]')).toHaveAttribute('data-pattern', 'square');
      square.unmount();
    } finally {
      window.history.replaceState({}, '', '/');
    }

    const evidence = render(
      <MemoryRouter>
        <CorpusFrame page="evidence" pageLabel="证据库" action="/corpus/evidence" shell={shell(A)} meta={null}>
          <p>页面内容</p>
        </CorpusFrame>
      </MemoryRouter>,
    );
    expect(evidence.container.querySelector('.v7-context-row')).not.toHaveAttribute('data-context-texture');
    expect(evidence.container.querySelector('[data-pattern]')).toBeNull();
  });

});
