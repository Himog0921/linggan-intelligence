import type { WorkDocument } from '../workbenchModel';
import { dedupeTags, previewParts } from './assetsText';
import { CopyPreviewBox } from './CopyPreviewBox';
import { CoverCanvas } from './CoverCanvas';

/**
 * The 预览 tab: the draft laid out like a phone note. The cover comes from the export renderer. It shows the
 * draft being edited, not the copy text — copying still goes through the saved, checked revision.
 */
export function PreviewTab({ document, workId, tagsDraft, savedRevisionId = null, unsaved = false, enabled = false }: {
  document: WorkDocument; workId: string; tagsDraft: string;
  /** For the 复制出去的文字 box, which is read from the server for the saved revision, only while this tab is showing. */
  savedRevisionId?: string | null; unsaved?: boolean; enabled?: boolean;
}) {
  const cover = document.pages.find(page => page.kind === 'cover') ?? null;
  const parts = previewParts(document);
  const tags = dedupeTags(tagsDraft);
  return <>
    <div className="phone">
      <div className="cover">
        {cover ? <CoverCanvas page={cover} workId={workId} /> : null}
        {document.coverAi === null ? null : <span className="cover-tag">{document.coverAi ? 'AI 作图' : '实拍'}</span>}
      </div>
      <div className="phone-body">
        <p className="phone-title">{document.title || '未命名稿件'}</p>
        {parts.length ? parts.map((part, index) => <div key={index}>
          <p className={part.bridge ? 'phone-bridge' : undefined}>{part.text}</p>
          {index < parts.length - 1 ? <p aria-hidden="true">ㅤ</p> : null}
        </div>) : <p className="aux">还没有正文。</p>}
        {tags.length ? <p className="phone-tags">{tags.join(' ')}</p> : null}
      </div>
      <div className="phone-bar" aria-hidden="true"><span>赞</span><span>收藏</span><span>评论</span></div>
    </div>
    <p className="aux" style={{ textAlign: 'center' }}>按复制到小红书后的样子显示。这是编辑中的预览，复制时会使用保存并检查后的稿件。</p>
    {enabled && savedRevisionId && parts.length ? <CopyPreviewBox workId={workId} revisionId={savedRevisionId} unsaved={unsaved} /> : null}
  </>;
}
