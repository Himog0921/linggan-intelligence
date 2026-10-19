import { useState } from "react";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MaterialBrowser } from "./MaterialBrowser";
import * as api from "./workbenchApi";
import {
  newDocument,
  selectMaterial,
  type MaterialCollection,
  type MaterialDocument,
  type SourceItem,
  type WorkDocument,
} from "./workbenchModel";
import {
  FRAGMENT_TYPES,
  materialPieces,
  readingLeaves,
} from "./materialReading";
vi.mock("./workbenchApi");
const id = "00000000-0000-4000-8000-000000000001";
const item: SourceItem = {
  fragmentId: `${id}.body`,
  sourceKind: "note",
  sourceRef: id,
  workRef: id,
  field: "body",
  sourceVersion: "version1",
  title: "合成原笔记",
  text: "第一段😀\n\n第二段，跨句。保留空白。\n第三段🙂\n第四段",
  bodyState: "KNOWN",
  authorLabel: "来源作者",
  qualification: "readable",
  location: null,
};
const material: MaterialDocument = {
  documentId: `note:${id}`,
  sourceKind: "note",
  sourceRef: id,
  workRef: id,
  sourceVersion: "document1",
  title: item.title,
  authorLabel: item.authorLabel,
  qualification: "readable",
  fragments: [
    { ...item, fragmentId: `${id}.title`, field: "title", text: "合成原笔记" },
    item,
  ],
  context: { parentComment: { state: "known", body: "合成回复的父评论" } },
  matchReasons: ["本篇意图中的两个词共同命中"],
};
const collection = (documents = [material]): MaterialCollection => ({
  contract: "content-workbench-materials.v1",
  asOf: "now",
  state: "ready",
  query: "作业启动",
  terms: ["作业", "启动"],
  methodVersion: "bounded.v1",
  gaps: [],
  documents,
  nextCursor: null,
  searchScope: "collected_only",
});
function Harness({ prepared = true }: { prepared?: boolean }) {
  const [document, setDocument] = useState<WorkDocument>(() => ({
    ...newDocument("作业启动"),
    materials: prepared ? [selectMaterial(material, "prepared")] : [],
    materialPreparation: prepared
      ? {
          state: "ready" as const,
          query: "作业启动",
          terms: ["作业", "启动"],
          methodVersion: "bounded.v1",
          gaps: [],
        }
      : null,
  }));
  return (
    <MaterialBrowser
      domain={id}
      document={document}
      onChange={setDocument}
      onCollect={vi.fn()}
    />
  );
}
function show(prepared = true) {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <Harness prepared={prepared} />
    </QueryClientProvider>,
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.resolveMaterials).mockReset().mockResolvedValue(collection());
  vi.mocked(api.prepareMaterials).mockReset().mockResolvedValue(collection());
  vi.mocked(api.searchMaterials).mockReset().mockResolvedValue(
    collection([{ ...material, documentId: `note:other`, sourceRef: "other" }]),
  );
  vi.mocked(api.errorText).mockReturnValue("来源已受限");
});
describe("V3 full documents and exact fragment linking", () => {
  it("keeps a long Unicode sentence intact instead of cutting a voice at scalar 88", () => {
    const sentence = "孩子😀不愿开始写作业的时候先看见他的困难".repeat(8) + "。";
    const doc = {...material, fragments:[{...item, sourceKind:'comment' as const, text:sentence + "下一句。"}]};
    const pieces = materialPieces(doc);
    expect(pieces.map(piece => piece.text)).toEqual([sentence, "下一句。"]);
    expect(pieces[0]?.end).toBe(Array.from(sentence).length);
    expect(pieces[1]?.start).toBe(pieces[0]?.end);
  });

  it("flows title and body together and keeps all Unicode scalar ranges exact", () => {
    const doc = {
      ...material,
      fragments: [
        { ...item, field: "title" as const, text: "标题" },
        { ...item, text: "甲😀乙。\n".repeat(250) },
      ],
    };
    const pieces = materialPieces(doc),
      leaves = readingLeaves(doc);
    expect(leaves.length).toBeGreaterThan(2);
    expect(leaves[0]?.some((piece) => piece.source.field === "body")).toBe(
      true,
    );
    expect(
      pieces
        .filter((piece) => piece.source.field === "body")
        .map((piece) => piece.text)
        .join(""),
    ).toBe(doc.fragments[1]?.text);
    for (const piece of pieces)
      expect(
        Array.from(piece.source.text).slice(piece.start, piece.end).join(""),
      ).toBe(piece.text);
    expect(pieces.every((piece) => /[。！？\n][。！？”’」』）)\]】》〉〕｝}"\s]*$/u.test(piece.text) || piece.end === Array.from(piece.source.text).length)).toBe(true);
  });
  it.each([
    ['他说：“先试两分钟。”然后开始。', ['他说：“先试两分钟。”', '然后开始。']],
    ['（对孩子来说！）还有下一步。', ['（对孩子来说！）', '还有下一步。']],
    ['真的吗？！太好了！！！', ['真的吗？！', '太好了！！！']],
    ['甲😀乙。\n下一句。', ['甲😀乙。\n', '下一句。']],
    ['他说"好。""不行。"', ['他说"好。"', '"不行。"']],
    ["他说'好。''不行。'", ["他说'好。'", "'不行。'"]],
    ['他很开心。"我们走吧。"他说。', ['他很开心。', '"我们走吧。"', '他说。']],
    ['他说"好。很好。"然后出门。', ['他说"好。', '很好。"', '然后出门。']],
    ["他说'好。很好。'然后出门。", ["他说'好。", "很好。'", '然后出门。']],
  ])('keeps closing marks with the sentence and exact scalar ranges: %s', (text, expected) => {
    const doc = {...material, fragments: [{...item, text}]};
    const pieces = materialPieces(doc);
    expect(pieces.map(piece => piece.text)).toEqual(expected);
    expect(pieces.map(piece => piece.text).join('')).toBe(text);
    for (const piece of pieces) expect(Array.from(text).slice(piece.start, piece.end).join('')).toBe(piece.text);
  });
  it("shares fragment identity across paper, book highlight and the compact matrix", async () => {
    show();
    const wall = screen.getByRole("region", { name: "分组摘录纸条" });
    const paper = await within(wall).findByRole("button", { name: "第一段😀" });
    fireEvent.click(paper);
    const reading = screen.getByRole("region", { name: "原文阅读" });
    const leaf = reading.querySelector(
      `[data-fragment-id="${paper.getAttribute("data-fragment-id")}"]`,
    );
    expect(leaf?.querySelector("mark")).toHaveTextContent("第一段😀");
    expect(
      screen
        .getByRole("navigation", { name: "材料分布索引" })
        .querySelector(
          `[data-piece-id="${paper.getAttribute("data-fragment-id")}"]`,
        ),
    ).toHaveAttribute("data-selected", "true");
    fireEvent.click(screen.getByRole("button", { name: "边界0" }));
    expect(
      within(wall).queryByRole("button", { name: "第一段😀" }),
    ).not.toBeInTheDocument();
  });
  it("previews a search result before adding it to this work and keeps collecting separate", async () => {
    show();
    await within(
      screen.getByRole("region", { name: "分组摘录纸条" }),
    ).findByRole("button", { name: "第一段😀" });
    fireEvent.click(screen.getByRole("button", { name: "搜寻更多" }));
    fireEvent.change(screen.getByLabelText("检索已取得材料"), {
      target: { value: "其他材料" },
    });
    fireEvent.click(screen.getByRole("button", { name: "检索" }));
    const add = await screen.findByRole("button", { name: "加入本篇材料" });
    await waitFor(() => expect(add).toBeEnabled());
    expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
    expect(screen.getByRole("region", { name: "原文阅读" })).toHaveTextContent(
      "合成原笔记",
    );
    fireEvent.click(add);
    expect(
      await screen.findByRole("button", { name: "已加入本篇" }),
    ).toBeDisabled();
  });
  it("registers a prepared collection without requiring any canvas excerpts", async () => {
    show(false);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "本篇材料 1" }),
      ).toBeInTheDocument(),
    );
    expect(api.prepareMaterials).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
  });
  it("shows a failed preparation and offers explicit retry without making a false empty result", async () => {
    vi.mocked(api.prepareMaterials).mockRejectedValue(new Error("failed"));
    show(false);
    expect(await screen.findByRole("alert")).toHaveTextContent("来源已受限");
    expect(
      screen.getByRole("button", { name: "重试读取" }),
    ).toBeInTheDocument();
  });
  it("does not render restricted material or enable collecting it", async () => {
    vi.mocked(api.resolveMaterials).mockResolvedValue(
      collection([
        {
          ...material,
          qualification: "restricted",
          fragments: [],
          context: null,
        },
      ]),
    );
    show();
    expect(await screen.findByText("来源已受限")).toBeInTheDocument();
    expect(screen.queryByText("第一段😀")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
  });
});

it("keeps Chinese, emoji and newline offsets exact when selecting across two paper sentences", async () => {
  vi.mocked(api.anchor).mockImplementation(async (request) => ({
    citation: {
      ...request,
      sourceKind: item.sourceKind,
      field: item.field,
      textHash: "hash",
      location: null,
    },
    selectedText: Array.from(item.text)
      .slice(request.start, request.end)
      .join(""),
    textHash: "hash",
  }));
  show();
  const wall = screen.getByRole("region", { name: "分组摘录纸条" });
  fireEvent.click(
    await within(wall).findByRole("button", { name: "第一段😀" }),
  );
  const book = screen.getByRole("region", { name: "原文阅读" });
  const passages = [...book.querySelectorAll('[aria-label="可选择原文"]')];
  const first = passages.find((element) =>
    element.textContent?.includes("第一段😀"),
  )!;
  const second = passages.find(
    (element) => element.textContent === "第二段，跨句。",
  )!;
  const firstNode = document
    .createTreeWalker(first, NodeFilter.SHOW_TEXT)
    .nextNode()!;
  const secondNode = document
    .createTreeWalker(second, NodeFilter.SHOW_TEXT)
    .nextNode()!;
  const range = document.createRange();
  range.setStart(firstNode, 1);
  range.setEnd(secondNode, 3);
  const selection = window.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
  fireEvent.mouseUp(first.closest("article")!.parentElement!);
  fireEvent.click(screen.getByRole("button", { name: "摘入画布" }));
  await waitFor(() => expect(api.anchor).toHaveBeenCalledTimes(1));
  const request = vi.mocked(api.anchor).mock.calls[0]![0];
  expect(request.start).toBe(1);
  expect(request.end).toBe(Array.from("第一段😀\n\n").length + 3);
  expect(Array.from(item.text).slice(request.start, request.end).join("")).toBe(
    "一段😀\n\n第二段",
  );
  selection.removeAllRanges();
});

it("reconciles a changed document only after the author explicitly accepts its current source version", async () => {
  vi.mocked(api.resolveMaterials).mockImplementation(async (request) =>
    collection([
      {
        ...material,
        sourceVersion: "document2",
        qualification:
          request.selections[0]?.sourceVersion === "document2"
            ? "readable"
            : "changed",
      },
    ]),
  );
  show();
  const confirm = await screen.findByRole("button", {
    name: "已核对，采用当前来源版本",
    hidden: true,
  });
  expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
  fireEvent.click(confirm);
  await waitFor(() =>
    expect(
      vi.mocked(api.resolveMaterials).mock.calls.at(-1)?.[0].selections[0]
        ?.sourceVersion,
    ).toBe("document2"),
  );
  expect(await screen.findByText("已取得原文")).toBeInTheDocument();
});

it("does not apply a prepared collection after the topic intent changed or the reading surface closed", async () => {
  let finish: (result: MaterialCollection) => void = () => undefined;
  vi.mocked(api.prepareMaterials)
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValueOnce(collection([]));
  const onChange = vi.fn();
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const original = newDocument("旧意图");
  const view = render(
    <QueryClientProvider client={client}>
      <MaterialBrowser
        domain={id}
        document={original}
        onChange={onChange}
        onCollect={vi.fn()}
      />
    </QueryClientProvider>,
  );
  await waitFor(() => expect(api.prepareMaterials).toHaveBeenCalledTimes(1));
  view.rerender(
    <QueryClientProvider client={client}>
      <MaterialBrowser
        domain={id}
        active={false}
        document={{ ...original, title: "新意图" }}
        onChange={onChange}
        onCollect={vi.fn()}
      />
    </QueryClientProvider>,
  );
  finish(collection());
  await new Promise((resolve) => setTimeout(resolve, 30));
  expect(onChange).not.toHaveBeenCalled();
});

it("does not project historical citation offsets onto a newer source version", async () => {
  vi.mocked(api.materialFromCitation).mockResolvedValue(material);
  const document = {
    ...newDocument("作业启动"),
    materials: [selectMaterial(material, "prepared")],
    materialPreparation: {
      state: "ready" as const,
      query: "作业启动",
      terms: ["作业"],
      methodVersion: "bounded.v1",
      gaps: [],
    },
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <MaterialBrowser
        domain={id}
        document={document}
        onChange={vi.fn()}
        onCollect={vi.fn()}
        focusCitation={{
          sourceKind: "note",
          sourceRef: id,
          workRef: id,
          field: "body",
          sourceVersion: "older",
          start: 0,
          end: 3,
          textHash: "old",
          location: null,
        }}
      />
    </QueryClientProvider>,
  );
  expect(await screen.findByText(/旧摘录的来源版本已变化/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
  expect(
    screen
      .getByRole("navigation", { name: "材料分布索引" })
      .querySelector('[aria-current="location"]'),
  ).toBeNull();
});

it.each([
  ["昨天晚上，每次坐在桌前。", "场景"],
  ["建议先把第一步拆成一个步骤。", "做法"],
  ["但是这句话不代表已经有边界标注。", "边界"],
  ["我认为关键在于这个观点。", "观点"],
  ["今天可以试着这样做，然后再看。", "做法"],
])("keeps wording-based reading hints explicitly unverified: %s", (text, type) => {
  const sources: SourceItem[] = [
    { ...item, sourceKind: "note", text },
    { ...item, sourceKind: "ocr", field: "ocr", text },
    { ...item, sourceKind: "ops_publication", text },
  ];
  for (const source of sources) {
    const pieces = materialPieces({ ...material, sourceKind: source.sourceKind === "ocr" ? "note" : source.sourceKind, fragments: [source] });
    expect(pieces.length).toBeGreaterThan(0);
    expect(pieces.every((piece) => piece.type === type && piece.typeBasis === "unverified_keyword")).toBe(true);
    expect(pieces.map((piece) => piece.text).join("")).toBe(text);
  }
});

it("uses factual source-kind basis for comment voices", () => {
  const pieces = materialPieces({ ...material, sourceKind: "comment", fragments: [{ ...item, sourceKind: "comment", field: "comment_body", text: "昨天我建议先做一步，但是还没开始。" }] });
  expect(pieces.length).toBeGreaterThan(0);
  expect(pieces.every((piece) => piece.type === "原声" && piece.typeBasis === "source_kind")).toBe(true);
});

it("qualifies both type filters and the distribution as local reading hints rather than research annotations", async () => {
  show();
  await within(screen.getByRole("region", { name: "分组摘录纸条" })).findByRole("button", { name: "第一段😀" });
  expect(screen.getByText("本机词语提示 · 未核对")).toBeInTheDocument();
  const filters = screen.getByRole("group", { name: "片段类型" });
  const matrix = screen.getByRole("navigation", { name: "材料分布索引" });
  expect(filters).toHaveAccessibleDescription(/本机词语匹配.*不是研究标注/);
  expect(matrix).toHaveAccessibleDescription(/本机词语匹配.*不是研究标注/);
  expect(within(matrix).getByText("词语提示分布（未核对）")).toBeInTheDocument();
});

it("keeps unclassified titles and factual sentences separate while conserving every fragment across matrix types", () => {
  const document = {
    ...material,
    fragments: [
      { ...item, field: "title" as const, text: "今天先把标题拿来" },
      {
        ...item,
        text: "记录一件事情。\n第一步先把本子打开。\n我认为关键在于开始。",
      },
    ],
  };
  const pieces = materialPieces(document);
  expect(pieces.find((piece) => piece.source.field === "title")?.type).toBe(
    "待分类",
  );
  expect(pieces.find((piece) => piece.text.includes("记录"))?.type).toBe(
    "待分类",
  );
  const counts = FRAGMENT_TYPES.map(
    (type) => pieces.filter((piece) => piece.type === type).length,
  );
  expect(counts.reduce((sum, count) => sum + count, 0)).toBe(pieces.length);
  expect(pieces.every((piece) => FRAGMENT_TYPES.includes(piece.type))).toBe(
    true,
  );
});

it("groups unknown authors by their original document titles rather than merging all anonymous notes", async () => {
  const first = {
    ...material,
    authorLabel: "作者未知·原篇标题分组",
    fragments: material.fragments.map((fragment) => ({
      ...fragment,
      authorLabel: "作者未知·原篇标题分组",
    })),
  };
  const second = {
    ...first,
    documentId: "note:second",
    sourceRef: "second",
    title: "另一篇原笔记",
    fragments: first.fragments.map((fragment) => ({
      ...fragment,
      fragmentId: fragment.fragmentId + "-second",
      title: "另一篇原笔记",
      sourceRef: "second",
    })),
  };
  vi.mocked(api.resolveMaterials).mockResolvedValue(
    collection([first, second]),
  );
  show();
  const wall = screen.getByRole("region", { name: "分组摘录纸条" });
  expect(
    await within(wall).findByRole("heading", { level: 3, name: /合成原笔记/ }),
  ).toBeInTheDocument();
  expect(
    within(wall).getByRole("heading", { level: 3, name: /另一篇原笔记/ }),
  ).toBeInTheDocument();
  expect(within(wall).getAllByRole("heading", { level: 3 })).toHaveLength(2);
});

it.each([
  ["below", 220, 200],
  ["above", -20, 20],
  ["visible", 20, null],
] as const)(
  "scrolls only the paper surface to the nearest visible position when the matrix selects a %s paper",
  async (_case, top, expectedTop) => {
    const voiceSource = {
      ...item,
      fragmentId: "voice.body",
      sourceKind: "comment" as const,
      sourceRef: "voice",
      field: "comment_body" as const,
      text: "匿名原声落在后面。",
      authorLabel: "匿名原声",
    };
    const voice = {
      ...material,
      documentId: "comment:voice",
      sourceKind: "comment" as const,
      sourceRef: "voice",
      title: "父笔记讨论",
      authorLabel: "匿名原声",
      fragments: [voiceSource],
    };
    vi.mocked(api.resolveMaterials).mockResolvedValue(
      collection([material, voice]),
    );
    show();
    const wall = screen.getByRole("region", { name: "分组摘录纸条" });
    const paper = await within(wall).findByRole("button", {
      name: "匿名原声落在后面。",
    });
    const host = wall.querySelector<HTMLElement>(
      "[data-material-paper-scroll]",
    )!;
    Object.defineProperties(host, {
      clientHeight: { configurable: true, value: 100 },
      clientWidth: { configurable: true, value: 240 },
      scrollTop: { configurable: true, writable: true, value: 40 },
      scrollLeft: { configurable: true, writable: true, value: 0 },
    });
    vi.spyOn(host, "getBoundingClientRect").mockReturnValue(
      new DOMRect(0, 0, 240, 100),
    );
    vi.spyOn(paper, "getBoundingClientRect").mockReturnValue(
      new DOMRect(0, top, 220, 40),
    );
    const localScroll = vi.fn();
    Object.defineProperty(host, "scrollTo", {
      configurable: true,
      value: localScroll,
    });
    const pageScroll = vi
      .spyOn(window, "scrollTo")
      .mockImplementation(() => undefined);
    const matrix = screen.getByRole("navigation", { name: "材料分布索引" });
    const locator = within(matrix).getByRole("button", {
      name: "定位：匿名原声落在后面。",
    });
    locator.focus();
    fireEvent.click(locator);
    if (expectedTop === null) expect(localScroll).not.toHaveBeenCalled();
    else
      await waitFor(() =>
        expect(localScroll).toHaveBeenCalledWith({
          top: expectedTop,
          left: 0,
          behavior: "instant",
        }),
      );
    expect(pageScroll).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(locator);
    expect(paper).toHaveAttribute("aria-pressed", "true");
    expect(
      within(wall)
        .getAllByRole("button")
        .some((button) => button.textContent?.includes("第一段😀")),
    ).toBe(true);
    expect(screen.getByRole("button", { name: "全部材料2" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    pageScroll.mockRestore();
  },
);

it("consumes a backlink positioning intent when the author flips a long document", async () => {
  const long = {
    ...material,
    fragments: [
      {
        ...item,
        text: Array.from(
          { length: 180 },
          (_, index) => `第${index + 1}段长文保留准确原文。`,
        ).join("\n"),
      },
    ],
  };
  vi.mocked(api.resolveMaterials).mockResolvedValue(
    collection([
      {
        ...material,
        documentId: "note:intro",
        sourceRef: "intro",
        title: "前一篇短文",
        fragments: [
          {
            ...item,
            fragmentId: "intro.body",
            sourceRef: "intro",
            text: "前一篇短文。",
          },
        ],
      },
      long,
    ]),
  );
  vi.mocked(api.materialFromCitation).mockResolvedValue(long);
  const document = {
    ...newDocument("选题"),
    materials: [selectMaterial(long, "prepared")],
    materialPreparation: {
      state: "ready" as const,
      query: "选题",
      terms: ["选题"],
      methodVersion: "bounded.v1",
      gaps: [],
    },
  };
  const citation = {
    sourceKind: "note" as const,
    sourceRef: id,
    workRef: id,
    field: "body" as const,
    sourceVersion: "version1",
    start: 0,
    end: 4,
    textHash: "hash",
    location: null,
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <MaterialBrowser
        domain={id}
        document={document}
        onChange={vi.fn()}
        onCollect={vi.fn()}
        focusCitation={citation}
      />
    </QueryClientProvider>,
  );
  await screen.findByText(/^1–2 \/ \d+ 页$/);
  fireEvent.click(screen.getByRole("button", { name: "下一展开页 →" }));
  expect(await screen.findByText(/^3–4 \/ \d+ 页$/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "下一展开页 →" }));
  expect(screen.getByText(/^5–6 \/ \d+ 页$/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "← 上一展开页" }));
  expect(screen.getByText(/^3–4 \/ \d+ 页$/)).toBeInTheDocument();
});

it("lets matrix document navigation release the backlink and a new citation position the reader again", async () => {
  const long = {
    ...material,
    fragments: [
      {
        ...item,
        text: Array.from(
          { length: 90 },
          (_, index) => `第${index + 1}段原篇长文。`,
        ).join("\n"),
      },
    ],
  };
  const other = {
    ...material,
    documentId: "note:other",
    sourceRef: "other",
    title: "另一篇完整文档",
    fragments: [
      {
        ...item,
        fragmentId: "other.body",
        sourceRef: "other",
        title: "另一篇完整文档",
        text: "另一篇的准确正文。",
      },
    ],
  };
  vi.mocked(api.resolveMaterials).mockResolvedValue(collection([long, other]));
  vi.mocked(api.materialFromCitation).mockResolvedValue(long);
  const document = {
    ...newDocument("选题"),
    materials: [
      selectMaterial(long, "prepared"),
      selectMaterial(other, "prepared"),
    ],
    materialPreparation: {
      state: "ready" as const,
      query: "选题",
      terms: ["选题"],
      methodVersion: "bounded.v1",
      gaps: [],
    },
  };
  const citation = {
    sourceKind: "note" as const,
    sourceRef: id,
    workRef: id,
    field: "body" as const,
    sourceVersion: "version1",
    start: 0,
    end: 4,
    textHash: "hash",
    location: null,
  };
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const content = (focus: typeof citation) => (
    <QueryClientProvider client={client}>
      <MaterialBrowser
        domain={id}
        document={document}
        onChange={vi.fn()}
        onCollect={vi.fn()}
        focusCitation={focus}
      />
    </QueryClientProvider>
  );
  const view = render(content(citation));
  await screen.findByText(/^1–2 \/ \d+ 页$/);
  const index = screen.getByRole("navigation", { name: "材料分布索引" });
  fireEvent.click(within(index).getByRole("button", { name: "02" }));
  expect(screen.getByRole("region", { name: "原文阅读" })).toHaveTextContent(
    "另一篇的准确正文。",
  );
  expect(
    screen.getByRole("region", { name: "原文阅读" }),
  ).not.toHaveTextContent("第1段原篇长文。");
  const later = materialPieces(long)[30]!;
  view.rerender(
    content({
      ...citation,
      start: later.start,
      end: later.end,
      textHash: "later",
    }),
  );
  await waitFor(() =>
    expect(
      screen
        .getByRole("region", { name: "原文阅读" })
        .querySelector("mark:not([class])"),
    ).toHaveTextContent(later.text.trim()),
  );
  expect(within(index).getByRole("button", { name: "01" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(
    screen.getByRole("region", { name: "原文阅读" }),
  ).not.toHaveTextContent("另一篇的准确正文。");
});

it("clears selection on both search page directions and cannot collect an unadded source using another page qualification", async () => {
  const unadded = {
    ...material,
    documentId: "note:unadded",
    sourceRef: "unadded",
    title: "未加入的A",
    fragments: [
      {
        ...item,
        fragmentId: "unadded.body",
        sourceRef: "unadded",
        title: "未加入的A",
        text: "未加入的A原文。",
      },
    ],
  };
  const added = {
    ...material,
    title: "已加入的B",
    fragments: [{ ...item, title: "已加入的B", text: "已加入的B原文。" }],
  };
  vi.mocked(api.resolveMaterials).mockResolvedValue(collection([added]));
  vi.mocked(api.searchMaterials).mockImplementation(async (request) => ({
    ...collection(request.cursor === 24 ? [added] : [unadded]),
    nextCursor: request.cursor === 24 ? null : 24,
  }));
  show();
  await within(screen.getByRole("region", { name: "分组摘录纸条" })).findByRole(
    "button",
    { name: "已加入的B原文。" },
  );
  fireEvent.click(screen.getByRole("button", { name: "搜寻更多" }));
  fireEvent.change(screen.getByLabelText("检索已取得材料"), {
    target: { value: "原文" },
  });
  fireEvent.click(screen.getByRole("button", { name: "检索" }));
  const wall = screen.getByRole("region", { name: "分组摘录纸条" });
  fireEvent.click(
    await within(wall).findByRole("button", { name: "未加入的A原文。" }),
  );
  expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "更多来源" }));
  const paperB = await within(wall).findByRole("button", {
    name: "已加入的B原文。",
  });
  expect(screen.getByRole("region", { name: "原文阅读" })).toHaveTextContent(
    "已加入的B原文。",
  );
  expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
  fireEvent.click(paperB);
  expect(screen.getByRole("button", { name: "摘入画布" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "上一页来源" }));
  await within(wall).findByRole("button", { name: "未加入的A原文。" });
  expect(screen.getByRole("button", { name: "摘入画布" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "摘入画布" }));
  expect(api.anchor).not.toHaveBeenCalled();
});

it('numbers voices independently of note groups', async () => {
  const notes = ['one','two'].map((key,index) => ({...material, documentId:key, title:`笔记${index+1}`, authorLabel:`作者${index+1}`, fragments:[{...item,fragmentId:key,authorLabel:`作者${index+1}`,text:`笔记正文${index+1}。`}]}));
  const voices = ['voice1','voice2'].map((key,index) => ({...material, documentId:key, sourceKind:'comment' as const, title:`评论所在笔记${index+1}`, fragments:[{...item,fragmentId:key,sourceKind:'comment' as const,text:`原声内容${index+1}。`}]}));
  vi.mocked(api.resolveMaterials).mockResolvedValue(collection([...notes,...voices]));show();
  const wall=screen.getByRole('region',{name:'分组摘录纸条'});
  expect(await within(wall).findByRole('button',{name:/评论所在笔记1/})).toHaveTextContent(/原声\s*1/);
  expect(within(wall).getByRole('button',{name:/评论所在笔记2/})).toHaveTextContent(/原声\s*2/);
  expect(within(wall).queryByText(/原声 [34]/)).not.toBeInTheDocument();
});

it('prepares and registers materials again after the same browser restores an unprepared historical document', async () => {
  function RestoredDocument() {
    const [document, setDocument] = useState(() => newDocument('作业启动'));
    return <>
      <button onClick={() => setDocument(newDocument('作业启动'))}>恢复早期稿</button>
      <output aria-label="材料登记状态">{document.materialPreparation ? '已登记' : '未登记'}</output>
      <MaterialBrowser domain={id} document={document} onChange={setDocument} onCollect={vi.fn()} />
    </>;
  }
  render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><RestoredDocument /></QueryClientProvider>);
  await waitFor(() => expect(screen.getByLabelText('材料登记状态')).toHaveTextContent('已登记'));
  expect(api.prepareMaterials).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('button', {name:'恢复早期稿'}));
  await waitFor(() => expect(api.prepareMaterials).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.getByLabelText('材料登记状态')).toHaveTextContent('已登记'));
  await waitFor(() => expect(api.resolveMaterials).toHaveBeenCalledTimes(2));
  await screen.findByRole('button', {name:'本篇材料 1'});
  fireEvent.click(await within(screen.getByRole('region', {name:'分组摘录纸条'})).findByRole('button', {name:'第一段😀'}));
  await waitFor(() => expect(screen.getByRole('button', {name:'摘入画布'})).toBeEnabled());
});
