import { useState } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MaterialDistribution } from "./MaterialDistribution";
import { materialPieces, type ReadingPiece } from "./materialReading";
import type { MaterialDocument } from "./workbenchModel";
const document: MaterialDocument = {
  documentId: "note:one",
  sourceKind: "note",
  sourceRef: "one",
  workRef: "one",
  sourceVersion: "document1",
  title: "密集原篇",
  authorLabel: "作者",
  qualification: "readable",
  matchReasons: [],
  context: null,
  fragments: [
    {
      fragmentId: "one.body", asOf: "2026-10-23T08:00:00Z", blockRef: "one.body", speaker: "author", structure: null, issues: [],
      sourceKind: "note",
      sourceRef: "one",
      workRef: "one",
      field: "body",
      sourceVersion: "version1",
      title: "密集原篇",
      text: "一句。二句。三句。四句。五句。六句。七句。八句。",
      authorLabel: "作者",
      qualification: "readable",
      bodyState: "KNOWN",
      location: null,
    },
  ],
};
const pieces = materialPieces(document);
describe("dense distribution preserves every selectable fragment", () => {
  it("uses one full cell control and an accessible list for eight closely spaced pieces", () => {
    const chosen = vi.fn();
    function Harness() {
      const [id, setId] = useState<string | null>(null);
      return (
        <MaterialDistribution
          documents={[document]}
          pieces={pieces}
          visible={pieces}
          selectedId={id}
          currentDocumentId={document.documentId}
          countsReady
          onOpenDocument={vi.fn()}
          onChoose={(piece) => {
            setId(piece.id);
            chosen(piece);
          }}
        />
      );
    }
    render(<Harness />);
    const index = screen.getByRole("navigation", { name: "材料分布索引" });
    expect(index.querySelectorAll("[data-piece-id]")).toHaveLength(8);
    expect(
      within(index).queryAllByRole("button", { name: /^定位：/ }),
    ).toHaveLength(0);
    for (const piece of pieces) {
      fireEvent.click(
        within(index).getByRole("button", {
          name: "展开密集原篇的待分类片段（8）",
        }),
      );
      const dialog = screen.getByRole("dialog", { name: "选择分布片段" });
      expect(
        within(dialog).getAllByRole("button", { name: /^定位：/ }),
      ).toHaveLength(8);
      const button = within(dialog).getByRole("button", {
        name: `定位：${piece.text}`,
      });
      expect(button.tagName).toBe("BUTTON");
      fireEvent.click(button);
      expect(chosen).toHaveBeenLastCalledWith(piece);
      expect(
        index.querySelector(`[data-piece-id="${piece.id}"]`),
      ).toHaveAttribute("data-selected", "true");
    }
    expect(chosen).toHaveBeenCalledTimes(8);
    expect(index.querySelectorAll("[data-piece-id]")).toHaveLength(8);
  });
  it("keeps the expanded list and visual markers on the exact same filtered collection", () => {
    const visible: ReadingPiece[] = pieces.slice(2, 4);
    render(
      <MaterialDistribution
        documents={[document]}
        pieces={pieces}
        visible={visible}
        selectedId={null}
        currentDocumentId={document.documentId}
        countsReady
        onOpenDocument={vi.fn()}
        onChoose={vi.fn()}
      />,
    );
    const index = screen.getByRole("navigation", { name: "材料分布索引" });
    expect(
      [...index.querySelectorAll("[data-piece-id]")].map((node) =>
        node.getAttribute("data-piece-id"),
      ),
    ).toEqual(visible.map((piece) => piece.id));
    fireEvent.click(
      within(index).getByRole("button", {
        name: "展开密集原篇的待分类片段（2）",
      }),
    );
    expect(
      within(screen.getByRole("dialog")).getAllByRole("button", {
        name: /^定位：/,
      }),
    ).toHaveLength(2);
  });
});
