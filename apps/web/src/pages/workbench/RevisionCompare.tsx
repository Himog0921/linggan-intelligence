import { useMemo, useRef, useState, type ReactNode } from "react";
import styles from "./WorkbenchPage.module.css";
import { compareText } from "./revisionDiff";
import { ActionButton } from "../../shared/ActionButton";
export interface RevisionPair {
  id: string;
  before: string;
  after: string;
  label: string;
}
export function RevisionCompare({
  pairs,
  stale,
  disabled,
  onAdopt,
  onClose,
  onUndo,
  canUndo = false,
  draftTexts,
  onDraftTexts,
}: {
  pairs: RevisionPair[];
  stale: boolean;
  disabled: boolean;
  onAdopt: (indices: number[] | null, values: string[]) => void;
  onClose?: () => void;
  onUndo?: () => void;
  canUndo?: boolean;
  draftTexts?: string[] | null;
  onDraftTexts?: (texts: string[]) => void;
}) {
  const [localDrafts, setLocalDrafts] = useState(() =>
    pairs.map((pair) => pair.after),
  );
  const drafts = draftTexts ?? localDrafts;
  function setDrafts(next: (texts: string[]) => string[]) {
    const result = next(drafts);
    setLocalDrafts(result);
    onDraftTexts?.(result);
  }
  const [composing, setComposing] = useState(false);
  const [editing, setEditing] = useState(true),
    [showDiff, setShowDiff] = useState(true),
    [sync, setSync] = useState(true);
  const beforeRef = useRef<HTMLDivElement>(null),
    afterRef = useRef<HTMLDivElement>(null),
    syncing = useRef(false);
  function scroll(from: HTMLDivElement, to: HTMLDivElement | null) {
    if (!sync || !to || syncing.current) return;
    syncing.current = true;
    to.scrollTop =
      (from.scrollTop / Math.max(1, from.scrollHeight - from.clientHeight)) *
      Math.max(0, to.scrollHeight - to.clientHeight);
    requestAnimationFrame(() => {
      syncing.current = false;
    });
  }
  const comparisons = useMemo(
    () => pairs.map((pair, index) => compareText(pair.before, drafts[index] ?? "")),
    [pairs, drafts],
  );
  function markup(index: number, side: "before" | "after"): ReactNode {
    return (comparisons[index]?.parts ?? [])
      .filter((part) => part.type !== (side === "before" ? "insert" : "delete"))
      .map((part, partIndex) =>
        !showDiff || part.type === "same" ? (
          part.text
        ) : part.type === "delete" ? (
          <del key={partIndex}>{part.text}</del>
        ) : (
          <ins key={partIndex}>{part.text}</ins>
        ),
      );
  }
  const delta = comparisons.flatMap((comparison) => comparison.parts);
  const coarse = comparisons.some((comparison) => comparison.coarse);
  return (
    <div className={styles.revisionWorkspace}>
      <div className={styles.revisionToolbar}>
        <div className={styles.row}>
          <label>
            <input
              type="checkbox"
              checked={showDiff}
              onChange={(event) => setShowDiff(event.target.checked)}
            />
            显示改动
          </label>
          <label>
            <input
              type="checkbox"
              checked={sync}
              onChange={(event) => setSync(event.target.checked)}
            />
            联动滚动
          </label>
        </div>
        <ActionButton onClick={() => setEditing(!editing)}>
          {editing ? "完成编辑，查看差异" : "编辑改稿"}
        </ActionButton>
      </div>
      {stale ? (
        <p className={styles.warning}>
          底稿已有新编辑，当前对照已过期；请返回正文重新建立对照。
        </p>
      ) : null}
      {coarse ? <p role="status" className={styles.muted}>部分段落改动较大，按整段标记；两侧文字均完整保留。</p> : null}
      <div className={styles.revisionPair}>
        <section className={styles.revisionPanel}>
          <header>
            <h2>原稿</h2>
            <small>只读</small>
          </header>
          <div
            className={styles.revisionText}
            ref={beforeRef}
            aria-label="原稿差异视图"
            onScroll={(event) => scroll(event.currentTarget, afterRef.current)}
          >
            {pairs.map((pair, index) => (
              <section className={styles.revisionParagraph} key={pair.id}>
                <small>{pair.label}</small>
                <p>{markup(index, "before")}</p>
              </section>
            ))}
          </div>
          <footer>原文保留至采用</footer>
        </section>
        <section className={styles.revisionPanel}>
          <header>
            <h2>改稿</h2>
            <small>可编辑</small>
          </header>
          <div
            className={styles.revisionText}
            ref={afterRef}
            aria-label="改稿差异视图"
            onScroll={(event) => scroll(event.currentTarget, beforeRef.current)}
          >
            {pairs.map((pair, index) => (
              <section className={styles.revisionParagraph} key={pair.id}>
                <small>{pair.label}</small>
                {editing ? (
                  <textarea
                    aria-label={`编辑改稿 ${pair.label}`}
                    value={drafts[index] ?? ""}
                    onCompositionStart={() => setComposing(true)}
                    onCompositionEnd={() => setComposing(false)}
                    onChange={(event) =>
                      setDrafts((old) =>
                        old.map((text, i) =>
                          i === index ? event.target.value : text,
                        ),
                      )
                    }
                  />
                ) : (
                  <p>{markup(index, "after")}</p>
                )}
                <ActionButton
                  disabled={disabled || stale || composing}
                  onClick={() => onAdopt([index], drafts)}
                >
                  采用这一段
                </ActionButton>
              </section>
            ))}
          </div>
          <footer>
            {coarse ? "整段标记 · " : ""}新增{" "}
            {delta
              .filter((part) => part.type === "insert")
              .reduce(
                (sum, part) => sum + Array.from(part.text).length,
                0,
              )}{" "}
            字 · 删去{" "}
            {delta
              .filter((part) => part.type === "delete")
              .reduce(
                (sum, part) => sum + Array.from(part.text).length,
                0,
              )}{" "}
            字
          </footer>
        </section>
      </div>
      <div className={styles.revisionActions}>
        <div className={styles.row}>
          {onClose ? <ActionButton variant="quiet" onClick={onClose}>返回正文</ActionButton> : null}
          {onUndo ? (
            <ActionButton disabled={!canUndo || disabled} onClick={onUndo}>
              撤销上次采用
            </ActionButton>
          ) : null}
        </div>
        <ActionButton
          variant="primary"
          disabled={disabled || stale || composing}
          onClick={() => onAdopt(null, drafts)}
        >
          采用整篇改稿
        </ActionButton>
      </div>
    </div>
  );
}
