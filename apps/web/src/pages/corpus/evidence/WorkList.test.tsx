import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { parseWorkList } from './evidenceDecode';
import { listItemJson, listJson, mediaHandle, workRef } from './evidenceFixtures';
import type { LayoutKey } from './evidenceQuery';
import type { WorkItem } from './evidenceTypes';
import { WorkList } from './WorkList';

function items(...specs: Parameters<typeof listItemJson>[]): WorkItem[] {
  return parseWorkList(listJson(specs.map(([n, options]) => listItemJson(n, options)))).items;
}

function renderList(overrides: Partial<Parameters<typeof WorkList>[0]> = {}, list: WorkItem[] = items([1], [2], [3])) {
  const onSelect = vi.fn();
  const onCheck = vi.fn();
  const props = { items: list, layout: 'research' as LayoutKey, selectedRef: workRef(2), checked: new Set<string>(), canCheck: false, onSelect, onCheck, ...overrides };
  const view = render(<WorkList {...props} />);
  return { ...view, onSelect, onCheck, user: userEvent.setup() };
}

const targets = () => [...document.querySelectorAll<HTMLElement>('[data-select-target]')];

describe('研读排版', () => {
  it('每行是真列表项：标题按钮、作者、原声、互动（图标有中文名）、材料完整度、最近观察', () => {
    renderList({}, items([1, { media: { images: 1 } }]));
    const list = screen.getByRole('list', { name: '作品材料集合' });
    const row = within(list).getAllByRole('listitem')[0]!;
    expect(within(row).getByRole('heading', { level: 2 })).toHaveTextContent('合成作品 1');
    expect(within(row).getByRole('button', { name: '合成作品 1' })).toBeInTheDocument();
    expect(within(row).getByText('合成作者 1')).toBeInTheDocument();
    expect(within(row).getByText('2026-10-13')).toBeInTheDocument();
    expect(row.querySelector('q')).toHaveTextContent('合成正文 1：先做两分钟，再决定要不要继续。');
    for (const name of ['点赞', '评论', '收藏', '分享']) expect(within(row).getByRole('img', { name })).toBeInTheDocument();
    expect(within(row).getByText('1,201')).toBeInTheDocument();
    expect(within(row).getByRole('img', { name: /^材料完整度：正文 可检索；评论 .*转录 本作品不适用，不计入分母$/ })).toBeInTheDocument();
    expect(within(row).getByText('最近观察 2026-10-10 17:00')).toBeInTheDocument();
    expect(row).toHaveAttribute('data-selecting', 'false');
  });

  it('未知不是 0：没有互动数字写「未知」；没有标题写「标题当前未知」；没有原声写明确空态', () => {
    renderList({}, items([1, { likes: null, comments: null, collects: null, shares: null, title: null, fragment: null }]));
    expect(screen.getAllByText('未知')).toHaveLength(4);
    expect(screen.getByRole('button', { name: '标题当前未知' })).toBeInTheDocument();
    expect(screen.getByText('无原声')).toBeInTheDocument();
    expect(screen.getByText('尚未取得可引用的原文、评论或图片文字')).toBeInTheDocument();
  });

  it('封面 OCR 标题与参照材料的标记；参照用中文「参照」，不写英文装饰', () => {
    renderList({}, items([1, { titleSource: 'cover_ocr', role: 'reference' }]));
    expect(screen.getByRole('button', { name: '合成作品 1' })).toBeInTheDocument();
    expect(screen.queryByText(/reference/i)).toBeNull();
  });

  it('选中行用 aria-current；漫游焦点：只有选中行的选择控件是 tab 停靠点', () => {
    renderList();
    const [first, second, third] = targets();
    expect(first).toHaveAttribute('tabindex', '-1');
    expect(second).toHaveAttribute('tabindex', '0');
    expect(second).toHaveAttribute('aria-current', 'true');
    expect(third).toHaveAttribute('tabindex', '-1');
    expect(first).not.toHaveAttribute('aria-current');
  });

  it('没有选中项时第一行是 tab 停靠点', () => {
    renderList({ selectedRef: null });
    expect(targets().map(target => target.getAttribute('tabindex'))).toEqual(['0', '-1', '-1']);
  });

  it('鼠标点整行、键盘 Enter／Space 都选中（用户操作）', async () => {
    const { user, onSelect } = renderList();
    await user.click(screen.getByText('合成正文 1：先做两分钟，再决定要不要继续。'));
    expect(onSelect).toHaveBeenLastCalledWith(expect.objectContaining({ ref: workRef(1) }), 'user');
    targets()[2]!.focus();
    await user.keyboard('{Enter}');
    expect(onSelect).toHaveBeenLastCalledWith(expect.objectContaining({ ref: workRef(3) }), 'user');
    await user.keyboard(' ');
    expect(onSelect).toHaveBeenCalledTimes(3);
  });

  it('方向键 ↑↓ Home End 在行间移动并选中、聚焦（自动选择，不新增历史），到头循环', async () => {
    const { user, onSelect } = renderList();
    targets()[1]!.focus();
    await user.keyboard('{ArrowDown}');
    expect(onSelect).toHaveBeenLastCalledWith(expect.objectContaining({ ref: workRef(3) }), 'auto');
    expect(document.activeElement).toBe(targets()[2]);
    await user.keyboard('{ArrowDown}');
    expect(onSelect).toHaveBeenLastCalledWith(expect.objectContaining({ ref: workRef(1) }), 'auto');
    await user.keyboard('{ArrowUp}');
    expect(onSelect).toHaveBeenLastCalledWith(expect.objectContaining({ ref: workRef(3) }), 'auto');
    await user.keyboard('{Home}');
    expect(onSelect).toHaveBeenLastCalledWith(expect.objectContaining({ ref: workRef(1) }), 'auto');
    await user.keyboard('{End}');
    expect(onSelect).toHaveBeenLastCalledWith(expect.objectContaining({ ref: workRef(3) }), 'auto');
  });

  it('勾选框是独立控件：点它只勾选，不选中行（F07）；名称「勾选作品用于批量操作」', async () => {
    const { user, onSelect, onCheck } = renderList({ canCheck:true, checked: new Set([workRef(1)]) });
    const boxes = screen.getAllByRole('checkbox', { name: /^勾选作品：/ });
    expect(boxes).toHaveLength(3);
    expect(boxes[0]).toBeChecked();
    expect(boxes[1]).not.toBeChecked();
    await user.click(boxes[1]!);
    expect(onCheck).toHaveBeenCalledWith(workRef(2), true);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('浏览态没有勾选框', () => {
    renderList({ canCheck: false });
    expect(screen.queryAllByRole('checkbox')).toHaveLength(0);
  });

  it('预览只用受控本地封面；没有封面写状态词', () => {
    renderList({}, items([1, { media: { cover: 'available' } }], [2]));
    expect(document.querySelector(`img[src="${mediaHandle(10)}"]`)).not.toBeNull();
    expect(screen.getByRole('img', { name: '封面尚未形成观察' })).toBeInTheDocument();
    expect(document.querySelectorAll('img[src^="http"]')).toHaveLength(0);
  });
});

describe('表格排版', () => {
  it('真表格：标题、列头；行内只有数字但读屏仍读出名称；选中用 aria-current', async () => {
    const { user, onSelect } = renderList({ layout: 'table' });
    const table = screen.getByRole('table', { name: '作品材料集合' });
    expect(within(table).getAllByRole('columnheader')).toHaveLength(9);
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows).toHaveLength(3);
    expect(within(rows[0]!).getByText('点赞：')).toHaveClass('v7-sr-only');
    expect(within(rows[0]!).getByRole('button', { name: '合成作品 1' })).toBeInTheDocument();
    expect(within(rows[1]!).getByRole('button', { name: '合成作品 2' })).toHaveAttribute('aria-current', 'true');
    await user.click(within(rows[2]!).getByText('合成作者 3'));
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ ref: workRef(3) }), 'user');
  });

  it('列头保留四个互动图标，且有文字名', () => {
    renderList({ layout: 'table' });
    for (const name of ['点赞', '评论', '收藏', '分享']) {
      expect(screen.getByRole('columnheader', { name })).toBeInTheDocument();
    }
  });
});

describe('封面排版（翻转卡）', () => {
  function stubHover(hoverNone: boolean) {
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: hoverNone && query === '(hover: none)', addEventListener: () => undefined, removeEventListener: () => undefined }));
  }

  it('每张卡：翻转面（role=button、名称「标题，查看封面」）＋铭牌上的选择按钮；序号补零', () => {
    renderList({ layout: 'cover' }, items([1, { media: { cover: 'available' } }], [2]));
    const flip = screen.getByRole('button', { name: '合成作品 1，查看封面' });
    expect(flip).toHaveAttribute('aria-pressed', 'false');
    expect(within(flip).getAllByText('001')).toHaveLength(2);
    expect(within(flip).getByText('图文 · 已观察')).toBeInTheDocument();
    expect(within(flip).getByText('查看封面 →')).toBeInTheDocument();
    expect(within(flip).getByText('3:4')).toBeInTheDocument();
    expect(within(flip).queryByText(/COVER/)).toBeNull();
    expect(screen.getAllByRole('button', { name: '合成作品 1' })).toHaveLength(1);
    expect(flip.querySelector(`img[src="${mediaHandle(10)}"]`)).not.toBeNull();
  });

  it('键盘 Enter／Space 翻转：名称与 aria-pressed 同步，且不触发行选择', async () => {
    const { user, onSelect } = renderList({ layout: 'cover' });
    const flip = screen.getByRole('button', { name: '合成作品 1，查看封面' });
    flip.focus();
    await user.keyboard('{Enter}');
    expect(screen.getByRole('button', { name: '合成作品 1，收起封面' })).toHaveAttribute('aria-pressed', 'true');
    await user.keyboard(' ');
    expect(screen.getByRole('button', { name: '合成作品 1，查看封面' })).toHaveAttribute('aria-pressed', 'false');
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('触屏（hover: none）点击卡面翻转且不触发行选择；支持悬停的设备点击卡面选中', () => {
    stubHover(true);
    const touch = renderList({ layout: 'cover' });
    fireEvent.click(screen.getByRole('button', { name: '合成作品 1，查看封面' }));
    expect(screen.getByRole('button', { name: '合成作品 1，收起封面' })).toBeInTheDocument();
    expect(touch.onSelect).not.toHaveBeenCalled();
    touch.unmount();
    stubHover(false);
    const mouse = renderList({ layout: 'cover' });
    fireEvent.click(screen.getByRole('button', { name: '合成作品 1，查看封面' }));
    expect(mouse.onSelect).toHaveBeenCalledWith(expect.objectContaining({ ref: workRef(1) }), 'user');
  });

  it('铭牌上的选择按钮给键盘与读屏：名称是标题，补充说明是五维状态（评论写保留数 / 平台数）', async () => {
    const { user, onSelect } = renderList({ layout: 'cover' }, items([1, { lanes: { comments: ['SEARCHABLE', 45] }, comments: 321 }]));
    const select = screen.getByRole('button', { name: '合成作品 1' });
    expect(select).not.toHaveAttribute('aria-current');
    select.focus();
    await user.keyboard('{Enter}');
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ ref: workRef(1) }), 'user');
    expect(select).toHaveAccessibleDescription(/评论 45 \/ 321/);
    expect(select).toHaveAccessibleDescription(/正文：可检索/);
  });

  it('铭牌：作者、发布日、互动、状态线、观察时刻（MM-DD HH:MM）', () => {
    renderList({ layout: 'cover' }, items([1]));
    const card = screen.getByRole('listitem');
    expect(within(card).getByText('合成作者 1')).toBeInTheDocument();
    expect(within(card).getByText('2026-10-13')).toBeInTheDocument();
    expect(within(card).getByText('10-10 17:00')).toBeInTheDocument();
    expect(within(card).getByText('可检索')).toBeInTheDocument();
  });
});

it('封面卡的受控图片也明确区分读取中、成功和失败，失败不伪装未取得', () => {
  renderList({ layout: 'cover' }, items([1, { media: { cover: 'available' } }]));
  const loading = screen.getByRole('button', { name: '合成作品 1，查看封面' }).querySelector('[data-media-state]')!;
  expect(loading).toHaveAttribute('data-media-state', 'loading');
  fireEvent.error(loading.querySelector('img')!);
  const failed = screen.getByRole('button', { name: '合成作品 1，查看封面' }).querySelector('[data-media-state]')!;
  expect(failed).toHaveAttribute('data-media-state', 'failed');
  expect(failed).toHaveTextContent('封面读取失败');
  expect(failed.querySelector('img')).toBeNull();
});
