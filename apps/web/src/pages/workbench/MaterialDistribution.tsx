import { useEffect, useRef, useState } from "react";
import { ActionButton } from "../../shared/ActionButton";
import type { MaterialDocument } from "./workbenchModel";
import {
  FRAGMENT_TYPES,
  type FragmentType,
  type ReadingPiece,
} from "./materialReading";
import styles from "./WorkbenchPage.module.css";
export function MaterialDistribution({
  documents,
  pieces,
  visible,
  selectedId,
  currentDocumentId,
  countsReady,
  onChoose,
  onOpenDocument,
}: {
  documents: MaterialDocument[];
  pieces: ReadingPiece[];
  visible: ReadingPiece[];
  selectedId: string | null;
  currentDocumentId: string | null;
  countsReady: boolean;
  onChoose: (piece: ReadingPiece) => void;
  onOpenDocument: (documentId: string) => void;
}) {
  const [openCell, setOpenCell] = useState<{
    documentId: string;
    type: FragmentType;
  } | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const selectedPieces = openCell
    ? visible.filter(
        (piece) =>
          piece.documentId === openCell.documentId &&
          piece.type === openCell.type,
      )
    : [];
  useEffect(() => {
    if (!openCell || !selectedPieces.length) return;
    const target = dialog.current,
      previous = document.activeElement;
    target?.showModal();
    return () => {
      target?.close();
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus();
    };
  }, [openCell, selectedPieces.length]);
  const columns = {
    gridTemplateColumns: `var(--lgi-space-10) repeat(${Math.max(1, documents.length)},minmax(var(--lgi-space-6),1fr))`,
  };
  return (
    <>
      <nav className={styles.distribution} aria-label="材料分布索引">
        <div className={styles.matrixHeading}>
          <small>片段分布</small>
          <small>
            {countsReady ? visible.length : "—"} 个片段 ·{" "}
            {countsReady ? documents.length : "—"} 份材料
          </small>
        </div>
        <div className={styles.matrixRow} style={columns}>
          <span />
          {documents.map((document, index) => (
            <button
              key={document.documentId}
              title={document.title}
              aria-pressed={document.documentId === currentDocumentId}
              onClick={() => onOpenDocument(document.documentId)}
            >
              {String(index + 1).padStart(2, "0")}
            </button>
          ))}
        </div>
        {FRAGMENT_TYPES.map((type) => (
          <div className={styles.matrixRow} key={type} style={columns}>
            <span>{type}</span>
            {documents.map((document) => {
              const all = pieces.filter(
                  (piece) => piece.documentId === document.documentId,
                ),
                matching = visible.filter(
                  (piece) =>
                    piece.documentId === document.documentId &&
                    piece.type === type,
                ),
                active = matching.some((piece) => piece.id === selectedId);
              return (
                <div className={styles.matrixCell} key={document.documentId}>
                  {matching.map((piece) => (
                    <span
                      key={piece.id}
                      className={styles.matrixTick}
                      data-piece-id={piece.id}
                      data-selected={piece.id === selectedId}
                      aria-hidden="true"
                      style={{
                        left: `clamp(var(--lgi-space-1),${((all.findIndex((item) => item.id === piece.id) + 0.5) / Math.max(1, all.length)) * 100}%,calc(100% - var(--lgi-space-1)))`,
                      }}
                    >
                      <i />
                    </span>
                  ))}
                  {matching.length ? (
                    <button
                      className={styles.matrixCellButton}
                      aria-label={
                        matching.length === 1
                          ? `定位：${matching[0]!.text}`
                          : `展开${document.title}的${type}片段（${matching.length}）`
                      }
                      aria-current={active ? "location" : undefined}
                      aria-expanded={
                        matching.length > 1
                          ? openCell?.documentId === document.documentId &&
                            openCell.type === type
                          : undefined
                      }
                      onClick={() =>
                        matching.length === 1
                          ? onChoose(matching[0]!)
                          : setOpenCell({
                              documentId: document.documentId,
                              type,
                            })
                      }
                    />
                  ) : null}
                </div>
              );
            })}
          </div>
        ))}
      </nav>
      {openCell && selectedPieces.length ? (
        <dialog
          ref={dialog}
          className={styles.dialog}
          aria-label="选择分布片段"
          onCancel={(event) => {
            event.preventDefault();
            setOpenCell(null);
          }}
        >
          <div className={styles.dialogHead}>
            <h2>
              {
                documents.find(
                  (document) => document.documentId === openCell.documentId,
                )?.title
              }{" "}
              · {openCell.type}
            </h2>
            <ActionButton variant="quiet" onClick={() => setOpenCell(null)}>
              关闭片段列表
            </ActionButton>
          </div>
          <ol className={styles.matrixPieceList}>
            {selectedPieces.map((piece) => (
              <li key={piece.id}>
                <button
                  aria-label={`定位：${piece.text}`}
                  aria-current={
                    piece.id === selectedId ? "location" : undefined
                  }
                  onClick={() => {
                    onChoose(piece);
                    setOpenCell(null);
                  }}
                >
                  {piece.text}
                </button>
              </li>
            ))}
          </ol>
        </dialog>
      ) : null}
    </>
  );
}
