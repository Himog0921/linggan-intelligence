import { ActionButton } from '../../shared/ActionButton';
import type { WorkDocument } from "./workbenchModel";
import styles from "./WorkbenchPage.module.css";
export function ManuscriptEditor({
  document,
  edit,
  compose,
  isComposing,
  textSelection,
  tagsDraft,
  onTagsChange,
}: {
  document: WorkDocument;
  edit: (document: WorkDocument) => void;
  compose: (value: boolean) => void;
  isComposing: () => boolean;
  textSelection: (id: string, target: HTMLTextAreaElement) => void;
  tagsDraft: string;
  onTagsChange: (value: string) => void;
}) {
  return (
    <div className={styles.manuscript}>
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
      {document.paragraphs.map((p, i) => (
        <div key={p.id} className={styles.paragraph}>
          <textarea
            aria-label={`正文第 ${i + 1} 段`}
            placeholder="写下这一段…"
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
