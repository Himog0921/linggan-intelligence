import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import * as ops from '../../ops/api';
import type { WorkDocument } from '../workbenchModel';

const COVER_MAX = 10;
const NOTE_MAX = 20;
const size = (text: string) => Array.from(text).length;
const formatReads = (value: number | null) => value === null ? '—' : value.toLocaleString('en-US');

export interface TitleCandidate { text: string; risk: boolean; chosen: boolean }
/** What the page already knows about title generation; the tab only shows it and hands actions back. */
export interface TitleTools {
  /** Starts the same preview → confirm → generate chain as the editor's "换一批". */
  generate: () => void;
  canGenerate: boolean;
  running: boolean;
  message: string;
  cover: readonly TitleCandidate[] | null;
  note: readonly TitleCandidate[] | null;
  /** True while a choice would be unsafe (unsaved edits, another request, or a changed revision). */
  choiceLocked: boolean;
  choose: (kind: 'cover' | 'note', index: number) => void;
}

type Source = 'hot' | 'ours' | 'skill';
const SOURCES: readonly { id: Source; label: string; reason?: string }[] = [
  { id: 'hot', label: '高赞标题', reason: '要等有跨领域的采集数据才有，现在没有。' },
  { id: 'ours', label: '我方爆款' },
  { id: 'skill', label: '标题 skill' },
];
const OURS_LIMIT = 10;

/**
 * The 标题 tab of the writing sidebar: what the draft uses now, and title candidates from the configured title skill.
 * Candidate choice and generation stay on the page (they save, check sources and spend budget); this only renders them.
 */
export function TitlesTab({ document, coverTitle, onCopyTitle, copyDisabled, onGoCover, tools, domainRef, enabled = false, usingLocked = false, onUseOurTitle }: {
  document: WorkDocument; coverTitle: string; onCopyTitle?: () => void; copyDisabled: boolean; onGoCover: () => void; tools?: TitleTools;
  /** For 我方爆款: which business to read the ledger of, whether the tab is showing, and how a chosen title is put into the draft. */
  domainRef?: string; enabled?: boolean; usingLocked?: boolean; onUseOurTitle?: (kind: 'cover' | 'note', text: string) => void;
}) {
  const [source, setSource] = useState<Source>('skill');
  const ours = useQuery({
    queryKey: ['creation-assets', 'own-hits', domainRef], enabled: enabled && !!domainRef && source === 'ours', retry: false, gcTime: 0,
    queryFn: async ({ signal }) => {
      // The ledger decides what a hit is (views=hit). Keeping Douyin out is still checked here: it is never judged a hit.
      const list = await ops.get(ops.url('publications', { domain: domainRef ?? '', views: 'hit' }), ops.publications, signal);
      return list.items.filter(item => item.platform === 'xhs' && item.views.hit && item.title).sort((a, b) => (b.views.value ?? 0) - (a.views.value ?? 0)).slice(0, OURS_LIMIT);
    },
  });
  const line = (kind: 'cover' | 'note', item: TitleCandidate, index: number, max: number) => <div className="t-adapt" key={`${index}:${item.text}`}>
    <span>{item.text}</span>
    <span className={`count${size(item.text) > max ? ' over' : ''}`}>{size(item.text)} 字</span>
    {item.risk ? <span className="chip warn">有疗效承诺，建议换一个</span> : null}
    {item.chosen ? <span className="chip ok">在用</span>
      : <ActionButton variant="quiet" disabled={tools?.choiceLocked ?? true} aria-label={`${kind === 'cover' ? '用作封面' : '用作标题'}：${item.text}`} onClick={() => tools?.choose(kind, index)}>{kind === 'cover' ? '用作封面' : '用作标题'}</ActionButton>}
  </div>;
  const hasCandidates = !!(tools?.cover?.length || tools?.note?.length);
  return <>
    <div className="seg" role="group" aria-label="标题来源">
      {SOURCES.map(item => <button type="button" key={item.id} aria-pressed={item.id === source} disabled={!!item.reason} title={item.reason} onClick={() => setSource(item.id)}>{item.label}</button>)}
    </div>
    <p className="aux">{SOURCES.filter(item => item.reason).map(item => `${item.label}：${item.reason}`).join(' ')}</p>
    <div className="r-sec">
      <h3>现在用的</h3>
      <div className="t-card">
        <div className="aux">笔记标题 <span className={`count${size(document.title) > NOTE_MAX ? ' over' : ''}`}>{size(document.title)}/{NOTE_MAX}</span></div>
        <div className="t-title">{document.title || '还没有填写标题'}</div>
        {onCopyTitle ? <div className="t-adapts"><ActionButton variant="outline" className="sm" disabled={copyDisabled} onClick={onCopyTitle}>复制标题</ActionButton></div> : null}
      </div>
      <div className="t-card">
        <div className="aux">封面大字 <span className={`count${size(coverTitle) > COVER_MAX ? ' over' : ''}`}>{size(coverTitle)}/{COVER_MAX}</span></div>
        <div className="t-title">{coverTitle || '还没有填写封面大字'}</div>
        <div className="t-adapts"><ActionButton variant="quiet" onClick={onGoCover}>编辑封面</ActionButton></div>
      </div>
    </div>
    {source === 'ours' ? <>
      <p className="aux">取运营台账里最近的 50 条爆款（小红书阅读达到爆款线），按阅读从高到低列前 {OURS_LIMIT} 条。阅读数以运营发布记录为准。</p>
      {ours.isPending ? <p className="aux" role="status">正在读取运营台账…</p>
        : ours.isError ? <p className="aux" role="alert">运营台账暂时读不到。<ActionButton variant="quiet" onClick={() => void ours.refetch()}>重试</ActionButton></p>
          : ours.data?.length ? ours.data.map(item => <div className="t-card" key={item.ref}>
            <div className="aux"><span className="data">{formatReads(item.views.value)}</span> 阅读 · {item.account?.name ?? '账号未登记'} · {item.businessDate}</div>
            <div className="t-adapts"><div className="t-adapt">
              <span>{item.title}</span><span className={`count${size(item.title ?? '') > COVER_MAX ? ' over' : ''}`}>{size(item.title ?? '')} 字</span>
              <ActionButton variant="quiet" disabled={usingLocked || !onUseOurTitle} aria-label={`用作封面：${item.title}`} onClick={() => onUseOurTitle?.('cover', item.title ?? '')}>用作封面</ActionButton>
              <ActionButton variant="quiet" disabled={usingLocked || !onUseOurTitle} aria-label={`用作标题：${item.title}`} onClick={() => onUseOurTitle?.('note', item.title ?? '')}>用作标题</ActionButton>
            </div></div>
          </div>) : <p className="aux">运营台账里还没有爆款记录。</p>}
      <p className="aux">用我方爆款的原标题就算“复用”：同标题，正文小改。</p>
    </> : <>
    <div className="skill-box">
      <div className="row">
        <span className="chip ok">标题 skill</span>
        <span className="grow" style={{ flex: 1 }} />
        {tools ? <ActionButton variant="primary" className="sm" disabled={!tools.canGenerate} onClick={tools.generate}>{hasCandidates ? '换一批' : '生成标题候选'}</ActionButton> : null}
      </div>
      <span className="lbl">选题意图</span>
      <div className="paper" style={{ padding: '8px 10px', fontSize: 13 }}>{document.angle || '还没写“这篇想说什么”。'}</div>
      {tools?.running ? <p className="aux" role="status">正在生成标题…</p> : null}
      {tools?.message ? <p className="aux" role="alert">{tools.message}</p> : null}
    </div>
    {tools?.cover?.length ? <div className="f-group"><h4>封面大字候选</h4>{tools.cover.map((item, index) => line('cover', item, index, COVER_MAX))}</div> : null}
    {tools?.note?.length ? <div className="f-group"><h4>笔记标题候选</h4>{tools.note.map((item, index) => line('note', item, index, NOTE_MAX))}</div> : null}
    {tools && !hasCandidates && !tools.running ? <p className="aux">还没有候选。点“生成标题候选”，会先让你确认交给模型的内容。</p> : null}
    </>}
    <p className="aux"><a href="/settings/models?section=skills&amp;task=creation_title">管理标题 skill ↗</a></p>
  </>;
}
