import type { ExportRecord, Revision } from './workbenchModel';
import { RequestIdentities } from '../../shared/requestIdentity';
import { ApiError } from '../../shared/api';
import { FONT_PROFILE, RENDERER_VERSION, RenderError, pngBlob, renderPage } from './renderer';

export interface FrozenExport { export: ExportRecord; revision: Revision }
export interface ExportTransport {
  freeze: (workId: string, request: {requestId: string; expectedRevision: string; rendererVersion: string; fontVersion: string}) => Promise<FrozenExport>;
  upload: (exportId: string, pageId: string, blob: Blob) => Promise<ExportRecord>;
  finalize: (exportId: string, request: {requestId: string}) => Promise<ExportRecord>;
  retry: (exportId: string, request: {requestId: string}) => Promise<FrozenExport>;
  fail: (exportId: string, request: {requestId: string; code: string}) => Promise<ExportRecord>;
  lookup: (requestId: string) => Promise<FrozenExport | null>;
  read: (exportId: string) => Promise<ExportRecord>;
}
export interface ExportProgress { stage: 'saving' | 'freezing' | 'rendering' | 'uploading' | 'finalizing' | 'complete'; page?: number; total?: number }
interface ExportSession {
  workId: string;
  saved?: Revision;
  frozen?: FrozenExport;
  freezeId: string;
  finalizeId: string;
  retryId: string;
  failId: string;
  needsRetry: boolean;
  blobs: Map<string, Blob>;
  save?: () => Promise<Revision>;
  resumeId?: string;
  singlePageId?: string;
  uncertain?: 'save' | 'freeze' | 'retry' | 'upload' | 'finalize' | 'fail';
  failureCode?: string;
}
export type RenderFrozenPage = (page: Revision['document']['pages'][number], assetUrl?: string) => Promise<Blob>;

async function renderFrozenPage(page: Revision['document']['pages'][number], assetUrl?: string): Promise<Blob> {
  const result = await renderPage(page, { assetUrl });
  if (result.issues.length) throw new RenderError('overflow', result.issues.map(issue => issue.message).join('；'));
  return pngBlob(result.canvas);
}

/** Command identities and exact PNG bytes survive uncertain network outcomes in memory. */
export class ExportWorkflow {
  private session: ExportSession | undefined;
  private running = false;
  private readonly identities = new RequestIdentities();
  constructor(private readonly transport: ExportTransport, private readonly render: RenderFrozenPage = renderFrozenPage) {}

  get frozenRevisionId(): string | undefined { return this.session?.frozen?.revision.id; }
  get canRetry(): boolean { return this.session !== undefined; }
  get hasUnconfirmed(): boolean { return this.session?.uncertain !== undefined; }

  async start(workId: string, save: () => Promise<Revision>, progress?: (value: ExportProgress) => void, singlePageId?: string): Promise<ExportRecord> {
    if (this.running) throw new Error('正在生成，请等待当前任务');
    if (this.hasUnconfirmed) throw new Error('原生成请求结果尚未确认，请先核实原请求再生成新版');
    this.session = this.newSession(workId);
    this.session.save = save;
    if (singlePageId !== undefined) this.session.singlePageId = singlePageId;
    return this.run(save, progress);
  }

  async resume(record: ExportRecord, progress?: (value: ExportProgress) => void): Promise<ExportRecord> {
    if (this.running) throw new Error('正在生成，请等待当前任务');
    if (this.hasUnconfirmed) throw new Error('原生成请求结果尚未确认，请先核实原请求');
    if (record.state === 'complete') return record;
    if (record.rendererVersion !== RENDERER_VERSION || record.fontVersion !== FONT_PROFILE) {
      throw new Error('此冻结版本使用的字体或绘图版本与当前页面不同，请生成新版图文包');
    }
    const session = this.newSession(record.workId);
    this.session = session;
    // retry returns the old frozen revision, never the mutable editor state.
    session.resumeId = record.id;
    return this.run(undefined, progress);
  }

  async retry(progress?: (value: ExportProgress) => void): Promise<ExportRecord> {
    if (!this.session) throw new Error('没有待重试的生成任务');
    return this.run(undefined, progress);
  }

  private newSession(workId: string): ExportSession {
    const sessionId = crypto.randomUUID();
    return { workId, freezeId: this.identities.keyFor('freeze', sessionId), finalizeId: this.identities.keyFor('finalize', sessionId), retryId: this.identities.keyFor('retry', sessionId),
      failId: this.identities.keyFor('fail', sessionId), needsRetry: false, blobs: new Map() };
  }

  private validateLookup(session: ExportSession, found: FrozenExport): void {
    if (found.export.workId !== session.workId || found.export.revisionId !== found.revision.id ||
      (session.saved && found.revision.id !== session.saved.id) ||
      (session.resumeId && found.export.id !== session.resumeId) ||
      (session.frozen && (found.export.id !== session.frozen.export.id || found.revision.id !== session.frozen.revision.id))) {
      throw new Error('原请求回执与冻结作品不符，未采用返回内容');
    }
  }

  private async command<T>(session: ExportSession, action: 'freeze' | 'retry' | 'finalize' | 'fail', requestId: string,
    send: () => Promise<T>, fromLookup: (found: FrozenExport) => T): Promise<T> {
    let reconciling = session.uncertain === action;
    try {
      if (reconciling) {
        const found = await this.transport.lookup(requestId);
        if (found) {
          this.validateLookup(session, found);
          session.uncertain = undefined;
          return fromLookup(found);
        }
        reconciling = false;
      }
      const result = await send(); session.uncertain = undefined; return result;
    } catch (error) {
      if (reconciling || !(error instanceof ApiError && error.answered)) session.uncertain = action;
      else session.uncertain = undefined;
      throw error;
    }
  }

  private async run(save?: () => Promise<Revision>, progress?: (value: ExportProgress) => void): Promise<ExportRecord> {
    if (this.running) throw new Error('正在生成，请等待当前任务');
    const session = this.session;
    if (!session) throw new Error('没有待生成的作品');
    this.running = true;
    try {
      if (!session.frozen && session.resumeId) {
        session.frozen = await this.command(session, 'retry', session.retryId,
          () => this.transport.retry(session.resumeId!, { requestId: session.retryId }), found => found);
      }
      if (!session.saved && !session.frozen) {
        const savePending = save ?? session.save;
        if (!savePending) throw new Error('保存尚未确认，请先重新保存作品再生成');
        progress?.({ stage: 'saving' });
        // No freeze or render can occur until a real save receipt arrives.
        try { session.saved = await savePending(); session.uncertain = undefined; }
        catch (error) { session.uncertain = error instanceof ApiError && error.answered ? undefined : 'save'; throw error; }
      }
      if (!session.frozen) {
        if (!session.saved) throw new Error('保存尚未确认');
        progress?.({ stage: 'freezing' });
        const saved = session.saved;
        session.frozen = await this.command(session, 'freeze', session.freezeId,
          () => this.transport.freeze(session.workId, { requestId: session.freezeId,
            expectedRevision: saved.id, rendererVersion: RENDERER_VERSION, fontVersion: FONT_PROFILE }), found => found);
      }
      if (session.uncertain === 'fail' && session.failureCode) {
        const frozen = session.frozen;
        await this.command(session, 'fail', session.failId, () => this.transport.fail(frozen.export.id,
          { requestId: session.failId, code: session.failureCode! }), found => found.export);
      }
      if (session.needsRetry) {
        const frozen = session.frozen;
        session.frozen = await this.command(session, 'retry', session.retryId,
          () => this.transport.retry(frozen.export.id, { requestId: session.retryId }), found => found);
        session.needsRetry = false;
      }
      const frozen = session.frozen;
      if (session.uncertain === 'upload') {
        const found = await this.transport.read(frozen.export.id);
        if (found.id !== frozen.export.id || found.workId !== session.workId || found.revisionId !== frozen.revision.id) throw new Error('图页回执与冻结版本不符');
        for (const item of found.pages) {
          const bytes = session.blobs.get(item.pageId);
          if (bytes && item.sha256) {
            const hash = await crypto.subtle.digest('SHA-256', await bytes.arrayBuffer());
            const hex = Array.from(new Uint8Array(hash), value => value.toString(16).padStart(2,'0')).join('');
            if (hex !== item.sha256) throw new Error('已登记图页与原 PNG 不一致，未继续上传');
          }
        }
        frozen.export = found; session.uncertain = undefined;
      }
      if (frozen.export.state === 'complete') { progress?.({ stage: 'complete' }); return frozen.export; }
      const pages = frozen.revision.document.pages;
      if (frozen.export.rendererVersion !== RENDERER_VERSION || frozen.export.fontVersion !== FONT_PROFILE) {
        throw new Error('此冻结版本使用的字体或绘图版本与当前页面不同，请生成新版图文包');
      }
      if (frozen.export.revisionId !== frozen.revision.id || frozen.export.workId !== session.workId ||
        frozen.export.pages.length !== pages.length || frozen.export.pages.some((item,index) => item.pageId !== pages[index]?.id || item.ordinal !== index + 1)) {
        throw new Error('冻结图页回执不一致，未上传图片，请重新读取作品后重试');
      }
      if (session.singlePageId && !pages.some(page => page.id === session.singlePageId)) {
        throw new Error('所选图页已改变，请返回编辑区重新选择');
      }
      for (const [index, page] of pages.entries()) {
        if (session.singlePageId && page.id !== session.singlePageId) continue;
        if (frozen.export.pages.find(item => item.pageId === page.id)?.sha256) continue;
        let blob = session.blobs.get(page.id);
        if (!blob) {
          progress?.({ stage: 'rendering', page: index + 1, total: pages.length });
          blob = await this.render(page, page.assetId ? `/ops/api/creation/works/${session.workId}/assets/${page.assetId}` : undefined);
          session.blobs.set(page.id, blob);
        }
        progress?.({ stage: 'uploading', page: index + 1, total: pages.length });
        try { frozen.export = await this.transport.upload(frozen.export.id, page.id, blob); }
        catch (error) { if (!(error instanceof ApiError && error.answered)) session.uncertain = 'upload'; throw error; }
      }
      if (session.singlePageId) { progress?.({ stage: 'complete' }); return frozen.export; }
      progress?.({ stage: 'finalizing' });
      frozen.export = await this.command(session, 'finalize', session.finalizeId,
        () => this.transport.finalize(frozen.export.id, { requestId: session.finalizeId }), found => found.export);
      if (frozen.export.state !== 'complete') throw new Error('服务端尚未确认图文包完整，请重试');
      progress?.({ stage: 'complete' });
      return frozen.export;
    } catch (error) {
      if (error instanceof RenderError && session.frozen) {
        // A render failure is known. Network upload failures remain unconfirmed, with identical bytes retained.
        session.needsRetry = true;
        const failure = crypto.randomUUID();
        session.failId = this.identities.keyFor('fail', failure);
        session.retryId = this.identities.keyFor('retry', failure);
        session.failureCode = error.code;
        const frozen = session.frozen;
        try { await this.command(session, 'fail', session.failId, () => this.transport.fail(frozen.export.id,
          { requestId: session.failId, code: error.code }), found => found.export); } catch { /* explicit retry reconciles this failure receipt first */ }
      }
      throw error;
    } finally { this.running = false; }
  }
}

export function exportPackageUrl(id: string): string { return `/ops/api/creation/exports/${id}/download`; }
export function exportPageUrl(id: string, pageId: string): string { return `/ops/api/creation/exports/${id}/pages/${pageId}`; }
