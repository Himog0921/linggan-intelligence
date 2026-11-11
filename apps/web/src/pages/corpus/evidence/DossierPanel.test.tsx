import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DossierPanel } from './DossierPanel';
import { parseDossierPayload } from './dossierDecode';
import { dossierKey } from './evidenceKeys';
import * as dossierApi from './dossierApi';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import { createFetchMock, fail, ok, renderWithProviders, type FetchKit } from './testKit';
const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
const understandingPath = '/api/local/content-understanding/latest';
let kit: FetchKit;
beforeEach(() => {
  kit = createFetchMock();
  kit.route('GET', understandingPath, ok({ contract: 'content-understanding.read.v1', workRef: workRef(1), analysis: null }));
  kit.install(); HTMLElement.prototype.scrollTo = vi.fn(); localStorage.clear();
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });
function mount(active = true) { return renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active={active} />); }
const source = (state: string, units = 0, readings = 0) => ({ state, units, readings });
const completeCoverage = { imageInventory: { basis: 'platform_image_list', expectedPages: 0, acquiredPages: 0, processedPages: 0, missingPages: [], unknownOrder: false }, sources: { body: source('known_empty'), images: source('not_applicable'), audio: source('available', 1, 1), videoFrames: source('not_applicable') }, issues: [] };

it.each([401,403,404,410])('刷新得到 %i 后撤下旧内容与导出入口', async status => {
  kit.route('GET', path, ok(dossierJson(1, { bodyText: '已有缓存但随后失去权限的正文' })), fail(status, 'source_restricted'));
  const view = mount(); await screen.findByText('已有缓存但随后失去权限的正文');
  await view.user.click(screen.getByRole('button', { name: '刷新底稿' })); await screen.findByText('研究底稿暂时不可读。');
  expect(screen.queryByText('已有缓存但随后失去权限的正文')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '导出' })).not.toBeInTheDocument();
});
it('临时服务故障保留并标记最后读到的版本', async () => {
  kit.route('GET', path, ok(dossierJson(1, { bodyText: '最后成功读取的完整正文' })), fail(503, 'temporarily_unavailable'));
  const view = mount(); await screen.findByText('最后成功读取的完整正文');
  await view.user.click(screen.getByRole('button', { name: '刷新底稿' })); await screen.findByText(/刷新未完成，当前保留上一次读到的版本/);
  expect(screen.getByText('最后成功读取的完整正文')).toBeVisible(); expect(screen.getByRole('button', { name: '导出' })).toBeEnabled();
});
it('未打开底稿不发请求，打开后不自动准备识读或读取来源附表', async () => {
  const inactive = mount(false); expect(kit.calls).toHaveLength(0); inactive.unmount();
  kit.route('GET', path, ok(dossierJson(1)));
  mount(); await screen.findByRole('button', { name: '导出' });
  await screen.findByText('这篇尚无当前材料的理解结果，以下保留原始材料。');
  expect(kit.calls.map(call => `${call.method} ${call.path}`)).toEqual([`GET ${path}`, `GET ${understandingPath}`]);
  for (const label of ['查看来源', '修正文字', '导出来源映射', '原文展开', '作者、时间与互动', '材料详情']) expect(screen.queryByText(label)).not.toBeInTheDocument();
});
it('多图沿 owner 编排连续阅读，保留标题来源及相同文字的不同来源', async () => {
  const materials = Array.from({ length: 12 }, (_, index) => dossierMaterialJson(index + 1, { blocks: [{ ref: `image-${index}`, kind: 'paragraph', text: index === 0 || index === 11 ? '真实重复' : `第 ${index + 1} 张完整文字` }] }));
  const raw = dossierJson(1, { materials, note: { displayTitle: '真实重复', displayTitleBasis: 'cover', titleBlockRefs: ['image-0'] }, readingView: { mainBlockRefs: materials.map((_, index) => `image-${index}`).reverse(), supplementBlockRefs: [] } });
  kit.route('GET', path, ok(raw));
  const view = mount(); await screen.findAllByText('真实重复');
  const content = screen.getByRole('article', { name: '连续阅读文档' });
  expect(within(content).getAllByText('真实重复')).toHaveLength(2);
  expect(content.querySelectorAll('[data-block-ref]')).toHaveLength(12);
  expect([...content.querySelectorAll('[data-block-ref]')].map(node => node.getAttribute('data-block-ref'))).toEqual(raw.dossier.readingView.sections.flatMap(section => section.groups.flatMap(group => group.blockRefs)));
  expect(screen.queryByRole('heading', { name: /^第 \d+ 页$/ })).not.toBeInTheDocument();
  expect(view.container.querySelector('img')).toBeNull();
  const notes = screen.getByText('材料说明').closest('details');
  expect(notes).not.toHaveAttribute('open');
  await view.user.click(screen.getByText('材料说明'));
  expect(notes).toHaveAttribute('open');
  expect(within(notes!).getByText('数据截至')).toBeVisible();
});
it('页面不展示核对说明，实际图片文字保留', async () => {
  const issue = { key: 'ocr_low_confidence_0', code: 'glyph', message: '数字需要对照原图核验', resolution: 'reading_edit' };
  kit.route('GET', path, ok(dossierJson(1, { materials: [1,2].map(n => dossierMaterialJson(n, { blocks: [{ ref: `paragraph-${n}`, kind: 'paragraph', text: `完整原文 ${n}`, issues: [issue] }], issues: [issue] })), coverage: { issues: [] } })));
  mount(); await screen.findByText('完整原文 1');
  expect(screen.queryByText(issue.message)).not.toBeInTheDocument();
  expect(screen.queryByText(/^核对说明/)).not.toBeInTheDocument();
  expect(screen.getByText('完整原文 2')).toBeVisible();
});
it('已确认缺页保留在数据，正文连续展示已有文字且不插入页码警示', async () => {
  const raw = dossierJson(1, { materials: [dossierMaterialJson(1, { blocks: [{ ref: 'before', kind: 'paragraph', text: '缺页之前' }] }), dossierMaterialJson(3, { blocks: [{ ref: 'after', kind: 'paragraph', text: '缺页之后' }] })], coverage: { imageInventory: { basis: 'platform_image_list', expectedPages: 3, acquiredPages: 2, processedPages: 2, missingPages: [2], unknownOrder: false } } });
  const section = raw.dossier.readingView.sections[0]!;
  section.gaps = [{ beforeGroupRef: section.groups[1]!.ref, missingPages: [2] }];
  kit.route('GET', path, ok(raw)); mount();
  await screen.findByText('缺页之前');
  expect(screen.queryByText(/第 2 页文字待补全/)).not.toBeInTheDocument();
  expect(raw.dossier.coverage.imageInventory.missingPages).toEqual([2]);
  expect(screen.getByText('缺页之前')).toBeVisible();
  expect(screen.getByText('缺页之后')).toBeVisible();
});
it('仅待处理样本保留真实缺失状态，不造口播、目录或原图入口', async () => {
  kit.route('GET', path, ok(dossierJson(1, { materials: [dossierMaterialJson(1, { blocks: [], reading: null, currentRevisionRef: null })], coverage: { ...completeCoverage, imageInventory: { basis: 'observed_set', expectedPages: null, acquiredPages: 1, processedPages: 0, missingPages: [], unknownOrder: true }, sources: { body: source('known_empty'), images: source('not_processed', 1), audio: source('not_applicable'), videoFrames: source('not_applicable') } } })));
  const view = mount();
  await screen.findByText('待识读');
  expect(screen.queryByText(/图片原始顺序未确认|原作总页数未知/)).not.toBeInTheDocument();
  expect(screen.getByText('尚无可用文字。')).toBeVisible();
  expect(screen.queryByText(/第 1 页文字待补全/)).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /章节导航/ })).not.toBeInTheDocument();
  expect(view.container.querySelector('[data-transcript-paragraph], img')).toBeNull();
  await screen.findByText('这篇尚无当前材料的理解结果，以下保留原始材料。');
  expect(kit.calls.map(call => `${call.method} ${call.path}`)).toEqual([`GET ${path}`, `GET ${understandingPath}`]);
});
it('5 分钟负载自然段全文保留，毫秒时码与专注弹窗不推造讲述人数', async () => {
  const paragraphs = Array.from({ length: 12 }, (_, index) => ({ ref: `speech-${index}`, kind: 'paragraph', speaker: index > 5 ? 'other' : 'author', text: `段落${index + 1}：${'需要完整阅读的真实长口播。'.repeat(12)}`, time: { startMs: index === 7 ? 173000 : index * 25000, endMs: (index + 1) * 25000 } }));
  kit.route('GET', path, ok(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', page: null, blocks: paragraphs })], coverage: completeCoverage })));
  const view = mount();
  const article = await screen.findByRole('article', { name: '连续阅读文档' });
  expect(article.querySelectorAll('[data-transcript-paragraph]')).toHaveLength(12);
  expect(article.textContent).toContain(paragraphs.at(-1)!.text);
  expect(screen.getByText('02:53')).toBeVisible();
  expect(screen.queryByText(/讲述者|说话人|共.*人/)).not.toBeInTheDocument();
  expect(screen.getAllByText('他人表述')).toHaveLength(1);
  await view.user.click(screen.getByRole('button', { name: '专注阅读' }));
  const dialog = screen.getByRole('dialog', { name: '视频口播 · 专注阅读' });
  expect(dialog.querySelectorAll('[data-transcript-paragraph]')).toHaveLength(12);
  const timeTrigger = within(dialog).getByRole('button', { name: /定位口播时间/ });
  await view.user.click(timeTrigger);
  const timeMenu = within(dialog).getByRole('listbox', { name: '口播时间' });
  expect(timeMenu).toHaveFocus();
  expect(within(timeMenu).getAllByRole('option')).toHaveLength(12);
  await view.user.keyboard('{Tab}');
  await waitFor(() => expect(within(dialog).queryByRole('listbox')).not.toBeInTheDocument());
  expect(document.activeElement).not.toBe(document.body);
  expect(within(dialog).getByLabelText('完整口播正文')).toHaveFocus();
  await view.user.click(timeTrigger);
  expect(within(dialog).getByRole('listbox', { name: '口播时间' })).toBeInTheDocument();
  await view.user.keyboard('{Home}{ArrowDown}{Enter}');
  await waitFor(() => expect(within(dialog).queryByRole('listbox')).not.toBeInTheDocument());
  expect(timeTrigger).toHaveTextContent('00:25');
  expect(document.activeElement?.getAttribute('data-reading-anchor')).toMatch(/speech-1$/);
  fireEvent.scroll(within(dialog).getByLabelText('完整口播正文'));
  await waitFor(() => expect(timeTrigger).toHaveTextContent('04:35'));
  await view.user.click(timeTrigger);
  expect(within(dialog).getByRole('listbox', { name: '口播时间' })).toBeInTheDocument();
  await view.user.keyboard('{Escape}');
  expect(within(dialog).queryByRole('listbox')).not.toBeInTheDocument();
  expect(dialog).toBeInTheDocument();
  expect(timeTrigger).toHaveFocus();
  await view.user.click(timeTrigger);
  expect(within(dialog).getByRole('listbox', { name: '口播时间' })).toBeInTheDocument();
  fireEvent(dialog, new Event('cancel', { bubbles: false, cancelable: true }));
  expect(within(dialog).queryByRole('listbox')).not.toBeInTheDocument();
  expect(screen.getByRole('dialog', { name: '视频口播 · 专注阅读' })).toBeInTheDocument();
  fireEvent(dialog, new Event('cancel', { bubbles: false, cancelable: true }));
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(screen.getByRole('button', { name: '专注阅读' })).toHaveFocus();
});
it('当前段落没有时间时触发器不冒充第一项，菜单仍可跳到真实时间', async () => {
  const paragraphs = [
    { ref: 'speech-0', kind: 'paragraph', text: '第一段有真实时间', time: { startMs: 0, endMs: 25000 } },
    { ref: 'speech-1', kind: 'paragraph', text: '第二段也有真实时间', time: { startMs: 25000, endMs: 50000 } },
    { ref: 'speech-2', kind: 'paragraph', text: '这一段没有取得时码' },
  ];
  kit.route('GET', path, ok(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', page: null, blocks: paragraphs })], coverage: completeCoverage })));
  const view = mount();
  await screen.findByText('这一段没有取得时码');
  await view.user.click(screen.getByRole('button', { name: '专注阅读' }));
  const dialog = screen.getByRole('dialog', { name: '视频口播 · 专注阅读' });
  const timeTrigger = within(dialog).getByRole('button', { name: /定位口播时间/ });
  await waitFor(() => expect(timeTrigger).toHaveTextContent('本段时间未知'));
  expect(timeTrigger).toHaveAttribute('aria-label', '定位口播时间，当前段落未取得时间');
  await view.user.click(timeTrigger);
  const timeMenu = within(dialog).getByRole('listbox', { name: '口播时间' });
  const options = within(timeMenu).getAllByRole('option');
  expect(options).toHaveLength(2);
  // 键盘初始焦点落在第一项，但第一项没有被标成“当前”。
  expect(timeMenu).toHaveAttribute('aria-activedescendant', options[0]!.id);
  for (const option of options) {
    expect(option).toHaveAttribute('aria-selected', 'false');
  }
  expect(within(timeMenu).queryByText('当前')).not.toBeInTheDocument();
  await view.user.keyboard('{Enter}');
  await waitFor(() => expect(within(dialog).queryByRole('listbox')).not.toBeInTheDocument());
  expect(timeTrigger).toHaveTextContent('00:00');
});
it('目录关闭消费 Esc，正文定位使用 canonical group 而不是秒数作身份', async () => {
  kit.route('GET', path, ok(dossierJson(1)));
  const view = mount(); const trigger = await screen.findByRole('button', { name: /章节导航/ });
  await view.user.click(trigger); expect(screen.getByLabelText('底稿章节与时间')).toBeVisible();
  const outsideEscape = vi.fn(); document.addEventListener('keydown', outsideEscape);
  await view.user.keyboard('{Escape}');
  expect(screen.queryByLabelText('底稿章节与时间')).not.toBeInTheDocument();
  expect(outsideEscape).not.toHaveBeenCalled(); expect(trigger).toHaveFocus();
  document.removeEventListener('keydown', outsideEscape);
});
it.each(['focus', 'export'] as const)('HTTP 200 中局部来源撤回也关闭 %s 并撤掉冻结旧文', async mode => {
  const material = dossierMaterialJson(1, { kind: 'audio', page: null, blocks: [{ ref: 'secret', kind: 'paragraph', text: '来源撤回前的旧口播', time: { startMs: 0, endMs: 1000 } }] });
  kit.route('GET', path, ok(dossierJson(1, { materials: [material], coverage: completeCoverage })));
  vi.spyOn(dossierApi, 'fetchDossierMarkdown').mockResolvedValue('# 来源撤回前的旧口播');
  const view = mount(); await screen.findByText('来源撤回前的旧口播');
  await view.user.click(screen.getByRole('button', { name: mode === 'focus' ? '专注阅读' : '导出' }));
  expect(screen.getByRole('dialog')).toBeVisible();
  const revoked = parseDossierPayload(dossierJson(1, { materials: [{ ...material, sourceState: 'restricted', blocks: [], reading: null, currentRevisionRef: null }], coverage: { ...completeCoverage, sources: { ...completeCoverage.sources, audio: source('restricted', 1) } } }));
  revoked.dossier.revision = 'revoked-revision';
  view.client.setQueryData(dossierKey(DOMAIN, workRef(1)), revoked);
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(screen.queryByText('来源撤回前的旧口播')).not.toBeInTheDocument();
  expect(screen.getByText('来源不可读取。')).toBeVisible();
});
