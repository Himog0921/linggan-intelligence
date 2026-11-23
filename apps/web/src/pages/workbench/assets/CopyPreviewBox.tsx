import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import * as api from './assetsApi';
import { blockedFindings } from './assetsModel';

const SHOWN_FINDINGS = 5;

/**
 * The text 复制 would put on the clipboard for the saved version, produced by the server's own copy rules (the same
 * ones that check and format it), so it cannot differ from what is copied. Reading it records nothing: copying still
 * goes through 复制, which writes the receipt.
 */
export function CopyPreviewBox({ workId, revisionId, unsaved }: { workId: string; revisionId: string; unsaved: boolean }) {
  const preview = useQuery({
    queryKey: ['creation-assets', 'copy-preview', workId, revisionId],
    queryFn: ({ signal }) => api.copyPreview(workId, signal),
    retry: false, gcTime: 0,
  });
  const blocked = preview.isError ? blockedFindings(preview.error) : null;
  const emptyBody = preview.isError && preview.error instanceof ApiError && preview.error.failure.kind === 'http' && preview.error.failure.code === 'creation_assets_parameter_invalid';
  return <div className="copy-box">
    <label className="lbl" htmlFor="copyPreview">复制出去的文字（空行里是ㅤ占位符）</label>
    {preview.data ? <textarea className="input" id="copyPreview" readOnly value={preview.data.text} />
      : preview.isPending ? <p className="aux" role="status">正在按复制的规则生成…</p>
        : blocked ? <div role="alert">
          <p className="aux">这个版本还有必改项，改完才能复制，所以现在没有可预览的文字：</p>
          {blocked.slice(0, SHOWN_FINDINGS).map((finding, index) => <p className="aux" key={`${finding.rule}:${index}`}><span className="dot bad" /> {finding.message}</p>)}
          {blocked.length > SHOWN_FINDINGS ? <p className="aux">还有 {blocked.length - SHOWN_FINDINGS} 条，在检查里查看。</p> : null}
        </div>
          : emptyBody ? <p className="aux">已保存的版本还没有正文，保存后这里会显示。</p>
            : <p className="aux" role="alert">{api.assetsErrorText(preview.error)} <ActionButton variant="quiet" onClick={() => void preview.refetch()}>重试</ActionButton></p>}
    {/* With unsaved edits every state is about the last saved version, so say so; otherwise only a shown text needs a caption. */}
    {unsaved || preview.data ? <p className="aux">
      {unsaved ? '这是上次保存的版本，编辑中还没保存的改动不在这里。' : '这是已保存版本要复制的文字。'}
      {preview.data ? `按复制的规则检查：提醒 ${preview.data.warnCount} 条、提示 ${preview.data.hintCount} 条，都不挡复制。` : ''}
    </p> : null}
  </div>;
}
