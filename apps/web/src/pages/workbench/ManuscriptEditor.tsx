import { ActionButton } from '../../shared/ActionButton';
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
}) {
  const cover = document.pages.find(page => page.kind === 'cover');
  return (
    <div className={styles.manuscript}>
      <div className={styles.titleArea}>
        {cover?.assetId && workId ? <button type="button" className={styles.coverPreview} onClick={onCover} aria-label="查看封面">
          <img src={`/ops/api/creation/works/${encodeURIComponent(workId)}/assets/${encodeURIComponent(cover.assetId)}`} alt="" />
        </button> : null}
        <div className={styles.titleFields}>
          {cover ? <label>封面大字<input aria-label="封面大字" value={cover.title} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => edit({ ...document,
            pages: document.pages.map(page => page.id === cover.id ? { ...page, title: event.target.value } : page) })} /></label>
            : <ActionButton variant="quiet" onClick={onCover}>设置封面大字</ActionButton>}
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
        </div>
      </div>
      <div className={styles.manuscriptHeading}><strong>正文</strong><small>{Array.from(document.paragraphs.map(p => p.text).join('')).length} 字 · {document.paragraphs.length} 段</small></div>
      {document.paragraphs.map((p, i) => (
        <div key={p.id} className={styles.paragraph}>
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
