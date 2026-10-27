import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DossierExportDialog } from './DossierExportDialog';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, REVISION, workRef } from './evidenceFixtures';
import * as exportFiles from './exportDossier';
import { createFetchMock, fail, ok, type FetchKit } from './testKit';
const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
let kit: FetchKit;
beforeEach(() => {
  kit = createFetchMock();
  vi.stubGlobal('fetch', async (input: string | URL | Request, init?: RequestInit) => {
    const response = await kit.fetchMock(input, init);
    if (response.ok) response.headers.set('Content-Type', 'text/markdown; charset=utf-8');
    return response;
  });
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });
function mount(text: string) {
  kit.route('GET', path, ok(text));
  const user = userEvent.setup();
  const payload = parseDossierPayload(dossierJson(1));
  const onClose = vi.fn(), onUnavailable = vi.fn();
  const view = render(<DossierExportDialog payload={payload} domain={DOMAIN} onClose={onClose} onUnavailable={onUnavailable}/>);
  return { ...view, user, payload, onClose, onUnavailable };
}
it('默认解析实际 MD 文件，保留表格换行和全文；源码、复制、下载使用同一字符串', async () => {
  const markdown = '# 实际文件标题\n\n第一行  \n第二行\n\n| 名称 | 说明 |\n| --- | --- |\n| 内容 | 上行<br>下行 |\n\n最后口播段落不截断。';
  const view = mount(markdown);
  const download = vi.spyOn(exportFiles, 'downloadFile').mockImplementation(() => {});
  const clipboard = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
  const preview = await screen.findByRole('article', { name: 'Markdown 排版预览' });
  expect(within(preview).getByRole('heading', { name: '实际文件标题' })).toBeVisible();
  expect(within(preview).getByRole('table')).toBeVisible();
  expect(preview.querySelectorAll('br')).toHaveLength(2);
  expect(preview).toHaveTextContent('最后口播段落不截断。');
  expect(kit.calls[0]?.search.get('expectedRevision')).toBe(REVISION);
  expect(kit.calls[0]?.search.get('asOf')).toBe(view.payload.dossier.asOf);
  await view.user.click(screen.getByRole('button', { name: '源码' }));
  expect(screen.getByLabelText('Markdown 源码').textContent).toBe(markdown);
  await view.user.click(screen.getByRole('button', { name: '复制内容' }));
  expect(clipboard).toHaveBeenCalledWith(markdown);
  await view.user.click(screen.getByRole('button', { name: '下载 .md' }));
  expect(download).toHaveBeenCalledWith(expect.objectContaining({ data: markdown, type: 'text/markdown;charset=utf-8' }));
});
it('HTML 与恶意链接只作为安全文本，允许 serializer 的单元格 br 而不加载图片', async () => {
  mount('# 安全预览\n\n<script>window.pwned=true</script>\n\n<img src=x onerror=alert(1)>\n\n[错误链接](javascript:alert%281%29)\n\n![图片文字](https://external.example/track.png)\n\n原文字面量：\\<br\\>、\\<product-id\\>。');
  const preview = await screen.findByRole('article', { name: 'Markdown 排版预览' });
  expect(preview.querySelector('script, img, iframe, [onerror]')).toBeNull();
  expect(preview.querySelector('a[href^="javascript:"]')).toBeNull();
  expect(preview).toHaveTextContent('<script>window.pwned=true</script>');
  expect(preview).toHaveTextContent('<br>、<product-id>');
  expect(preview.querySelector('br')).toBeNull();
});
it('弹窗冻结 revision 与 JSON，后台新 payload 不替换已打开会话；JSON 隐藏 MD 预览方式', async () => {
  const view = mount('# 冻结版本');
  await screen.findByRole('article', { name: 'Markdown 排版预览' });
  const changed = parseDossierPayload(dossierJson(1, { bodyText: '后台新正文' })); changed.dossier.revision = 'new-revision';
  view.rerender(<DossierExportDialog payload={changed} domain="changed" onClose={view.onClose} onUnavailable={view.onUnavailable}/>);
  await view.user.click(screen.getByRole('button', { name: 'JSON' }));
  expect(screen.queryByRole('button', { name: '源码' })).not.toBeInTheDocument();
  const json = screen.getByLabelText('JSON 源码').textContent;
  expect(json).toBe(exportFiles.dossierExport(view.payload).data);
  expect(json).not.toContain('后台新正文');
  expect(kit.calls).toHaveLength(1);
  await view.user.click(screen.getByRole('button', { name: 'Markdown' }));
  expect(screen.getByRole('article', { name: 'Markdown 排版预览' })).toHaveTextContent('冻结版本');
});
it('409 阻止混合版本导出，不把旧 JSON 和新 MD 拼在一起', async () => {
  kit.route('GET', path, fail(409, 'dossier_revision_changed'));
  const user = userEvent.setup();
  render(<DossierExportDialog payload={parseDossierPayload(dossierJson(1))} domain={DOMAIN} onClose={vi.fn()} onUnavailable={vi.fn()}/>);
  await screen.findByText(/底稿版本已变化/);
  expect(screen.getByRole('button', { name: '下载 .md' })).toBeDisabled();
  await user.click(screen.getByRole('button', { name: 'JSON' }));
  expect(screen.getByRole('button', { name: '下载 .json' })).toBeDisabled();
  expect(screen.queryByLabelText('JSON 源码')).not.toBeInTheDocument();
});
it('失去来源权限通知面板撤回，复制失败不会给成功提示', async () => {
  const view = mount('# 可读文件');
  await screen.findByRole('article', { name: 'Markdown 排版预览' });
  vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('denied'));
  await view.user.click(screen.getByRole('button', { name: '复制内容' }));
  await screen.findByText('复制未完成，请使用下载文件。');
  fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }));
  expect(view.onClose).toHaveBeenCalledOnce();
  view.unmount();
  const unavailable = vi.fn(); kit.route('GET', path, fail(403, 'source_restricted'));
  render(<DossierExportDialog payload={view.payload} domain={DOMAIN} onClose={vi.fn()} onUnavailable={unavailable}/>);
  await waitFor(() => expect(unavailable).toHaveBeenCalledOnce());
  expect(screen.getByRole('button', { name: '下载 .md' })).toBeDisabled();
});
