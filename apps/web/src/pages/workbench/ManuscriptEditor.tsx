import { ActionButton } from '../../shared/ActionButton';
import { IdeaBoxLink } from './IdeaBoxLink';
import { newImagePage } from './imagePageFactory';
import type { ReactNode } from 'react';
import type { IdeaBoxSummary } from './workbenchModel';
import type { WorkDocument } from "./workbenchModel";
import { ExperiencePicker } from './assets/ExperiencePicker';
export function ManuscriptEditor({
  document,
  edit,
  compose,
  isComposing,
  textSelection,
  tagsDraft,
  onTagsChange,
  workId,
  onCover,
  accountName,
  ideas = [], ideaState = 'success', onOpenAnchor = () => {}, draftInputs, onDraftInputs, titleGenerator, footer, tools,
}: {
  document: WorkDocument;
  edit: (document: WorkDocument) => void;
  compose: (value: boolean) => void;
  isComposing: () => boolean;
  textSelection: (id: string, target: HTMLTextAreaElement) => void;
  tagsDraft: string;
  onTagsChange: (value: string) => void;
  workId?: string;
  onCover?: () => void;
  accountName?: string;
  ideas?: IdeaBoxSummary[]; ideaState?: 'pending' | 'error' | 'success'; onOpenAnchor?: (anchor: string) => void;
  draftInputs: { cover: string; note: string; idea: string };
  onDraftInputs: (inputs: { cover: string; note: string; idea: string }) => void;
  titleGenerator?: ReactNode;
  footer?: ReactNode;
  tools?: ReactNode;
}) {
  const cover = document.pages.find(page => page.kind === 'cover');
  const coverCandidate = draftInputs.cover;
  const noteCandidate = draftInputs.note;
  const tags = tagsDraft.split(/\s+/u).filter(Boolean);
  const charCount = Array.from(document.paragraphs.map(p => p.text).join('')).length;
  function setCoverTitle(title: string) {
    if (cover) edit({ ...document, pages: document.pages.map(page => page.id === cover.id ? { ...page, title } : page) });
    else if (document.pages.length < 30) edit({ ...document, pages: [{ ...newImagePage('cover'), title }, ...document.pages] });
  }
  function addCandidate(kind: 'cover' | 'note') {
    const value = (kind === 'cover' ? coverCandidate : noteCandidate).trim();
    const candidates = kind === 'cover' ? document.coverTitleCandidates : document.noteTitleCandidates;
    if (!value || candidates.includes(value) || candidates.length >= 20 || Array.from(value).length > (kind === 'cover' ? 10 : 200)) return;
    edit(kind === 'cover' ? { ...document, coverTitleCandidates: [...candidates, value] } : { ...document, noteTitleCandidates: [...candidates, value] });
    onDraftInputs({ ...draftInputs, [kind]: '' });
  }
  return <section className="editor" aria-label="正文编辑">
    <div className="ed-head">
      <span className="acc-btn"><span className="avatar">{Array.from(accountName ?? '未')[0]}</span>{accountName ?? (document.accountRef ? '账号读取中' : '未选择账号')}</span>
      {document.ideaName || document.ideaRef ? null : <span className="frame-chip ghost">没有选题框</span>}
      <span className="grow" />
      {tools}
      <div className="ed-meta" aria-label="稿件信息">
        <span className="meta-basic">{document.angle ? `意图 · ${document.angle}` : '还没有选题意图'}</span>
      </div>
    </div>
    <div className="ed-scroll">
      <div className="ed-inner">
        <IdeaBoxLink document={document} ideas={ideas} state={ideaState} edit={edit} openAnchor={onOpenAnchor} compose={compose} name={draftInputs.idea} onNameChange={idea => onDraftInputs({ ...draftInputs, idea })} />
        <div className="cover-row">
          <div>
            {cover?.assetId && workId ? <button type="button" className="cover" onClick={onCover} aria-label="查看封面">
              <img src={`/ops/api/creation/works/${encodeURIComponent(workId)}/assets/${encodeURIComponent(cover.assetId)}`} alt="" />
              <span className="cover-title">{cover.title || '未写封面大字'}</span>
            </button> : <button type="button" className="cover empty" onClick={onCover} aria-label="设置封面"><span className="cover-title">设置封面</span></button>}
          </div>
          <div className="cover-fields">
            <div>
              <label className="lbl" htmlFor="studio-cover-title">封面大字 <span className="count">{Array.from(cover?.title ?? '').length}/10</span></label>
              <div className="cover-entry">
                <input className="input" id="studio-cover-title" aria-label="封面大字" value={cover?.title ?? ''} disabled={!cover && document.pages.length >= 30} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => setCoverTitle(event.target.value)} />
                <span className="cand-tools"><ActionButton variant="quiet" onClick={onCover}>设置封面</ActionButton></span>
              </div>
              <div className="cands" aria-label="封面大字候选">
                {document.coverTitleCandidates.map((candidate, index) => <span key={`${index}-${candidate}`}><button type="button" className="cand" disabled={!cover && document.pages.length >= 30} aria-pressed={candidate === cover?.title} onClick={() => setCoverTitle(candidate)}>{candidate} <span className="count">{Array.from(candidate).length}</span></button><button type="button" aria-label={`删除封面候选 ${candidate}`} onClick={() => edit({ ...document, coverTitleCandidates: document.coverTitleCandidates.filter((_, i) => i !== index) })}>×</button></span>)}
                <details><summary>添加封面候选</summary><div className="field-row"><input className="input" aria-label="新封面候选" value={coverCandidate} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => onDraftInputs({ ...draftInputs, cover: event.target.value })} placeholder="手动添加，最多 10 字" /><ActionButton onClick={() => addCandidate('cover')} disabled={!coverCandidate.trim() || Array.from(coverCandidate.trim()).length > 10 || document.coverTitleCandidates.length >= 20}>添加候选</ActionButton></div></details>
              </div>
            </div>
            <div>
              <label className="lbl" htmlFor="studio-note-title">笔记标题 <span className="count">{Array.from(document.title).length}/20</span></label>
              <div className="field-row">
                <input className="input" id="studio-note-title" aria-label="正文标题" placeholder="标题" value={document.title} onChange={e => edit({ ...document, title: e.target.value })} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} />
              </div>
              <div className="cands" aria-label="笔记标题候选">
                {document.noteTitleCandidates.map((candidate, index) => <span key={`${index}-${candidate}`}><button type="button" className="cand" aria-pressed={candidate === document.title} onClick={() => edit({ ...document, title: candidate })}>{candidate}</button><button type="button" aria-label={`删除标题候选 ${candidate}`} onClick={() => edit({ ...document, noteTitleCandidates: document.noteTitleCandidates.filter((_, i) => i !== index) })}>×</button></span>)}
                <details><summary>添加笔记标题候选</summary><div className="field-row"><input className="input" aria-label="新标题候选" value={noteCandidate} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => onDraftInputs({ ...draftInputs, note: event.target.value })} placeholder="手动添加标题候选" /><ActionButton onClick={() => addCandidate('note')} disabled={!noteCandidate.trim() || Array.from(noteCandidate.trim()).length > 200 || document.noteTitleCandidates.length >= 20}>添加候选</ActionButton></div></details>
              </div>
            </div>
          </div>
        </div>
        {titleGenerator}
        <div className="body-head"><h3 className="h-sec">正文</h3><span className="aux">约 <b>{charCount}</b> 字 · <b>{document.paragraphs.length}</b> 段{document.experienceSlots.length ? <> · 来自经历库 <b>{document.experienceSlots.length}</b> 段</> : null}</span></div>
        <div className="paras">
          {document.paragraphs.map((p, i) => (
            <div key={p.id} className="para">
              <div className="para-label" title={`第 ${i + 1} 段`}>{i + 1}</div>
              <div className="para-main">
                <textarea
                  className="para-text"
                  aria-label={`正文第 ${i + 1} 段`}
                  placeholder="写下这一段…"
                  readOnly={document.experienceGaps.includes(p.id)}
                  value={p.text}
                  onCompositionStart={() => compose(true)}
                  onCompositionEnd={() => compose(false)}
                  onChange={(e) => edit({ ...document, paragraphs: document.paragraphs.map((x) => x.id === p.id ? { ...x, text: e.target.value } : x) })}
                  onSelect={(e) => textSelection(p.id, e.currentTarget)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing && !isComposing()) {
                      // Keep edits to an experience together until a source-aware split operation exists.
                      if (document.experienceSlots.some(slot => slot.paragraphId === p.id)) return;
                      e.preventDefault();
                      const t = e.currentTarget;
                      const before = p.text.slice(0, t.selectionStart), after = p.text.slice(t.selectionEnd);
                      const next = { id: crypto.randomUUID(), text: after };
                      edit({
                        ...document,
                        derivedSources: [...document.derivedSources, ...document.derivedSources
                          .filter(source => source.target.kind === 'paragraph' && source.target.paragraphId === p.id)
                          .map(source => ({ ...source, target: { kind: 'paragraph' as const, paragraphId: next.id } }))],
                        paragraphs: document.paragraphs.flatMap((x) => x.id === p.id ? [{ ...x, text: before }, next] : [x]),
                      });
                      window.setTimeout(() => window.document.querySelector<HTMLTextAreaElement>(`[data-paragraph="${next.id}"]`)?.focus(), 0);
                    }
                  }}
                  data-paragraph={p.id}
                />
                <div className="para-meta">
                  {document.experienceSlots.some(slot => slot.paragraphId === p.id) ? <span className="src exp"><span className="dot ok" />来源：经历库 · 使用前重新核验</span> : null}
                  <span className="para-tools">
                    <details>
                      <summary>段落与来源</summary>
                      <ActionButton variant="danger" onClick={() => edit({
                        ...document,
                        paragraphs: document.paragraphs.filter((x) => x.id !== p.id),
                        experienceSlots: document.experienceSlots.filter(slot => slot.paragraphId !== p.id),
                        experienceGaps: document.experienceGaps.filter(id => id !== p.id),
                        derivedSources: document.derivedSources.filter(source => source.target.kind !== 'paragraph' || source.target.paragraphId !== p.id),
                        uses: document.uses.filter((u) => u.target.kind !== "paragraph" || u.target.paragraphId !== p.id),
                      })}>删除这一段</ActionButton>
                      {document.derivedSources.some(source => source.target.kind === 'paragraph' && source.target.paragraphId === p.id) ? <p>已记录参考来源；来源不可用时，本段会暂时隐藏。</p> : null}
                      {document.uses.filter((u) => u.target.kind === "paragraph" && u.target.paragraphId === p.id).map((u) => <p key={u.id}>{u.contentOrigin === "expression" ? "我的表达" : "原文摘录"} · {u.state === "current" ? "当前使用" : u.state === "modified" ? "已手动修改" : "需要核验"}</p>)}
                    </details>
                  </span>
                </div>
              </div>
            </div>
          ))}
        </div>
        <button type="button" className="add-para" onClick={() => edit({ ...document, paragraphs: [...document.paragraphs, { id: crypto.randomUUID(), text: "" }] })}>＋ 继续写一段</button>
        <ExperiencePicker document={document} edit={edit} />
        <div className="tags-row">
          {tags.map((tag, index) => <span key={`${tag}-${index}`} className={tags.indexOf(tag) !== index ? "tag dup" : "tag"}>#{tag}<button type="button" aria-label={`删除话题 #${tag}`} onClick={() => onTagsChange(tags.filter((_, i) => i !== index).join(' '))}>×</button></span>)}
          <input className="tag-add" aria-label="确认发布标签" placeholder="＋ 话题，空格分隔" value={tagsDraft} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={(e) => onTagsChange(e.target.value)} />
        </div>
      </div>
    </div>
    {footer}
  </section>;
}
