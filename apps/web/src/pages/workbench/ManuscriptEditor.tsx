import { ActionButton } from '../../shared/ActionButton';
import { IdeaBoxLink } from './IdeaBoxLink';
import { newImagePage } from './imagePageFactory';
import type { ReactNode } from 'react';
import type { IdeaBoxSummary } from './workbenchModel';
import type { WorkDocument } from "./workbenchModel";
import styles from "./WorkbenchPage.module.css";
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
  onTopic,
  accountName,
  ideas = [], ideaState = 'success', onOpenTopic = () => {}, draftInputs, onDraftInputs, titleGenerator,
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
  onTopic?: () => void;
  accountName?: string;
  ideas?: IdeaBoxSummary[]; ideaState?: 'pending' | 'error' | 'success'; onOpenTopic?: (anchor: string) => void;
  draftInputs: { cover: string; note: string; idea: string };
  onDraftInputs: (inputs: { cover: string; note: string; idea: string }) => void;
  titleGenerator?: ReactNode;
}) {
  const cover = document.pages.find(page => page.kind === 'cover');
  const coverCandidate = draftInputs.cover;
  const noteCandidate = draftInputs.note;
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
  return (
    <div className={styles.manuscript}>
      <div className={styles.manuscriptContext}>
        <strong>{accountName ?? (document.accountRef ? '账号读取中' : '未选择账号')}</strong>
        <ActionButton variant="quiet" disabled={!!document.ideaRef && document.ideaName === null && !ideas.some(idea => idea.ideaRef === document.ideaRef)} onClick={onTopic}>{document.ideaRef && document.ideaName === null ? '查看选题方案' : document.angle ? `选题角度：${document.angle}` : '补充选题角度'}</ActionButton>
      </div>
      <IdeaBoxLink document={document} ideas={ideas} state={ideaState} edit={edit} openTopic={onOpenTopic} compose={compose} name={draftInputs.idea} onNameChange={idea => onDraftInputs({ ...draftInputs, idea })} />
      <div className={styles.titleArea}>
        {cover?.assetId && workId ? <button type="button" className={styles.coverPreview} onClick={onCover} aria-label="查看封面">
          <img src={`/ops/api/creation/works/${encodeURIComponent(workId)}/assets/${encodeURIComponent(cover.assetId)}`} alt="" />
        </button> : <button type="button" className={`${styles.coverPreview} ${styles.emptyCover}`} onClick={onCover} aria-label="设置封面"><span>设置封面</span></button>}
        <div className={styles.titleFields}>
          <label>封面大字<input aria-label="封面大字" value={cover?.title ?? ''} disabled={!cover && document.pages.length >= 30} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => setCoverTitle(event.target.value)} /></label>
          {titleGenerator}
          <div className={styles.titleCandidates} aria-label="封面大字候选">
            <div className={styles.candidateList}>{document.coverTitleCandidates.map((candidate, index) => <span key={`${index}-${candidate}`}><button type="button" disabled={!cover && document.pages.length >= 30} aria-pressed={candidate === cover?.title} onClick={() => setCoverTitle(candidate)}>{candidate}</button><button type="button" aria-label={`删除封面候选 ${candidate}`} onClick={() => edit({ ...document, coverTitleCandidates: document.coverTitleCandidates.filter((_, i) => i !== index) })}>×</button></span>)}</div>
            <details><summary>添加封面候选</summary><div className={styles.candidateInput}><input aria-label="新封面候选" value={coverCandidate} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => onDraftInputs({ ...draftInputs, cover: event.target.value })} placeholder="手动添加，最多 10 字" /><ActionButton onClick={() => addCandidate('cover')} disabled={!coverCandidate.trim() || Array.from(coverCandidate.trim()).length > 10 || document.coverTitleCandidates.length >= 20}>添加候选</ActionButton></div></details>
          </div>
          <label>笔记标题
      <input
        className={styles.manuscriptTitle}
        aria-label="正文标题"
        placeholder="标题"
        value={document.title}
        onChange={(e) =>
          edit({
            ...document,
            title: e.target.value,
          })
        }
        onCompositionStart={() => compose(true)}
        onCompositionEnd={() => compose(false)}
      />
          </label>
          <div className={styles.titleCandidates} aria-label="笔记标题候选">
            <div className={styles.candidateList}>{document.noteTitleCandidates.map((candidate, index) => <span key={`${index}-${candidate}`}><button type="button" aria-pressed={candidate === document.title} onClick={() => edit({ ...document, title: candidate })}>{candidate}</button><button type="button" aria-label={`删除标题候选 ${candidate}`} onClick={() => edit({ ...document, noteTitleCandidates: document.noteTitleCandidates.filter((_, i) => i !== index) })}>×</button></span>)}</div>
            <details><summary>添加笔记标题候选</summary><div className={styles.candidateInput}><input aria-label="新标题候选" value={noteCandidate} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => onDraftInputs({ ...draftInputs, note: event.target.value })} placeholder="手动添加标题候选" /><ActionButton onClick={() => addCandidate('note')} disabled={!noteCandidate.trim() || Array.from(noteCandidate.trim()).length > 200 || document.noteTitleCandidates.length >= 20}>添加候选</ActionButton></div></details>
          </div>
        </div>
      </div>
      <div className={styles.manuscriptHeading}><strong>正文</strong><small>{Array.from(document.paragraphs.map(p => p.text).join('')).length} 字 · {document.paragraphs.length} 段</small></div>
      {document.paragraphs.map((p, i) => (
        <div key={p.id} className={styles.paragraph}>
          <span className={styles.paragraphIndex}>第 {i + 1} 段</span>
          {document.experienceSlots.some(slot => slot.paragraphId === p.id) ? <p className={styles.sourceLabel}>来源：经历库 · 使用前重新核验</p> : null}
          <textarea
            aria-label={`正文第 ${i + 1} 段`}
            placeholder="写下这一段…"
            readOnly={document.experienceGaps.includes(p.id)}
            value={p.text}
            onCompositionStart={() => compose(true)}
            onCompositionEnd={() => compose(false)}
            onChange={(e) =>
              edit({
                ...document,
                paragraphs: document.paragraphs.map((x) =>
                  x.id === p.id
                    ? {
                        ...x,
                        text: e.target.value,
                      }
                    : x,
                ),
              })
            }
            onSelect={(e) => textSelection(p.id, e.currentTarget)}
            onKeyDown={(e) => {
              if (
                e.key === "Enter" &&
                !e.shiftKey &&
                !e.nativeEvent.isComposing &&
                !isComposing()
              ) {
                // Keep edits to an experience together until a source-aware split operation exists.
                if (document.experienceSlots.some(slot => slot.paragraphId === p.id)) return;
                e.preventDefault();
                const t = e.currentTarget;
                const before = p.text.slice(0, t.selectionStart),
                  after = p.text.slice(t.selectionEnd);
                const next = {
                  id: crypto.randomUUID(),
                  text: after,
                };
                edit({
                  ...document,
                  derivedSources: [...document.derivedSources, ...document.derivedSources
                    .filter(source => source.target.kind === 'paragraph' && source.target.paragraphId === p.id)
                    .map(source => ({ ...source, target: { kind: 'paragraph' as const, paragraphId: next.id } }))],
                  paragraphs: document.paragraphs.flatMap((x) =>
                    x.id === p.id
                      ? [
                          {
                            ...x,
                            text: before,
                          },
                          next,
                        ]
                      : [x],
                  ),
                });
                window.setTimeout(
                  () =>
                    window.document
                      .querySelector<HTMLTextAreaElement>(
                        `[data-paragraph="${next.id}"]`,
                      )
                      ?.focus(),
                  0,
                );
              }
            }}
            data-paragraph={p.id}
          />
          <details>
            <summary>段落与来源</summary>
            <ActionButton variant="danger"
              onClick={() =>
                edit({
                  ...document,
                  paragraphs: document.paragraphs.filter((x) => x.id !== p.id),
                  experienceSlots: document.experienceSlots.filter(slot => slot.paragraphId !== p.id),
                  experienceGaps: document.experienceGaps.filter(id => id !== p.id),
                  derivedSources: document.derivedSources.filter(source => source.target.kind !== 'paragraph' || source.target.paragraphId !== p.id),
                  uses: document.uses.filter(
                    (u) =>
                      u.target.kind !== "paragraph" ||
                      u.target.paragraphId !== p.id,
                  ),
                })
              }
            >
              删除这一段
            </ActionButton>
            {document.derivedSources.some(source => source.target.kind === 'paragraph' && source.target.paragraphId === p.id) ? <p>已记录参考来源；来源不可用时，本段会暂时隐藏。</p> : null}
            {document.uses
              .filter(
                (u) =>
                  u.target.kind === "paragraph" &&
                  u.target.paragraphId === p.id,
              )
              .map((u) => (
                <p key={u.id}>
                  {u.contentOrigin === "expression" ? "我的表达" : "原文摘录"} ·{" "}
                  {u.state === "current"
                    ? "当前使用"
                    : u.state === "modified"
                      ? "已手动修改"
                      : "需要核验"}
                </p>
              ))}
          </details>
        </div>
      ))}
      <ActionButton variant="outline"
        className={styles.addParagraph}
        onClick={() =>
          edit({
            ...document,
            paragraphs: [
              ...document.paragraphs,
              {
                id: crypto.randomUUID(),
                text: "",
              },
            ],
          })
        }
      >
        ＋ 继续写一段
      </ActionButton>
      <ExperiencePicker document={document} edit={edit} />
      <label className={styles.tags}>
        确认发布标签
        <input
          aria-label="确认发布标签"
          placeholder="以空格分隔"
          value={tagsDraft}
          onCompositionStart={() => compose(true)}
          onCompositionEnd={() => compose(false)}
          onChange={(e) => onTagsChange(e.target.value)}
        />
      </label>
    </div>
  );
}
