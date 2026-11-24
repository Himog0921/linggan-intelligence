import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { IntelligenceIcon } from '../../shared/IntelligenceIcon';
import { humanMoment } from '../../shared/formatting';
import { readRevision } from '../workbench/workbenchApi';
import * as api from './api';
import * as f from './format';
import { c } from './styles';
import { Dialog, External, Status, Tag } from './ui';

export type PublicationCreationLink = {
  linkRef: string;
  workId: string;
  revisionId: string;
};

export function PublicationDetail({ detail, isAdmin, snapshotLinks, close }: {
  detail: api.Publication;
  isAdmin: boolean;
  snapshotLinks?: PublicationCreationLink[];
  close: () => void;
}) {
  const [cover, setCover] = useState<string | null>(null);
  const [viewRevision, setViewRevision] = useState(false);
  const creation = detail.creation;
  // The lens uses frozen membership and counts, while the ledger fields are read live.
  // Require the same immutable link, including when an unlink is followed by a re-link.
  const snapshotMatch = snapshotLinks === undefined || (creation && snapshotLinks.some(link =>
    link.linkRef === creation.linkRef && link.workId === creation.workId && link.revisionId === creation.revisionId));
  const canViewRevision = isAdmin && creation && snapshotMatch;
  const revisionLabel = creation?.registeredWithDraft ? '登记时稿件' : '关联时稿件';

  return <>
    <Dialog label="发布记录详情" outsideClose onClose={close}>
      <header className={c('ops-detail-head')}>
        <h2>{viewRevision ? revisionLabel : detail.title || '无标题发布记录'}</h2>
        <div className={c('ops-row-actions')}>
          {canViewRevision ? <ActionButton variant="quiet" onClick={() => setViewRevision(!viewRevision)}>
            {viewRevision ? '返回发布记录详情' : `查看${revisionLabel}`}
          </ActionButton> : null}
          <ActionButton onClick={close}><IntelligenceIcon name="close" size={16}/>关闭详情</ActionButton>
        </div>
      </header>
      <div className={c('ops-dialog-body')}>
        {viewRevision && canViewRevision ? <LinkedRevision workId={creation.workId} revisionId={creation.revisionId} /> : <>
          {snapshotLinks !== undefined ? <p className={c('ops-note')}>当前台账详情 · 看板指标取自快照</p> : null}
          <p>{detail.businessDate} · {f.platformLabel[detail.platform]} · {detail.account?.name || '账号未知'}</p>
          <p className={c('ops-note')}>
            {detail.source === "native" ? "运营台账原生登记" : "工作台历史迁入"}
            {' · '}系统尚未核对平台发布状态
          </p>
          {detail.publisher ? <p className={c("ops-note")}>原发布人 · {detail.publisher}</p> : null}
          {isAdmin ? <HistoricalOwnerTag publication={detail} /> : null}
          {snapshotLinks !== undefined && !snapshotMatch ? <p className={c('ops-note')}>
            {snapshotLinks.length === 0 ? '此快照未记录稿件关联。' : '快照中的稿件关联在当前台账已不可用。'}
          </p> : null}
          {snapshotLinks === undefined && creation ? <p><Link to={`/ops/creation?${new URLSearchParams({ domain: creation.domainRef, work: creation.workId, view: "body" })}`}>
            {isAdmin ? '打开当前稿件' : '查看关联稿件'}
          </Link></p> : null}
          <External href={detail.url} title="原发布链接"><IntelligenceIcon name="external" size={16}/> 查看原发布链接</External>
          <dl className={c('ops-detail-facts')}>
            <div><dt>阅读量</dt><dd>{detail.views.state === 'value' ? f.count(detail.views.value).text : detail.views.state === 'pending' ? '待补' : '选填，未知'}</dd></div>
            <div><dt>点赞</dt><dd>{f.count(detail.likes).text}</dd></div><div><dt>收藏</dt><dd>{f.count(detail.collects).text}</dd></div><div><dt>评论</dt><dd>{f.count(detail.comments).text}</dd></div>
          </dl>
          <p className={c('ops-note')}>
            采集媒体：{mediaLabels[detail.media.state]}{detail.media.observed ? `（已观察 ${detail.media.observed}，本地可用 ${detail.media.available}，失败 ${detail.media.failed}，当前不可用 ${detail.media.unavailable}）` : ''}
          </p>
          <FieldAdoptionNotice adoption={detail.fieldAdoption} />
          <h3>发布正文</h3><p className={c('ops-full-text')}>{detail.body || '未迁入正文'}</p>
          {detail.duplicates.length ? <p>合并的重复登记日期：{detail.duplicates.map(row => row.businessDate).join('、')}</p> : null}
          {detail.cover.available ? <ActionButton onClick={() => setCover(`/ops/covers/${encodeURIComponent(detail.ref)}`)}>查看封面大图</ActionButton> : <p>封面未迁入</p>}
        </>}
      </div>
    </Dialog>
    {cover ? <PublicationCover src={cover} close={() => setCover(null)} /> : null}
  </>;
}

function LinkedRevision({ workId, revisionId }: { workId: string; revisionId: string }) {
  const revision = useQuery({
    queryKey: ['ops', 'publication-revision', workId, revisionId],
    queryFn: ({ signal }) => readRevision(workId, revisionId, signal),
    retry: false,
  });
  if (revision.isError) return <Status error retry={() => void revision.refetch()}>此稿件版本无法读取。</Status>;
  if (revision.isPending) return <Status>正在读取稿件版本…</Status>;
  const { number, savedAt, document } = revision.data;
  return <section>
    <h3>{document.title || '未填写稿件标题'}</h3>
    <p className={c('ops-note')}>版本 {number} · 保存于 {humanMoment(savedAt)}</p>
    <div className={c('ops-full-text')}>{document.paragraphs.length > 0
      ? document.paragraphs.map(paragraph => <p key={paragraph.id}>{paragraph.text}</p>)
      : <p>此版本未填写正文。</p>}</div>
    {document.tags.length > 0 ? <p>{document.tags.join(' · ')}</p> : null}
  </section>;
}

export function PublicationCover({ src, close }: { src: string; close: () => void }) {
  const [failed, setFailed] = useState(false);
  return <Dialog label="封面大图" outsideClose className="ops-cover-dialog" onClose={close}>
    <form method="dialog" onSubmit={event => { event.preventDefault(); close(); }}>
      <ActionButton type="submit">关闭</ActionButton>
    </form>
    {failed ? <div role="status"><p>封面暂时无法读取，原记录仍保留。</p>
      <ActionButton onClick={() => setFailed(false)}>重新加载封面</ActionButton>
    </div> : <img id="ops-cover-large" src={src} alt="封面大图" onError={() => setFailed(true)} />}
  </Dialog>;
}

export function HistoricalOwnerTag({ publication }: { publication: api.Publication }) {
  if (publication.source !== "imported") return null;
  const state = publication.historicalOwnerState;
  if (state === "matched") return <Tag tone="success">归属已核实</Tag>;
  return <Tag tone="warning">{state === "conflict_pending" ? "归属有冲突" : "归属待核实"}</Tag>;
}

const mediaLabels: Record<api.Publication["media"]["state"], string> = {
  not_requested: "未请求采集媒体",
  waiting_for_slots: "等待读取图片/视频",
  processing: "图片/视频处理中",
  partial: "媒体部分可用",
  available: "媒体已本地验证",
  failed: "媒体处理失败",
  unavailable: "媒体当前不可用",
};
export function MediaTag({ media }: { media: api.Publication["media"] }) {
  const tone = media.state === "available" ? "success" : media.state === "failed" || media.state === "partial" || media.state === "unavailable" ? "warning" : undefined;
  const title = media.observed
    ? `已观察 ${media.observed} 项；本地验证 ${media.available} 项；失败 ${media.failed} 项；当前不可用 ${media.unavailable} 项。远程地址不等于可用媒体。`
    : "媒体状态独立于文字回填；远程地址不等于本地可用资源。";
  return <Tag tone={tone} title={title}>{mediaLabels[media.state]}</Tag>;
}
const adoptionLabels: Record<api.Publication["fieldAdoption"]["title"], string> = {
  adopted: "已采用",
  not_provided: "未提供",
  conflict: "待核实",
};
const adoptionNames: Record<keyof api.Publication["fieldAdoption"], string> = {
  title: "标题", body: "正文", likes: "点赞", collects: "收藏", comments: "评论", publishedAt: "发布时间",
};
function conflictingFields(adoption: api.Publication["fieldAdoption"]) {
  return (Object.entries(adoption) as [keyof typeof adoption, typeof adoption.title][])
    .filter(([, state]) => state === "conflict")
    .map(([field]) => adoptionNames[field]);
}
export function FieldAdoptionTag({ adoption }: { adoption: api.Publication["fieldAdoption"] }) {
  const conflicts = conflictingFields(adoption);
  if (!conflicts.length) return null;
  return <Tag tone="warning" title={`采集观察与原生登记不一致：${conflicts.join("、")}。当前保留冲突，尚未自动覆盖。`}>字段待核实</Tag>;
}
function FieldAdoptionNotice({ adoption }: { adoption: api.Publication["fieldAdoption"] }) {
  const conflicts = conflictingFields(adoption);
  if (conflicts.length) return <p className={c("ops-note")}>字段采用：{conflicts.join("、")}待核实；当前保留原登记，尚未自动覆盖。</p>;
  const missing = (Object.entries(adoption) as [keyof typeof adoption, typeof adoption.title][])
    .filter(([, state]) => state === "not_provided")
    .map(([field]) => adoptionNames[field]);
  return missing.length ? <p className={c("ops-note")}>字段采用：{missing.join("、")}未由当前来源提供。</p> : <p className={c("ops-note")}>字段采用：{adoptionLabels.adopted}的观察已按当前身份版本显示。</p>;
}
