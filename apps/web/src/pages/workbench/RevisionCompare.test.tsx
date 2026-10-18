import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RevisionCompare } from "./RevisionCompare";
import { compareText, textDiff } from "./revisionDiff";
describe("precise revision diff and author adoption", () => {
  it.each([
    ["甲😀乙。保留这句话。尾声", "甲😀丙。保留这句话。新尾声"],
    ["", "新稿😀"],
    ["原稿", ""],
    ["保持不变", "保持不变"],
    ["abcabc", "abcbac"],
  ])("reconstructs both exact scalar documents: %s", (before, after) => {
    const parts = textDiff(before, after);
    expect(
      parts
        .filter((part) => part.type !== "insert")
        .map((part) => part.text)
        .join(""),
    ).toBe(before);
    expect(
      parts
        .filter((part) => part.type !== "delete")
        .map((part) => part.text)
        .join(""),
    ).toBe(after);
    for (const part of parts)
      expect(part.text).not.toMatch(
        /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u,
      );
  });
  it("keeps shared words outside ins/del and accepts the editable right hand paragraph", () => {
    const adopt = vi.fn();
    render(
      <RevisionCompare
        pairs={[
          {
            id: "p",
            before: "孩子没有开始写作业。先坐下。",
            after: "孩子还没开始写作业。先坐下。",
            label: "第 1 段",
          },
        ]}
        disabled={false}
        stale={false}
        onAdopt={adopt}
      />,
    );
    fireEvent.change(screen.getByLabelText("编辑改稿 第 1 段"), {
      target: { value: "孩子开始写作业。先坐下。" },
    });
    fireEvent.click(screen.getByRole("button", { name: "完成编辑，查看差异" }));
    expect(
      screen.getByLabelText("原稿差异视图").querySelector("del"),
    ).toHaveTextContent("没有");
    expect(screen.getByLabelText("改稿差异视图")).toHaveTextContent(
      "孩子开始写作业。先坐下。",
    );
    fireEvent.click(screen.getByRole("button", { name: "采用这一段" }));
    expect(adopt).toHaveBeenCalledWith([0], ["孩子开始写作业。先坐下。"]);
  });
  it("blocks stale suggestions and composition from overwriting a newer draft", () => {
    const adopt = vi.fn();
    const view = render(
      <RevisionCompare
        pairs={[{ id: "p", before: "原文", after: "建议", label: "第 1 段" }]}
        disabled={false}
        stale={false}
        onAdopt={adopt}
      />,
    );
    const editor = screen.getByLabelText("编辑改稿 第 1 段");
    fireEvent.compositionStart(editor);
    expect(screen.getByRole("button", { name: "采用整篇改稿" })).toBeDisabled();
    fireEvent.compositionEnd(editor);
    view.rerender(
      <RevisionCompare
        pairs={[{ id: "p", before: "原文", after: "建议", label: "第 1 段" }]}
        disabled={false}
        stale
        onAdopt={adopt}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "采用整篇改稿" }));
    expect(adopt).not.toHaveBeenCalled();
    expect(screen.getByText(/对照已过期/)).toBeInTheDocument();
  });
});

describe("bounded revision comparison", () => {
  it("bounds a large rewrite with shared characters without dropping either string", () => {
    const before = "甲乙".repeat(50000);
    const after = "丙甲".repeat(50000);
    const started = performance.now();
    const result = compareText(before, after);
    expect(result.coarse).toBe(true);
    expect(result.parts.filter(p => p.type !== "insert").map(p => p.text).join("")).toBe(before);
    expect(result.parts.filter(p => p.type !== "delete").map(p => p.text).join("")).toBe(after);
    expect(performance.now() - started).toBeLessThan(1000);
  });
  it("labels coarse markup while allowing the exact edited text to be adopted", () => {
    const before = "甲乙".repeat(800), after = "丙甲".repeat(800), adopt = vi.fn();
    render(<RevisionCompare pairs={[{ id: "long", before, after, label: "第 1 段" }]}
      disabled={false} stale={false} onAdopt={adopt} />);
    expect(screen.getByRole("status")).toHaveTextContent("按整段标记");
    expect(screen.getByLabelText("编辑改稿 第 1 段")).toHaveValue(after);
    fireEvent.click(screen.getByRole("button", { name: "采用整篇改稿" }));
    expect(adopt).toHaveBeenCalledWith(null, [after]);
  });
});
