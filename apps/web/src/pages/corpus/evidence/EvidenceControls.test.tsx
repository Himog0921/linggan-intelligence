import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { EvidenceControls } from './EvidenceControls';
import { DEFAULT_QUERY, type EvidenceQuery } from './evidenceQuery';
import { workRef } from './evidenceFixtures';

function setup(query: EvidenceQuery = DEFAULT_QUERY, hasDomain = true) {
  const onChange = vi.fn();
  const onSubmit = vi.fn();
  const onClearFilters = vi.fn();
  const view = render(<EvidenceControls query={query} hasDomain={hasDomain} onChange={onChange} onSubmit={onSubmit} onClearFilters={onClearFilters} />);
  const rerender = (next: EvidenceQuery) => view.rerender(<EvidenceControls query={next} hasDomain={hasDomain} onChange={onChange} onSubmit={onSubmit} onClearFilters={onClearFilters} />);
  return { ...view, rerender, onChange, onSubmit, onClearFilters, user: userEvent.setup() };
}

/** 取出 onChange 收到的更新函数，应用到给定查询上。 */
function applied(onChange: ReturnType<typeof vi.fn>, query: EvidenceQuery = DEFAULT_QUERY, call = 0): EvidenceQuery {
  const update = onChange.mock.calls[call]?.[0] as (current: EvidenceQuery) => EvidenceQuery;
  return update(query);
}

describe('检索', () => {
  it('只在提交（Enter 或「读取材料」）时读取，没有自动搜索', async () => {
    const { user, onSubmit } = setup();
    const box = screen.getByRole('searchbox', { name: '检索作品与证据' });
    expect(box).toHaveAttribute('placeholder', '标题、作者或原声片段……');
    await user.type(box, '两分钟');
    expect(onSubmit).not.toHaveBeenCalled();
    await user.keyboard('{Enter}');
    expect(onSubmit).toHaveBeenCalledWith('两分钟');
    await user.click(screen.getByRole('button', { name: '读取材料' }));
    expect(onSubmit).toHaveBeenCalledTimes(2);
  });

  it('地址里的检索词变了（后退、清空）：输入框跟着变，而不是留着旧文字', () => {
    const { rerender } = setup({ ...DEFAULT_QUERY, q: '旧词' });
    expect(screen.getByRole('searchbox')).toHaveValue('旧词');
    rerender({ ...DEFAULT_QUERY, q: '' });
    expect(screen.getByRole('searchbox')).toHaveValue('');
  });

  it('没有领域：检索、筛选、读取、系统视图全部禁用并写明原因，提交不读取；排版切换不依赖领域', async () => {
    const { user, onSubmit } = setup(DEFAULT_QUERY, false);
    const reason = '请先选择研究领域，再读取统一作品材料。';
    expect(screen.getByRole('searchbox')).toBeDisabled();
    expect(screen.getByRole('searchbox')).toHaveAttribute('title', reason);
    expect(screen.getByRole('button', { name: /筛选/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: '读取材料' })).toBeDisabled();
    for (const name of ['全部材料', '部分取得', '风险停止', '媒体已清理', '撤回或受限']) expect(screen.getByRole('button', { name })).toBeDisabled();
    expect(screen.getByRole('button', { name: '研读' })).toBeEnabled();
    await user.keyboard('{Enter}');
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

describe('筛选浮层', () => {
  it('三个统一选择器：材料通道、通道状态、媒体类型；DR-1 移除的排序与读取窗口不再出现', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: /筛选/ }));
    const panel = screen.getByRole('group', { name: '筛选条件' });
    expect(within(panel).getAllByRole('combobox').map(select => select.getAttribute('aria-label'))).toEqual(['材料通道', '通道状态', '媒体类型']);
    for (const [name, count] of [['材料通道', 10], ['通道状态', 15], ['媒体类型', 5]] as const) {
      await user.click(within(panel).getByRole('combobox', { name }));
      expect(within(screen.getByRole('listbox', { name })).getAllByRole('option')).toHaveLength(count);
      await user.keyboard('{Escape}');
    }
    expect(screen.queryByText('读取窗口')).toBeNull();
    expect(screen.queryByText(/近 7 天|近 30 天/)).toBeNull();
    expect(screen.queryByRole('combobox', { name: '结果排序' })).toBeNull();
    expect(screen.queryByText(/相关度|RELEVANCE/)).toBeNull();
    expect(screen.queryByText('旧 OCR 已停用')).toBeNull();
  });

  it('选一项：新增历史、重新读取（work 清空，让第一项自动选中）；系统视图保持', async () => {
    const { user, onChange } = setup({ ...DEFAULT_QUERY, view: 'risk', work: workRef(1) });
    await user.click(screen.getByRole('button', { name: /筛选/ }));
    await user.click(screen.getByRole('combobox', { name: '材料通道' }));
    await user.click(screen.getByRole('option', { name: '评论' }));
    const next = applied(onChange, { ...DEFAULT_QUERY, view: 'risk', work: workRef(1) });
    expect(next).toMatchObject({ lane: 'comments', view: 'risk', work: null });
    expect(onChange.mock.calls[0]?.[1]).toBeUndefined();
  });

  it('徽标数：与默认不同的下拉个数；展开状态用 aria-expanded', async () => {
    const { user } = setup({ ...DEFAULT_QUERY, lane: 'ocr', mediaKind: 'video', view: 'risk' });
    const toggle = screen.getByRole('button', { name: /筛选/ });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(within(toggle).getByLabelText('已选 2 项筛选')).toHaveTextContent('02');
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(toggle).toHaveAttribute('aria-controls', screen.getByRole('group', { name: '筛选条件' }).id);
  });

  it('Esc 关闭并把焦点还给筛选按钮；点击外部也关闭', async () => {
    const { user } = setup();
    const toggle = screen.getByRole('button', { name: /筛选/ });
    await user.click(toggle);
    await user.click(screen.getByRole('combobox', { name: '通道状态' }));
    await user.click(screen.getByRole('option', { name: '部分取得' }));
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('group', { name: '筛选条件' })).toBeNull();
    expect(document.activeElement).toBe(toggle);
    await user.click(toggle);
    expect(screen.getByRole('group', { name: '筛选条件' })).toBeInTheDocument();
    await user.click(document.body);
    expect(screen.queryByRole('group', { name: '筛选条件' })).toBeNull();
  });

  it('「清空筛选」交给页面处理（也要重新读取）', async () => {
    const { user, onClearFilters } = setup({ ...DEFAULT_QUERY, lane: 'ocr' });
    await user.click(screen.getByRole('button', { name: /筛选/ }));
    await user.click(screen.getByRole('button', { name: '清空筛选' }));
    expect(onClearFilters).toHaveBeenCalledTimes(1);
  });
});

describe('系统视图与排版', () => {
  it('五个系统视图：aria-pressed 随地址；点选新增历史并清空 work', async () => {
    const { user, onChange } = setup({ ...DEFAULT_QUERY, view: 'partial', work: workRef(4) });
    const group = screen.getByRole('group', { name: '按材料状态快速筛选' });
    expect(within(group).getAllByRole('button').map(button => button.textContent)).toEqual(['全部材料', '部分取得', '风险停止', '媒体已清理', '撤回或受限']);
    expect(within(group).getByRole('button', { name: '部分取得' })).toHaveAttribute('aria-pressed', 'true');
    expect(within(group).getByRole('button', { name: '全部材料' })).toHaveAttribute('aria-pressed', 'false');
    await user.click(within(group).getByRole('button', { name: '风险停止' }));
    expect(applied(onChange, { ...DEFAULT_QUERY, view: 'partial', work: workRef(4) })).toMatchObject({ view: 'risk', work: null });
  });

  it('三种排版：不清空 work（只重排已读到的集合，不重新读取）', async () => {
    const { user, onChange } = setup({ ...DEFAULT_QUERY, work: workRef(4) });
    const group = screen.getByRole('group', { name: '结果排版' });
    expect(within(group).getAllByRole('button').map(button => button.textContent)).toEqual(['研读', '表格', '封面']);
    await user.click(within(group).getByRole('button', { name: '封面' }));
    expect(applied(onChange, { ...DEFAULT_QUERY, work: workRef(4) })).toMatchObject({ layout: 'cover', work: workRef(4) });
  });

  it('DR-2：未接通的「我的视图 · 暂无已保存视图」占位已删除', () => {
    setup();
    expect(screen.queryByText(/我的视图|暂无已保存视图/)).toBeNull();
  });
});

describe('查询输入边界', () => {
  it('中文组词时Enter不提交；组词完成后仍由用户确认', async () => {
    const { user, onSubmit } = setup();
    const box = screen.getByRole('searchbox');
    fireEvent.compositionStart(box);
    await user.type(box, '家庭{Enter}');
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.compositionEnd(box);
    await user.keyboard('{Enter}');
    expect(onSubmit).toHaveBeenCalledWith('家庭');
  });
  it('清空检索同时提交空词并保持输入焦点', async () => {
    const { user, onSubmit } = setup({ ...DEFAULT_QUERY, q: '旧词' });
    await user.click(screen.getByRole('button', { name: '清空检索' }));
    expect(onSubmit).toHaveBeenCalledWith('');
    expect(screen.getByRole('searchbox')).toHaveValue('');
    expect(screen.getByRole('searchbox')).toHaveFocus();
  });
  it('斜线定位FIND，不吞掉输入框里的斜线', async () => {
    const { user } = setup();
    await user.keyboard('/');
    expect(screen.getByRole('searchbox')).toHaveFocus();
    await user.keyboard('/');
    expect(screen.getByRole('searchbox')).toHaveValue('/');
  });
  it('选择器Esc只收起选项；再次Esc才收起筛选面板', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const combo = screen.getByRole('combobox', { name: '材料通道' });
    await user.click(combo);
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('listbox')).toBeNull();
    expect(screen.getByRole('group', { name: '筛选条件' })).toBeInTheDocument();
    expect(combo).toHaveFocus();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('group', { name: '筛选条件' })).toBeNull();
  });
});

it('切换领域不保留上个领域尚未提交的词，即使两个领域地址q都为空', async () => {
  const onSubmit = vi.fn(); const onChange = vi.fn(); const onClearFilters = vi.fn();
  const view = render(<EvidenceControls scope="one" query={DEFAULT_QUERY} hasDomain onSubmit={onSubmit} onChange={onChange} onClearFilters={onClearFilters} />);
  await userEvent.type(screen.getByRole('searchbox'), '旧领域草稿');
  view.rerender(<EvidenceControls scope="two" query={DEFAULT_QUERY} hasDomain onSubmit={onSubmit} onChange={onChange} onClearFilters={onClearFilters} />);
  expect(screen.getByRole('searchbox')).toHaveValue('');
  expect(onSubmit).not.toHaveBeenCalled();
});
