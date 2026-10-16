import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { CorpusSideNav } from './CorpusSideNav';

const D = '00000000-0000-4000-8000-000000000001';

describe('语料侧栏', () => {
  it('三项按序号排列，当前项 aria-current，底部说明本页数据范围', () => {
    render(<CorpusSideNav active="creators" domain={null} foot="作者来自当前领域已有作品" />);
    const nav = screen.getByRole('complementary', { name: '语料导航' });
    const links = screen.getAllByRole('link');
    expect(links.map(link => link.textContent)).toEqual(['01证据库', '02评论研究', '03创作者']);
    expect(links.map(link => link.getAttribute('aria-current'))).toEqual([null, null, 'page']);
    expect(nav).toHaveTextContent('作者来自当前领域已有作品');
  });

  it('没有选定领域时链接是裸路径，不传入虚假的读取范围', () => {
    render(<CorpusSideNav active="creators" domain={null} foot="" />);
    expect(screen.getAllByRole('link').map(link => link.getAttribute('href'))).toEqual(['/corpus/evidence', '/corpus/comments', '/corpus/creators']);
  });

  it('选定领域后每一项都带上它（F09：领域在三页之间不丢），包括当前页', () => {
    render(<CorpusSideNav active="creators" domain={D} foot="" />);
    expect(screen.getAllByRole('link').map(link => link.getAttribute('href'))).toEqual([
      `/corpus/evidence?domain=${D}`,
      `/corpus/comments?domain=${D}`,
      `/corpus/creators?domain=${D}`,
    ]);
  });

  it('证据库计数徽标位在，且保持隐藏（由证据库页填写）', () => {
    const { container } = render(<CorpusSideNav active="creators" domain={null} foot="" />);
    const badge = container.querySelector('#ev-rail-count');
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveAttribute('hidden');
    expect(badge?.closest('a')).toHaveAttribute('href', '/corpus/evidence');
  });
});
