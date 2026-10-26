import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DOMAIN, OTHER_DOMAIN, dossierJson, shellJson, workRef } from './evidenceFixtures';
import { assignLocation } from './navigation';
import { LIST_PATH, commentsPathOf, detailPathOf, ocrPathOf, pageRoutes, pageUrl, renderEvidencePage, stubViewport } from './pageKit';
import { createFetchMock, ok, type FetchKit } from './testKit';

vi.mock('./navigation', () => ({ assignLocation: vi.fn() }));

let kit: FetchKit;
beforeEach(() => {
  kit = createFetchMock();
  kit.install();
  sessionStorage.clear();
  localStorage.clear();
  stubViewport();
});
afterEach(() => {
  vi.unstubAllGlobals();
});

const aside = () => screen.getByRole('complementary', { name: /^(合成作品 \d+|请选择)/ });
const rowButton = (n: number) => screen.getByRole('button', { name: `合成作品 ${n}` });
const selectionKey = `collection-selected:${DOMAIN}`;
const bench = () => document.querySelector<HTMLElement>('[data-inspector]')!;

async function opened(url?: string) {
  const view = renderEvidencePage(url);
  await screen.findByRole('button', { name: '合成作品 1' });
  await view.user.click(rowButton(1));
  await waitFor(() => expect(view.location()).toContain('work='));
  return view;
}

describe('勾选与批量动作', () => {
  it('只有选择态才显示勾选框；勾选本页再次点击取消，存储仍用稳定引用', async () => {
    pageRoutes(kit);
    const view = await opened();
    expect(screen.queryAllByRole('checkbox', { name: /^勾选作品：/ })).toHaveLength(0);
    await view.user.click(screen.getByRole('button', { name: '选择' }));
    await view.user.click(screen.getByRole('button', { name: '勾选本页' }));
    expect(screen.getByRole('group', { name: '批量操作' })).toHaveTextContent('已选 3 篇');
    expect(JSON.parse(sessionStorage.getItem(selectionKey) ?? 'null')).toEqual([workRef(1), workRef(2), workRef(3)]);
    expect(screen.getAllByRole('checkbox', { name: /^勾选作品：/ }).every(box => (box as HTMLInputElement).checked)).toBe(true);
    await view.user.click(screen.getByRole('button', { name: '取消本页' }));
    expect(screen.getByRole('group', { name: '批量操作' })).toHaveTextContent('已选 0 篇');
    await view.user.click(screen.getByRole('button', { name: '勾选本页' }));

    view.unmount();
    pageRoutes(kit);
    renderEvidencePage();
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(screen.queryByRole('group', { name: '批量操作' })).toBeNull();
    const resume = screen.getByRole('button', { name: '选择' });
    await (await import('@testing-library/user-event')).default.setup().click(resume);
    expect(screen.getByRole('group', { name: '批量操作' })).toHaveTextContent('已选 3 篇');
  });

  it('取消勾选同步到存储；不同领域是另一份集合', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '选择' }));
    const boxes = screen.getAllByRole('checkbox', { name: /^勾选作品：/ });
    await view.user.click(boxes[0]!);
    await view.user.click(boxes[1]!);
    await view.user.click(boxes[0]!);
    expect(JSON.parse(sessionStorage.getItem(selectionKey) ?? 'null')).toEqual([workRef(2)]);

    view.unmount();
    pageRoutes(kit, { shell: ok(shellJson({ selectedDomain: OTHER_DOMAIN })) });
    renderEvidencePage(pageUrl(`domain=${OTHER_DOMAIN}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(screen.queryByRole('group', { name: '批量操作' })).toBeNull();
    // A new browsing visit does not automatically re-enter the bulk-selection mode.
  });

  it('「评论深采」把集合交给采集页：键名、数组格式与跳转形状保持（DR-6）', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '选择' }));
    expect(screen.getByRole('button', { name: '评论深采' })).toBeDisabled();
    await view.user.click(screen.getByRole('button', { name: '勾选本页' }));
    await view.user.click(screen.getByRole('button', { name: '评论深采' }));
    const here = `/corpus/evidence?domain=${DOMAIN}&work=${workRef(1)}`;
    expect(assignLocation).toHaveBeenCalledTimes(1);
    expect(assignLocation).toHaveBeenCalledWith(`/collection/tasks?domain=${DOMAIN}&selected=1&returnTo=${encodeURIComponent(here)}`);
    expect(JSON.parse(sessionStorage.getItem(selectionKey) ?? 'null')).toHaveLength(3);
  });

  it('超过 200 篇就地说明，两个批量按钮都不可用', async () => {
    sessionStorage.setItem(selectionKey, JSON.stringify(Array.from({ length: 201 }, (_, index) => workRef(1000 + index))));
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '选择' }));
    expect(screen.getByRole('group', { name: '批量操作' })).toHaveTextContent('已选 201 篇');
    expect(screen.getByText('最多 200 篇，请减少勾选')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '评论深采' })).toBeDisabled();
  });

  it('已移除旧图片整理与续跑入口', async () => {
    pageRoutes(kit); const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '选择' }));
    await view.user.click(screen.getByRole('button', { name: '勾选本页' }));
    expect(screen.queryByRole('button', { name: '整理图片文字' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '继续上次整理' })).not.toBeInTheDocument();
  });
});

describe('检查器外壳', () => {
  it('三个标签，默认「作品」；点击研究底稿替换历史并写入地址', async () => {
    pageRoutes(kit);
    const view = await opened();
    expect(screen.getAllByRole('tab').map(tab => tab.textContent)).toEqual(['作品', '研究底稿', '采集记录']);
    expect(screen.getByRole('tab', { name: '作品' })).toHaveAttribute('aria-selected', 'true');
    await view.user.click(screen.getByRole('tab', { name: '研究底稿' }));
    expect(view.location()).toBe(`/corpus/evidence?domain=${DOMAIN}&work=${workRef(1)}&tab=dossier`);
    expect(view.router.state.historyAction).toBe('REPLACE');
    expect(screen.getByRole('tab', { name: '研究底稿' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tab', { name: '作品' })).toHaveAttribute('tabindex', '-1');
  });

  it('← → Home End 在标签间移动并聚焦', async () => {
    pageRoutes(kit);
    const view = await opened();
    screen.getByRole('tab', { name: '作品' }).focus();
    await view.user.keyboard('{ArrowRight}');
    expect(screen.getByRole('tab', { name: '研究底稿' })).toHaveFocus();
    await view.user.keyboard('{End}');
    expect(screen.getByRole('tab', { name: '采集记录' })).toHaveFocus();
    await view.user.keyboard('{ArrowRight}');
    expect(screen.getByRole('tab', { name: '作品' })).toHaveFocus();
    await view.user.keyboard('{ArrowLeft}');
    expect(screen.getByRole('tab', { name: '采集记录' })).toHaveFocus();
    await view.user.keyboard('{Home}');
    expect(screen.getByRole('tab', { name: '作品' })).toHaveFocus();
    expect(view.location()).toBe(`/corpus/evidence?domain=${DOMAIN}&work=${workRef(1)}`);
  });

  it('作品正文与研究底稿共享同一 canonical 读取，附加元数据按需读取', async () => {
    pageRoutes(kit);
    kit.route('GET', `${detailPathOf(1)}/research-dossier`, ok(dossierJson(1)));
    const view = await opened();
    await waitFor(() => expect(kit.of('GET', `${detailPathOf(1)}/research-dossier`)).toHaveLength(1));
    await view.user.click(screen.getByRole('tab', { name: '研究底稿' }));
    await waitFor(() => expect(kit.of('GET', `${detailPathOf(1)}/research-dossier`)).toHaveLength(1));
    expect(kit.of('GET', `${detailPathOf(1)}/research-dossier`)[0]?.search.get('domain')).toBe(DOMAIN);
    expect(kit.of('GET', `${detailPathOf(1)}/research-dossier/metadata`)).toHaveLength(0);
  });

  it('`tab=dossier` 的深链直接读取底稿', async () => {
    pageRoutes(kit);
    kit.route('GET', `${detailPathOf(1)}/research-dossier`, ok(dossierJson(1)));
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&tab=dossier&work=${workRef(1)}`));
    await waitFor(() => expect(kit.of('GET', `${detailPathOf(1)}/research-dossier`)).toHaveLength(1));
    expect(screen.getByRole('tab', { name: '研究底稿' })).toHaveAttribute('aria-selected', 'true');
  });

  it('宽度：选了就记入本机偏好并写入地址（替换历史）；没有 panel 参数时取偏好，地址优先', async () => {
    pageRoutes(kit);
    const view = await opened();
    expect(screen.getByRole('button', { name: '标准' })).toHaveAttribute('aria-pressed', 'true');
    await view.user.click(screen.getByRole('button', { name: '加宽' }));
    expect(localStorage.getItem('linggan.evidence.inspectorWidth')).toBe('wide');
    expect(view.location()).toContain('panel=wide');
    expect(view.router.state.historyAction).toBe('REPLACE');
    expect(bench()).toHaveAttribute('data-inspector', 'wide');

    view.unmount();
    pageRoutes(kit);
    const again = await opened();
    expect(bench()).toHaveAttribute('data-inspector', 'wide');
    again.unmount();
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&panel=focus&work=${workRef(1)}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(bench()).toHaveAttribute('data-inspector', 'focus');
    expect(localStorage.getItem('linggan.evidence.inspectorWidth')).toBe('wide');
  });

  it('关闭检查器：结果区占满、焦点回到所选行；再点选作品自动重开', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '关闭检查器' }));
    expect(view.location()).toContain('panel=closed');
    expect(view.router.state.historyAction).toBe('REPLACE');
    expect(bench()).toHaveAttribute('data-inspector', 'closed');
    expect(rowButton(1)).toHaveFocus();

    await view.user.click(rowButton(2));
    expect(view.location()).toBe(`/corpus/evidence?domain=${DOMAIN}&work=${workRef(2)}`);
    expect(bench()).toHaveAttribute('data-inspector', 'normal');
    // 点选是新增历史：后退回到「已关闭」的那一步。
    await act(() => view.router.navigate(-1));
    expect(view.location()).toContain('panel=closed');
    expect(bench()).toHaveAttribute('data-inspector', 'closed');
  });

  it('重开时回到关闭前的宽度', async () => {
    pageRoutes(kit);
    const view = await opened(pageUrl(`domain=${DOMAIN}&panel=wide`));
    await view.user.click(screen.getByRole('button', { name: '关闭检查器' }));
    await view.user.click(rowButton(2));
    expect(bench()).toHaveAttribute('data-inspector', 'wide');
    expect(view.location()).toBe(`/corpus/evidence?domain=${DOMAIN}&work=${workRef(2)}&panel=wide`);
  });
});

describe('抽屉模式（≤1180px）', () => {
  beforeEach(() => {
    stubViewport({ drawer: true });
    Element.prototype.scrollIntoView = vi.fn();
  });
  afterEach(() => {
    // @ts-expect-error jsdom 本来就没有它；还原成没有
    delete Element.prototype.scrollIntoView;
  });

  it('首次浏览不展开抽屉；点选才展开；宽度控件与「×」不存在', async () => {
    pageRoutes(kit);
    const view = renderEvidencePage();
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(aside()).toHaveAttribute('inert');
    expect(screen.queryByRole('button', { name: '关闭检查器' })).not.toBeInTheDocument();
    expect(screen.queryByRole('group', { name: '检查器宽度' })).not.toBeInTheDocument();
    await view.user.click(rowButton(2));
    expect(aside()).not.toHaveAttribute('inert');
  });

  it('Esc 关闭抽屉并把焦点还给所选行', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(rowButton(2));
    expect(aside()).not.toHaveAttribute('inert');
    await view.user.keyboard('{Escape}');
    expect(aside()).toHaveAttribute('inert');
    expect(rowButton(2)).toHaveFocus();
  });

  it('「← 返回当前作品」收起抽屉，把所选行滚入视野并聚焦', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(rowButton(3));
    await view.user.click(screen.getByRole('button', { name: '← 返回当前作品' }));
    expect(aside()).toHaveAttribute('inert');
    expect(Element.prototype.scrollIntoView).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(rowButton(3)).toHaveFocus());
  });

  it('只读抽屉打开后定位返回控件，外部点击关闭并回到当前作品', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(rowButton(2));
    expect(screen.getByRole('button', { name: '← 返回当前作品' })).toHaveFocus();
    await view.user.click(document.body);
    expect(aside()).toHaveAttribute('inert');
    expect(rowButton(2)).toHaveFocus();
  });

  it('从抽屉内部拖到外部不关闭；嵌套模态存在时不处理外部点击和Esc', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(rowButton(2));
    fireEvent.pointerDown(screen.getByRole('button', { name: '← 返回当前作品' }));
    fireEvent.pointerUp(document.body);
    expect(aside()).not.toHaveAttribute('inert');
    const nested = document.createElement('dialog'); nested.setAttribute('open', ''); document.body.append(nested);
    await view.user.click(document.body);
    await view.user.keyboard('{Escape}');
    expect(aside()).not.toHaveAttribute('inert');
    nested.remove();
    await view.user.keyboard('{Escape}');
    expect(aside()).toHaveAttribute('inert');
  });

  it('地址直达一件作品：抽屉展开（旧页的「从地址还原」）', async () => {
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&work=${workRef(2)}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    await waitFor(() => expect(aside()).not.toHaveAttribute('inert'));
  });

  it('没有领域时带着作品进来，在对话框里选定领域之后，抽屉展开并定位那件作品（A12）', async () => {
    pageRoutes(kit, { shell: call => ({ status: 200, body: shellJson({ selectedDomain: call.search.get('domain') }) }) });
    const view = renderEvidencePage(pageUrl(`work=${workRef(2)}`));
    const dialog = await screen.findByRole('dialog', { name: '选择研究领域' });
    expect(screen.getByRole('complementary', { name: /^请选择/ })).toHaveAttribute('inert');
    await view.user.click(within(dialog).getByRole('link', { name: /合成领域一/ }));
    expect(view.location()).toBe(pageUrl(`domain=${DOMAIN}&work=${workRef(2)}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    await waitFor(() => expect(aside()).not.toHaveAttribute('inert'));
    expect(within(aside()).getByRole('heading', { level: 2, hidden: true, name: '合成作品 2' })).toBeInTheDocument();
  });

  it('后退到一件作品：抽屉展开', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(rowButton(2));
    await view.user.keyboard('{Escape}');
    await view.user.click(rowButton(3));
    await view.user.keyboard('{Escape}');
    expect(aside()).toHaveAttribute('inert');
    await act(() => view.router.navigate(-1));
    await waitFor(() => expect(aside()).not.toHaveAttribute('inert'));
    expect(view.location()).toContain(workRef(2));
  });
});

describe('敏感面', () => {
  const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;
  const activePanel = () => aside().querySelector<HTMLElement>('[role="tabpanel"]:not([hidden])')!;

  /** 普通层的文字：去掉折叠着的「技术事实」；`withoutCode` 再去掉技术旁注（<code>）。 */
  function ordinaryText(root: HTMLElement, withoutCode = false): string {
    const copy = root.cloneNode(true) as HTMLElement;
    copy.querySelectorAll(withoutCode ? 'details:not([open]), code' : 'details:not([open])').forEach(node => node.remove());
    return copy.textContent ?? '';
  }

  it('概览的普通层不出现 UUID、外部编号；英文状态码只作为技术旁注（<code>）出现', async () => {
    pageRoutes(kit);
    await opened();
    expect(ordinaryText(activePanel())).not.toMatch(UUID);
    expect(ordinaryText(activePanel())).not.toMatch(/ext-\d|authorExternalId|contentExternalId|externalId/);
    expect(ordinaryText(activePanel(), true)).not.toMatch(/\b[A-Z]{2,}(?:_[A-Z]+)+\b|\b(?:UNKNOWN|SEARCHABLE|NOT_REQUESTED)\b/);
  });

  it('引用与执行血缘只在「来源轨迹」这一技术层里', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('tab', { name: '采集记录' }));
    expect(activePanel().textContent).toMatch(UUID);
    expect(screen.getByRole('tab', { name: '采集记录' })).toHaveAttribute('aria-selected', 'true');
  });

  it('只写宽度偏好与选择引用，且值里没有作品内容', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '加宽' }));
    await view.user.click(screen.getByRole('button', { name: '选择' }));
    await view.user.click(screen.getByRole('button', { name: '勾选本页' }));
    expect(Object.keys(localStorage)).toEqual(['linggan.evidence.inspectorWidth']);
    expect(Object.keys(sessionStorage).sort()).toEqual([selectionKey]);
    for (const store of [localStorage, sessionStorage]) {
      for (const key of Object.keys(store)) expect(store.getItem(key) ?? '').not.toMatch(/合成|先做两分钟/);
    }
  });

  // Mog 2026-10-23：评论直接展示——打开作品即读评论第一页（只读、只读这一篇，仍不写任何存储）。
  it('页面打开时读取选中作品的 canonical 正文、识别状态与评论第一页，不发起识别或读取附件', async () => {
    pageRoutes(kit);
    await opened();
    await screen.findByRole('tab', { name: '作品' });
    await waitFor(() => expect(kit.of('GET', commentsPathOf(1))).toHaveLength(1));
    const paths = [...new Set(kit.calls.map(call => call.path))].sort();
    expect(paths).toEqual(['/api/local/corpus/shell', LIST_PATH, detailPathOf(1), ocrPathOf(1), `${detailPathOf(1)}/research-dossier`, commentsPathOf(1)].sort());
    expect(kit.calls.every(call => call.method === 'GET')).toBe(true);
  });
});
