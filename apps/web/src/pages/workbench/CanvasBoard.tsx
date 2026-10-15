import { ActionButton } from '../../shared/ActionButton';
import type {
  CanvasBlock,
  CanvasGroup,
  Citation,
  UseTarget,
  WorkDocument,
} from "./workbenchModel";
import { GROUPS, newCanvasBlock } from "./workbenchModel";
import styles from "./WorkbenchPage.module.css";
export function CanvasBoard({
  document,
  busy,
  onCompose,
  onChange,
  onUpdate,
  onLocate,
  onPlace,
  onMaterials,
  onUse,
}: {
  document: WorkDocument;
  busy: boolean;
  onCompose: (value: boolean) => void;
  onChange: (document: WorkDocument) => void;
  onUpdate: (id: string, patch: Partial<CanvasBlock>) => void;
  onLocate: (citation: Citation) => void;
  onPlace: (card: CanvasBlock) => void;
  onMaterials: () => void;
  onUse: (target: UseTarget) => { label: string; open: () => void };
}) {
  return (
    <>
      <div className={styles.canvasTools}>
        <span>{document.canvas.length} 张材料卡</span>
        <ActionButton variant="outline"
          onClick={() =>
            onChange({
              ...document,
              canvas: [...document.canvas, newCanvasBlock()],
            })
          }
        >
          ＋ 写一张自己的想法
        </ActionButton>
        <ActionButton variant="quiet" onClick={onMaterials}>继续取材 →</ActionButton>
      </div>
      <div className={styles.canvasBoard}>
        {GROUPS.map((group) => (
          <section key={group.id} className={styles.canvasLane}>
            <h2>
              {group.label}
              <small>
                {
                  document.canvas.filter((card) => card.group === group.id)
                    .length
                }
              </small>
            </h2>
            {document.canvas
              .filter((card) => card.group === group.id)
              .map((card) => {
                const excerpt = document.excerpts.find(
                  (item) => item.id === card.excerptId,
                );
                return (
                  <article className={styles.canvasCard} key={card.id}>
                    {excerpt ? (
                      <>
                        <small>来源原文</small>
                        <blockquote className={styles.canvasQuote}>
                          {excerpt.selectedText ?? "来源已受限或不可用"}
                        </blockquote>
                        {excerpt.selectedText &&
                        Array.from(excerpt.selectedText).length > 100 ? (
                          <details className={styles.fullQuote}>
                            <summary>展开完整引文</summary>
                            <blockquote>{excerpt.selectedText}</blockquote>
                          </details>
                        ) : null}
                        <ActionButton variant="quiet" onClick={() => onLocate(excerpt.citation)}>
                          回到出处 ↗
                        </ActionButton>
                      </>
                    ) : (
                      <small>自己的想法</small>
                    )}
                    <label>
                      我的表达
                      <textarea
                        aria-label={`我的表达 ${card.id}`}
                        value={card.expression}
                        onCompositionStart={() => onCompose(true)}
                        onCompositionEnd={() => onCompose(false)}
                        onChange={(event) =>
                          onUpdate(card.id, { expression: event.target.value })
                        }
                      />
                    </label>
                    <details className={styles.cardNotes}>
                      <summary>备注与整理</summary>
                      <label>
                        备注
                        <input
                          value={card.note}
                          onChange={(event) =>
                            onUpdate(card.id, { note: event.target.value })
                          }
                        />
                      </label>
                      <select
                        aria-label="材料归组"
                        value={card.group}
                        onChange={(event) =>
                          onUpdate(card.id, {
                            group: event.target.value as CanvasGroup,
                          })
                        }
                      >
                        {GROUPS.map((item) => (
                          <option key={item.id} value={item.id}>
                            {item.label}
                          </option>
                        ))}
                      </select>
                    </details>
                    <div className={styles.useLinks}>
                      {document.uses
                        .filter((use) => use.canvasId === card.id)
                        .map((use) => {
                          const location = onUse(use.target);
                          return (
                            <ActionButton variant="outline" key={use.id} onClick={location.open}>
                              {location.label} ·{" "}
                              {use.state === "current"
                                ? "已放入"
                                : use.state === "modified"
                                  ? "已手改"
                                  : "需核验"}
                            </ActionButton>
                          );
                        })}
                    </div>
                    <ActionButton variant="outline"
                      className={styles.canvasAction}
                      disabled={
                        busy ||
                        (!card.expression.trim() && !excerpt?.selectedText)
                      }
                      onClick={() => onPlace(card)}
                    >
                      放入正文 / 指定图页 →
                    </ActionButton>
                    <ActionButton variant="outline"
                      onClick={() =>
                        onChange({
                          ...document,
                          canvas: document.canvas.filter(
                            (item) => item.id !== card.id,
                          ),
                          uses: document.uses.filter(
                            (use) => use.canvasId !== card.id,
                          ),
                        })
                      }
                    >
                      移除卡片
                    </ActionButton>
                  </article>
                );
              })}
          </section>
        ))}
      </div>
    </>
  );
}
