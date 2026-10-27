import { useEffect, useId, useMemo, useState } from 'react';
import { marked } from 'marked';
import DOMPurify from 'dompurify';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { dossierExport, dossierMarkdownExport, downloadFile, type DossierFile } from './exportDossier';
import type { DossierPayload } from './dossierTypes';
import { DossierIcon } from './DossierIcon';
import styles from './DossierDocument.module.css';

function escapeMarkup(text: string): string { return text.replace(/[&<>"']/g, character => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character] ?? character); }
function renderMarkdown(text: string): string {
  const renderer = new marked.Renderer();
  renderer.html = token => /^<br\s*\/?\s*>$/i.test(token.text) ? '<br>' : escapeMarkup(token.text);
  renderer.image = token => escapeMarkup(token.text);
  return DOMPurify.sanitize(marked.parse(text, { gfm: true, breaks: false, async: false, renderer }), {
    ALLOWED_TAGS: ['h1','h2','h3','h4','h5','h6','p','br','hr','em','strong','del','blockquote','ul','ol','li','table','thead','tbody','tr','th','td','pre','code','a'],
    ALLOWED_ATTR: ['href','title','start'],
    ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto):|\/(?!\/)|#)/i,
    RETURN_TRUSTED_TYPE: false,
  });
}
function failureMessage(error: unknown): string {
  if (error instanceof ApiError && error.failure.kind === 'http') {
    if (error.failure.status === 409) return '底稿版本已变化，请关闭弹窗并刷新底稿后重新导出。';
    if ([401,403,404,410].includes(error.failure.status)) return '当前来源已不可读取，导出已停止。请重新读取底稿。';
  }
  return 'Markdown 读取未完成，可以重试；当前阅读内容仍保留。';
}
async function copyFile(text: string): Promise<boolean> {
  try { if (navigator.clipboard?.writeText) { await navigator.clipboard.writeText(text); return true; } } catch { /* Report failure instead of a misleading success toast. */ }
  return false;
}
type MarkdownState = { state: 'loading' } | { state: 'ready'; file: DossierFile } | { state: 'error'; error: unknown };

/** Each mount freezes one exact revision. Refreshes behind the dialog cannot replace its files. */
export function DossierExportDialog({ payload, domain, onClose, onUnavailable }: { payload: DossierPayload; domain: string; onClose: () => void; onUnavailable: () => void }) {
  const titleId = useId();
  const [snapshot] = useState(() => ({ payload, domain, json: dossierExport(payload) }));
  const [format, setFormat] = useState<'md' | 'json'>('md');
  const [previewMode, setPreviewMode] = useState<'rendered' | 'source'>('rendered');
  const [markdown, setMarkdown] = useState<MarkdownState>({ state: 'loading' });
  const [attempt, setAttempt] = useState(0);
  const [feedback, setFeedback] = useState('');
  useEffect(() => {
    const controller = new AbortController();
    void dossierMarkdownExport(snapshot.payload, snapshot.domain, controller.signal).then(file => {
      if (!controller.signal.aborted) setMarkdown({ state: 'ready', file });
    }).catch((error: unknown) => {
      if (controller.signal.aborted) return;
      setMarkdown({ state: 'error', error });
      if (error instanceof ApiError && error.failure.kind === 'http' && [401,403,404,410].includes(error.failure.status)) onUnavailable();
    });
    return () => controller.abort();
  }, [snapshot, attempt, onUnavailable]);
  const blocked = markdown.state === 'error' && markdown.error instanceof ApiError && markdown.error.failure.kind === 'http' && [401,403,404,409,410].includes(markdown.error.failure.status);
  const file = blocked ? null : format === 'json' ? snapshot.json : markdown.state === 'ready' ? markdown.file : null;
  const html = useMemo(() => markdown.state === 'ready' ? renderMarkdown(markdown.file.data) : '', [markdown]);
  const bytes = file ? new TextEncoder().encode(file.data).length : 0;
  function retry() { setMarkdown({ state: 'loading' }); setAttempt(value => value + 1); }
  return <ModalDialog labelledBy={titleId} className={`${styles.surface} ${styles.dialog} ${styles.exportDialog}`} onClose={onClose} closeOnBackdrop>
    <header className={styles.modalHeader}><div><h2 id={titleId}>导出研究底稿</h2><p>与当前阅读版本一致</p></div><button type="button" className={styles.close} aria-label="关闭导出窗口" onClick={onClose}><DossierIcon name="close"/></button></header>
    <div className={styles.exportSettings}>
      <div className={styles.formatTabs} aria-label="导出格式"><button type="button" aria-pressed={format === 'md'} onClick={() => { setFormat('md'); setFeedback(''); }}>Markdown</button><button type="button" aria-pressed={format === 'json'} onClick={() => { setFormat('json'); setFeedback(''); }}>JSON</button></div>
      <div className={styles.displayActions}><span className={styles.formatHint}>{format === 'md' ? '阅读与 AI 对话' : '程序与 Agent 读取'}</span>{format === 'md' ? <div className={styles.previewMode} aria-label="Markdown 预览方式"><button type="button" aria-pressed={previewMode === 'rendered'} onClick={() => setPreviewMode('rendered')}>排版</button><button type="button" aria-pressed={previewMode === 'source'} onClick={() => setPreviewMode('source')}>源码</button></div> : null}</div>
    </div>
    <p className={styles.exportScope}>导出当前快照的全部内容，并保留真实缺口；不受当前阅读位置影响。</p>
    <div className={styles.exportContent}>
      {markdown.state === 'error' && (format === 'md' || blocked) ? <div className={styles.empty} role="status"><p>{failureMessage(markdown.error)}</p>{!blocked ? <ActionButton className={styles.control} onClick={retry}>重试读取</ActionButton> : null}</div>
        : !file ? <p className={styles.empty} role="status" aria-busy="true">正在读取当前版本的 Markdown…</p>
          : format === 'md' && previewMode === 'rendered' ? <article className={styles.markdown} tabIndex={0} aria-label="Markdown 排版预览" dangerouslySetInnerHTML={{ __html: html }}/>
            : <pre className={styles.source} tabIndex={0} aria-label={format === 'md' ? 'Markdown 源码' : 'JSON 源码'}>{file.data}</pre>}
    </div>
    <footer className={styles.modalFooter}><div className={styles.fileInfo}>{file ? `${file.name} · ${(bytes / 1024).toFixed(1)} KB` : '当前版本尚未准备完成'}<p className={styles.feedback} role="status">{feedback}</p></div><div className={styles.actions}>
      <ActionButton className={styles.control} disabled={!file} onClick={() => { if (file) void copyFile(file.data).then(ok => setFeedback(ok ? '已复制完整底稿' : '复制未完成，请使用下载文件。')); }}><DossierIcon name="copy"/>复制内容</ActionButton>
      <ActionButton className={`${styles.control} ${styles.primary}`} disabled={!file} onClick={() => { if (file) { downloadFile(file); setFeedback('文件已准备，请查看浏览器下载。'); } }}><DossierIcon name="download"/>下载 .{format}</ActionButton>
    </div></footer>
  </ModalDialog>;
}
