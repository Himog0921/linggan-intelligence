import { parseDossier } from '../corpus/evidence/dossierDecode';
import { DossierBlocks } from '../corpus/evidence/DossierBlocks';
import { mediaPath } from '../corpus/evidence/evidencePaths';
import { MaterialDistribution } from "./MaterialDistribution";
import { ActionButton } from "../../shared/ActionButton";
import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { isRecord } from "../../shared/api";
import * as api from "./workbenchApi";
import {
  GROUPS,
  SOURCE_LABELS,
  addMaterials,
  selectMaterial,
  type Citation,
  type CanvasGroup,
  type ValidatedAnchor,
  type WorkDocument,
} from "./workbenchModel";
import {
  FRAGMENT_TYPES,
  READING_HINT_NOTICE,
  materialPieces,
  readingLeaves,
  type ReadingPiece,
} from "./materialReading";
import styles from "./WorkbenchPage.module.css";
interface Props {
  contextKey?: string;
  active?: boolean;
  domain: string;
  document: WorkDocument;
  onChange: (document: WorkDocument) => void;
  onCollect: (anchor: ValidatedAnchor, group: CanvasGroup) => void;
  focusCitation?: Citation | null;
  onCanvas?: () => void;
}
interface Selection {
  piece: ReadingPiece;
  start: number;
  end: number;
  text: string;
}
function SourceContextView({ value }: { value: unknown }) {
  if (!isRecord(value)) return null;
  const parent = isRecord(value.parentWork) ? value.parentWork : null;
  const comment = isRecord(value.parentComment) ? value.parentComment : null;
  let dossier: ReturnType<typeof parseDossier> | null = null;
  try { if (parent?.dossier !== undefined) dossier = parseDossier(parent.dossier); } catch { /* An invalid source is visible as unavailable, never replaced with a legacy body. */ }
  return <div className={styles.context}>
    {comment && comment.state !== 'not_applicable' ? <section><h3>回复语境</h3>{comment.state === 'known' && typeof comment.body === 'string' ? <blockquote>{comment.body}</blockquote> : <p>{comment.state === 'restricted' ? '父评论已受限' : '父评论正文尚未确认'}</p>}</section> : null}
    {parent ? <section><h3>所在笔记</h3>{dossier ? <>{dossier.note.platformTitle ? <p className={styles.evidence}>{dossier.note.platformTitle}</p> : null}{dossier.materials.map(unit => <DossierBlocks key={unit.ref} blocks={unit.blocks} issues={unit.issues} />)}</> : <p>{parent.state === 'restricted' ? '所在笔记已受限' : '所在笔记暂不可读'}</p>}</section> : null}
  </div>;
}
function OcrEvidence({ piece, context }: { piece: ReadingPiece; context: unknown; start: number; end: number }) {
  const location = piece.source.location;
  if (!location) return null;
  let dossier: ReturnType<typeof parseDossier> | null = null;
  try { if (isRecord(context)) dossier = parseDossier(context.dossier); } catch { /* The reading is unavailable; do not locate against a different revision. */ }
  const material = dossier?.materials.find(unit => unit.ref === location.unitRef && unit.currentRevisionRef === location.readingRef && unit.blocks.some(block => block.ref === location.blockRef));
  const url = mediaPath(material?.originalUrl);
  return <section><h3>原文区块位置</h3>{url ? <div className={styles.ocrImage}><img src={url} alt="文字来源原图" /><span style={{ left: `${location.bbox[0] * 100}%`, top: `${location.bbox[1] * 100}%`, width: `${location.bbox[2] * 100}%`, height: `${location.bbox[3] * 100}%` }} /></div> : <p>该版本的原图当前不可读；保留原区块引用。</p>}</section>;
}
function BookPassage({
  piece,
  selected,
  matching,
  onSelect,
}: {
  piece: ReadingPiece;
  selected: Selection | null;
  matching: boolean;
  onSelect: (selection: Selection) => void;
}) {
  const ref = useRef<HTMLParagraphElement>(null);
  function capture() {
    const root = ref.current,
      selection = window.getSelection();
    if (!root || !selection || selection.isCollapsed || !selection.rangeCount)
      return;
    const range = selection.getRangeAt(0);
    if (
      !root.contains(range.startContainer) ||
      !root.contains(range.endContainer)
    )
      return;
    const before = range.cloneRange();
    before.selectNodeContents(root);
    before.setEnd(range.startContainer, range.startOffset);
    const start = piece.start + Array.from(before.toString()).length;
    onSelect({
      piece,
      start,
      end: start + Array.from(range.toString()).length,
      text: range.toString(),
    });
  }
  const local =
    selected?.piece.source.fragmentId === piece.source.fragmentId &&
    selected.end > piece.start &&
    selected.start < piece.end
      ? selected
      : null;
  const chars = Array.from(piece.text);
  return (
    <p
      ref={ref}
      tabIndex={0}
      className={`${styles.sourceText} ${piece.source.field === "title" ? styles.bookHeading : ""}`}
      data-fragment-id={piece.id}
      data-source-ref={piece.source.sourceRef}
      data-work-ref={piece.source.workRef ?? undefined}
      aria-label="可选择原文"
      onMouseUp={capture}
      onKeyUp={capture}
    >
      {local ? (
        <>
          {chars.slice(0, Math.max(0, local.start - piece.start)).join("")}
          <mark>
            {chars
              .slice(
                Math.max(0, local.start - piece.start),
                Math.min(chars.length, local.end - piece.start),
              )
              .join("")}
          </mark>
          {chars
            .slice(Math.min(chars.length, local.end - piece.start))
            .join("")}
        </>
      ) : matching ? (
        <mark className={styles.matchingPassage}>{piece.text}</mark>
      ) : (
        piece.text
      )}
    </p>
  );
}
export function MaterialBrowser({
  contextKey = "standalone",
  active = true,
  domain,
  document,
  onChange,
  onCollect,
  focusCitation,
  onCanvas,
}: Props) {
  const paperScroller = useRef<HTMLDivElement>(null);
  const paperButtons = useRef(new Map<string, HTMLButtonElement>());
  const readingHost = useRef<HTMLElement>(null);
  const [leafCapacity, setLeafCapacity] = useState(160);
  useEffect(() => {
    const host = readingHost.current;
    if (!host) return;
    function measure() {
      if (!host) return;
      const width = host.getBoundingClientRect().width;
      if (width <= 0) return;
      const leaf = host.querySelector("article");
      const prose = host.querySelector("p");
      const leafStyle = window.getComputedStyle(leaf ?? host),
        textStyle = window.getComputedStyle(prose ?? host);
      const font = parseFloat(textStyle.fontSize) || 15,
        line = parseFloat(textStyle.lineHeight) || font * 1.9,
        padding = parseFloat(leafStyle.paddingLeft) || 20;
      const columns = Math.max(5, Math.floor((width / 2 - padding * 2) / font));
      const rows = Math.max(
        2,
        Math.floor(
          (Math.max(
            80,
            leaf?.getBoundingClientRect().height ?? window.innerHeight * 0.4,
          ) -
            100) /
            line,
        ),
      );
      setLeafCapacity(columns * rows);
    }
    const frame = requestAnimationFrame(measure);
    const observer =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(measure)
        : null;
    observer?.observe(host);
    window.addEventListener("resize", measure);
    return () => {
      cancelAnimationFrame(frame);
      observer?.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, []);
  const [scope, setScope] = useState<"prepared" | "search">("prepared");
  const [kind, setKind] = useState("all"),
    [type, setType] = useState("全部"),
    [filter, setFilter] = useState("");
  const [search, setSearch] = useState(""),
    [query, setQuery] = useState(""),
    [cursor, setCursor] = useState(0);
  const [activeDocumentId, setActiveDocumentId] = useState<string | null>(null);
  const [selection, setSelection] = useState<Selection | null>(null),
    [spread, setSpread] = useState(0);
  const [citationIntent, setCitationIntent] = useState({
    citation: focusCitation,
    consumed: false,
  });
  // A new citation reference is a new positioning intent. Local navigation consumes it.
  if (citationIntent.citation !== focusCitation) {
    setCitationIntent({ citation: focusCitation, consumed: false });
    setSelection(null);
    setActiveDocumentId(null);
    setSpread(0);
  }
  const citationFocusActive = !!focusCitation && !citationIntent.consumed;
  function releaseCitationFocus() {
    if (focusCitation)
      setCitationIntent({ citation: focusCitation, consumed: true });
  }
  const [group, setGroup] = useState<CanvasGroup>("inbox"),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const [busyAction, setBusyAction] = useState<"prepare" | "collect" | null>(
    null,
  );
  const preparationIntent =
    document.materialPreparation || document.materials.length
      ? null
      : JSON.stringify([
          document.title,
          document.angle,
          document.outline,
          document.topicRef,
        ]);
  const prepared = useQuery({
    queryKey: [
      "creation-materials",
      domain,
      contextKey,
      document.materials,
      document.materialPreparation?.terms,
      preparationIntent,
    ],
    queryFn: async ({ signal }) => {
      const collection = await (document.materials.length ||
      document.materialPreparation
        ? api.resolveMaterials(
            {
              domainRef: domain,
              selections: document.materials,
              ...(document.materialPreparation
                ? { terms: document.materialPreparation.terms }
                : {}),
            },
            signal,
          )
        : api.prepareMaterials(
            {
              domainRef: domain,
              title: document.title,
              angle: document.angle,
              outline: document.outline,
              topicRef: document.topicRef,
            },
            signal,
          ));
      return { ...collection, requestedIntent: preparationIntent };
    },
    enabled: active,
    gcTime: 0,
    retry: false,
  });
  const found = useQuery({
    queryKey: [
      "creation-material-search",
      domain,
      contextKey,
      query,
      cursor,
      kind,
    ],
    queryFn: ({ signal }) =>
      api.searchMaterials(
        {
          domainRef: domain,
          query,
          cursor,
          ...(kind === "all"
            ? {}
            : { kind: kind as "note" | "comment" | "ops_publication" }),
        },
        signal,
      ),
    enabled: active && scope === "search" && !!query.trim(),
    gcTime: 0,
    retry: false,
  });
  const liveDocument = useRef(document),
    requestEpoch = useRef(0);
  useEffect(() => {
    liveDocument.current = document;
  }, [document]);
  useEffect(
    () => () => {
      requestEpoch.current++;
    },
    [domain, contextKey],
  );
  const registered = useRef(false);
  const hadPreparation = useRef(!!document.materialPreparation || document.materials.length > 0);
  useEffect(() => {
    const hasPreparation = !!document.materialPreparation || document.materials.length > 0;
    if (hadPreparation.current && !hasPreparation) registered.current = false;
    hadPreparation.current = hasPreparation;
    if (
      !active ||
      !prepared.data ||
      prepared.data.requestedIntent === null ||
      prepared.data.requestedIntent !== preparationIntent ||
      registered.current ||
      document.materialPreparation
    )
      return;
    registered.current = true;
    const { methodVersion, query, terms, state, gaps, documents } =
      prepared.data;
    onChange({
      ...document,
      materials: addMaterials(
        document.materials,
        documents.map((item) => selectMaterial(item, "prepared")),
      ),
      materialPreparation: { methodVersion, query, terms, state, gaps },
    });
  }, [active, prepared.data, preparationIntent, document, onChange]);
  const focused = useQuery({
    queryKey: ["creation-material-focus", domain, contextKey, focusCitation],
    queryFn: ({ signal }) =>
      api.materialFromCitation(domain, focusCitation!, signal),
    enabled: active && citationFocusActive,
    gcTime: 0,
    retry: false,
  });
  const collection = scope === "prepared" ? prepared : found;
  const countsReady = collection.isSuccess;
  const docs = useMemo(() => {
    const items = collection.data?.documents ?? [];
    return focused.data &&
      !items.some((item) => item.documentId === focused.data.documentId)
      ? [...items, focused.data]
      : items;
  }, [collection.data, focused.data]);
  const pieces = useMemo(() => docs.flatMap(materialPieces), [docs]);
  const visible = pieces.filter(
    (piece) =>
      (kind === "all" ||
        piece.source.sourceKind === kind ||
        (kind === "note" && ["ocr", "transcript", "frame_text"].includes(piece.source.sourceKind))) &&
      (type === "全部" || piece.type === type) &&
      `${piece.text} ${piece.source.authorLabel}`.includes(filter),
  );
  const focusedPiece =
    citationFocusActive && focusCitation
      ? pieces.find(
          (piece) =>
            piece.source.sourceRef === focusCitation.sourceRef &&
            piece.source.field === focusCitation.field &&
            piece.source.blockRef === focusCitation.blockRef &&
            piece.source.asOf === focusCitation.asOf &&
            piece.source.sourceVersion === focusCitation.sourceVersion &&
            piece.start <= focusCitation.start &&
            piece.end > focusCitation.start,
        )
      : undefined;
  const changedFocus =
    citationFocusActive &&
    !!focusCitation &&
    pieces.some(
      (piece) =>
        piece.source.sourceRef === focusCitation.sourceRef &&
        piece.source.field === focusCitation.field &&
            piece.source.blockRef === focusCitation.blockRef &&
            piece.source.asOf === focusCitation.asOf &&
        piece.source.sourceVersion !== focusCitation.sourceVersion,
    );
  const requestedSelection =
    selection ??
    (focusedPiece && focusCitation
      ? {
          piece: focusedPiece,
          start: focusCitation.start,
          end: focusCitation.end,
          text: Array.from(focusedPiece.source.text)
            .slice(focusCitation.start, focusCitation.end)
            .join(""),
        }
      : null);
  const refreshedPiece = requestedSelection && pieces.find(
    (piece) => piece.id === requestedSelection.piece.id &&
      piece.source.sourceVersion === requestedSelection.piece.source.sourceVersion,
  );
  const selected = requestedSelection && refreshedPiece ? {
    ...requestedSelection,
    piece: refreshedPiece,
    text: Array.from(refreshedPiece.source.text).slice(requestedSelection.start, requestedSelection.end).join(""),
  } : null;
  const selectedPaperId = selected?.piece.id ?? null;
  useEffect(() => {
    if (!active || !selectedPaperId) return;
    const host = paperScroller.current,
      paper = paperButtons.current.get(selectedPaperId);
    if (
      !host ||
      !paper ||
      !host.contains(paper) ||
      !host.clientHeight ||
      !host.clientWidth
    )
      return;
    const viewport = host.getBoundingClientRect(),
      target = paper.getBoundingClientRect();
    const topEdge = viewport.top + host.clientTop,
      leftEdge = viewport.left + host.clientLeft;
    const bottomEdge = topEdge + host.clientHeight,
      rightEdge = leftEdge + host.clientWidth;
    const nearestTop =
      target.top < topEdge
        ? host.scrollTop + target.top - topEdge
        : target.bottom > bottomEdge
          ? host.scrollTop + target.bottom - bottomEdge
          : host.scrollTop;
    const nearestLeft =
      target.left < leftEdge
        ? host.scrollLeft + target.left - leftEdge
        : target.right > rightEdge
          ? host.scrollLeft + target.right - rightEdge
          : host.scrollLeft;
    // Only this scroll surface moves. Instant nearest scrolling also respects reduced motion.
    if (nearestTop !== host.scrollTop || nearestLeft !== host.scrollLeft)
      host.scrollTo({
        top: nearestTop,
        left: nearestLeft,
        behavior: "instant",
      });
  }, [active, selectedPaperId, selection]);
  const current =
    docs.find(
      (item) =>
        item.documentId === (selected?.piece.documentId ?? activeDocumentId),
    ) ??
    docs.find((item) =>
      visible.some((piece) => piece.documentId === item.documentId),
    ) ??
    docs[0];
  const leaves = current ? readingLeaves(current, leafCapacity) : [];
  const selectedPage = selected
    ? leaves.findIndex((leaf) =>
        leaf.some((piece) => piece.id === selected.piece.id),
      )
    : -1;
  const currentSpread =
    selectedPage >= 0
      ? Math.floor(selectedPage / 2) * 2
      : Math.min(spread, Math.max(0, Math.floor((leaves.length - 1) / 2) * 2));
  const groups = new Map<string, ReadingPiece[]>();
  for (const piece of visible) {
    const anonymous =
      !piece.source.authorLabel.trim() ||
      /未知|匿名|未取得|不详/u.test(piece.source.authorLabel);
    const key =
      piece.source.sourceKind === "comment"
        ? `comment:${piece.documentId}`
        : anonymous
          ? `unknown:${piece.documentId}`
          : piece.source.authorLabel;
    groups.set(key, [...(groups.get(key) ?? []), piece]);
  }
  const commentGroups = [...groups.keys()].filter(key => key.startsWith("comment:"));
  const added =
    !!current &&
    document.materials.some((item) => item.documentId === current.documentId);
  function choose(piece: ReadingPiece) {
    releaseCitationFocus();
    setActiveDocumentId(piece.documentId);
    setSelection({
      piece,
      start: piece.start,
      end: piece.end,
      text: piece.text,
    });
    setError("");
  }
  function collected(piece: ReadingPiece) {
    return document.excerpts.some(
      (excerpt) =>
        excerpt.citation.sourceRef === piece.source.sourceRef &&
        excerpt.citation.sourceVersion === piece.source.sourceVersion &&
        excerpt.citation.field === piece.source.field &&
        excerpt.citation.blockRef === piece.source.blockRef &&
        excerpt.citation.asOf === piece.source.asOf &&
        excerpt.citation.start < piece.end &&
        excerpt.citation.end > piece.start,
    );
  }
  const canCollect =
    !!selected &&
    !!current &&
    selected.piece.documentId === current.documentId &&
    document.materials.some(
      (item) => item.documentId === selected.piece.documentId,
    ) &&
    current.qualification === "readable";
  async function collect() {
    if (!canCollect || !selected || !current) return;
    setBusy(true);
    setBusyAction("collect");
    setError("");
    try {
      const source = selected.piece.source;
      const anchor = await api.anchor({
        domainRef: domain,
        sourceKind: source.sourceKind,
        sourceRef: source.sourceRef,
        workRef: source.workRef,
        field: source.field,
        sourceVersion: source.sourceVersion,
        blockRef: source.blockRef,
        asOf: source.asOf,
        start: selected.start,
        end: selected.end,
      });
      onCollect(anchor, group);
    } catch (cause) {
      setError(api.errorText(cause));
    } finally {
      setBusy(false);
      setBusyAction(null);
    }
  }
  async function prepareAgain(terms: string[]) {
    setBusy(true);
    setBusyAction("prepare");
    setError("");
    const epoch = ++requestEpoch.current;
    const intent = JSON.stringify([
      document.title,
      document.angle,
      document.outline,
    ]);
    try {
      const result = await api.prepareMaterials({
        domainRef: domain,
        title: document.title,
        angle: document.angle,
        outline: document.outline,
        topicRef: document.topicRef,
        terms,
      });
      if (
        epoch !== requestEpoch.current ||
        intent !==
          JSON.stringify([
            liveDocument.current.title,
            liveDocument.current.angle,
            liveDocument.current.outline,
          ])
      ) {
        setError("选题意图已有变化，请按当前方案重新准备。");
        return;
      }
      const latest = liveDocument.current;
      const { methodVersion, query, terms: chosen, state, gaps } = result;
      onChange({
        ...latest,
        materials: addMaterials(
          latest.materials.filter((item) => item.addedBy === "manual"),
          result.documents.map((item) => selectMaterial(item, "prepared")),
        ),
        materialPreparation: {
          methodVersion,
          query,
          terms: chosen,
          state,
          gaps,
        },
      });
      setSelection(null);
      setActiveDocumentId(null);
    } catch (cause) {
      setError(api.errorText(cause));
    } finally {
      setBusy(false);
      setBusyAction(null);
    }
  }
  function captureBookSelection() {
    const chosen = window.getSelection();
    if (!chosen || chosen.isCollapsed || !chosen.rangeCount) return;
    const range = chosen.getRangeAt(0);
    const first =
      range.startContainer.parentElement?.closest("[data-fragment-id]");
    const last =
      range.endContainer.parentElement?.closest("[data-fragment-id]");
    const from = pieces.find(
        (piece) => piece.id === first?.getAttribute("data-fragment-id"),
      ),
      to = pieces.find(
        (piece) => piece.id === last?.getAttribute("data-fragment-id"),
      );
    if (
      !first ||
      !last ||
      !from ||
      !to ||
      from.source.fragmentId !== to.source.fragmentId
    )
      return;
    const before = range.cloneRange();
    before.selectNodeContents(first);
    before.setEnd(range.startContainer, range.startOffset);
    const start = from.start + Array.from(before.toString()).length,
      text = range.toString();
    if (
      Array.from(from.source.text)
        .slice(start, start + Array.from(text).length)
        .join("") !== text
    )
      return;
    setSelection({
      piece: from,
      start,
      end: start + Array.from(text).length,
      text,
    });
    setActiveDocumentId(from.documentId);
  }
  function addCurrent() {
    if (!current) return;
    onChange({
      ...document,
      materials: addMaterials(document.materials, [
        selectMaterial(current, "manual"),
      ]),
    });
  }
  function switchScope(next: "prepared" | "search") {
    releaseCitationFocus();
    setScope(next);
    setSelection(null);
    setActiveDocumentId(null);
    setSpread(0);
  }
  return (
    <div className={styles.materialBrowser}>
      <div className={styles.materialScope}>
        <nav className={styles.steps} aria-label="材料范围">
          <button
            aria-pressed={scope === "prepared"}
            onClick={() => switchScope("prepared")}
          >
            本篇材料{" "}
            <small>
              {prepared.isSuccess ? document.materials.length : "—"}
            </small>
          </button>
          <button
            aria-pressed={scope === "search"}
            onClick={() => switchScope("search")}
          >
            搜寻更多
          </button>
        </nav>
        {scope === "prepared" ? (
          <details>
            <summary>
              {prepared.isPending
                ? "材料准备中…"
                : prepared.isError
                  ? "材料准备失败"
                  : document.materialPreparation?.state === "insufficient"
                    ? "材料不足"
                    : "材料已准备"}
            </summary>
            <p>{document.materialPreparation?.query}</p>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                const data = new FormData(event.currentTarget);
                void prepareAgain(
                  String(data.get("terms") ?? "")
                    .split(/[、,，\s]+/u)
                    .filter(Boolean),
                );
              }}
            >
              <label>
                本篇检索词
                <input
                  key={document.materialPreparation?.terms.join(",")}
                  aria-label="本篇检索词"
                  name="terms"
                  defaultValue={
                    document.materialPreparation?.terms.join("、") ?? ""
                  }
                />
              </label>
              <ActionButton
                type="submit"
                variant="outline"
                busy={busyAction === "prepare"}
                disabled={busy}
              >
                {busy ? "准备中…" : "按这些词重新准备"}
              </ActionButton>
            </form>
            {document.materialPreparation?.gaps.map((gap) => (
              <p key={gap}>{gap}</p>
            ))}
            <p>按关键词匹配，相关程度请结合原文核对。</p>
            <ActionButton
              variant="outline"
              onClick={() => void prepared.refetch()}
            >
              重新读取
            </ActionButton>
          </details>
        ) : (
          <form
            className={styles.row}
            onSubmit={(event) => {
              event.preventDefault();
              releaseCitationFocus();
              setQuery(search);
              setCursor(0);
              setSelection(null);
              setActiveDocumentId(null);
            }}
          >
            <input
              aria-label="检索已取得材料"
              placeholder="检索其他笔记、原声或自有作品"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
            <ActionButton
              type="submit"
              variant="outline"
              busy={found.isFetching}
              disabled={found.isFetching}
            >
              检索
            </ActionButton>
          </form>
        )}
      </div>
      <div className={styles.filterbar}>
        <div role="group" aria-label="材料类型">
          {[
            { id: "all", label: "全部材料" },
            { id: "note", label: "原笔记" },
            { id: "comment", label: "用户原声" },
            { id: "ops_publication", label: "我方作品" },
          ].map((item) => (
            <button
              key={item.id}
              aria-pressed={kind === item.id}
              onClick={() => {
                releaseCitationFocus();
                setKind(item.id);
                setCursor(0);
                setSelection(null);
                setActiveDocumentId(null);
              }}
            >
              {item.label}
              <small>
                {countsReady
                  ? docs.filter(
                      (doc) => item.id === "all" || doc.sourceKind === item.id,
                    ).length
                  : "—"}
              </small>
            </button>
          ))}
        </div>
        <input
          aria-label="搜索材料中的片段"
          placeholder="找一句话、一个细节…"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
        />
      </div>
      <div className={styles.filterbar}>
        <div role="group" aria-label="片段类型" aria-description={READING_HINT_NOTICE}>
          {["全部", ...FRAGMENT_TYPES].map((item) => (
            <button
              key={item}
              aria-pressed={type === item}
              onClick={() => {
                releaseCitationFocus();
                setType(item);
                setSelection(null);
              }}
            >
              {item}
              <small>
                {countsReady
                  ? pieces.filter(
                      (piece) => item === "全部" || piece.type === item,
                    ).length
                  : "—"}
              </small>
            </button>
          ))}
        </div>
        <small title={READING_HINT_NOTICE}>本机词语提示 · 未核对</small>
      </div>
      {collection.error ? (
        <p role="alert" className={styles.error}>
          {api.errorText(collection.error)}{" "}
          <ActionButton
            variant="outline"
            onClick={() => void collection.refetch()}
          >
            重试读取
          </ActionButton>
        </p>
      ) : null}
      {changedFocus ? (
        <p className={styles.warning}>
          旧摘录的来源版本已变化。当前显示最新原文，请重新选择准确片段后使用。
        </p>
      ) : null}
      <div className={styles.materialStage}>
        <section
          ref={readingHost}
          className={styles.readingSpace}
          aria-label="原文阅读"
        >
          <div className={styles.bookMeta}>
            <span>
              {current
                ? `${SOURCE_LABELS[current.sourceKind]} · ${current.authorLabel}`
                : "选择一份材料"}
            </span>
            <span>
              {current
                ? current.qualification === "readable"
                  ? "已取得原文"
                  : current.qualification === "changed"
                    ? "来源已变化，请重新核对"
                    : current.qualification === "restricted"
                      ? "来源已受限"
                      : "来源暂不可用"
                : ""}
            </span>
            {current ? (
              <details className={styles.sourceDrawer}>
                <summary>来源详情</summary>
                <SourceContextView value={current.context} />
                {selected ? (
                  <OcrEvidence
                    piece={selected.piece}
                    context={current.context}
                    start={selected.start}
                    end={selected.end}
                  />
                ) : null}
                {current.matchReasons.map((reason) => (
                  <p key={reason}>{reason}</p>
                ))}
                <p>来源版本：{current.sourceVersion}</p>
                {current.qualification === "changed" ? (
                  <ActionButton
                    variant="outline"
                    onClick={() =>
                      onChange({
                        ...document,
                        materials: document.materials.map((item) =>
                          item.documentId === current.documentId
                            ? { ...item, sourceVersion: current.sourceVersion }
                            : item,
                        ),
                      })
                    }
                  >
                    已核对，采用当前来源版本
                  </ActionButton>
                ) : null}
                {scope === "prepared" ? (
                  <ActionButton
                    variant="outline"
                    onClick={() => {
                      onChange({
                        ...document,
                        materials: document.materials.filter(
                          (item) => item.documentId !== current.documentId,
                        ),
                      });
                      setSelection(null);
                      setActiveDocumentId(null);
                    }}
                  >
                    从本篇移除材料
                  </ActionButton>
                ) : null}
              </details>
            ) : null}
          </div>
          <div
            className={styles.openBook}
            onMouseUp={captureBookSelection}
            onKeyUp={captureBookSelection}
            key={`${current?.documentId}:${currentSpread}`}
          >
            {[0, 1].map((side) => (
              <article
                className={side ? styles.rightLeaf : styles.leftLeaf}
                key={side}
              >
                <div className={styles.running}>
                  {current?.title ?? "材料书页"}
                </div>
                {current && leaves[currentSpread + side]?.length ? (
                  <>
                    {currentSpread === 0 &&
                    side === 0 &&
                    !current.fragments.some(
                      (fragment) => fragment.field === "title",
                    ) ? (
                      <h2>{current.title}</h2>
                    ) : null}
                    {leaves[currentSpread + side]?.map((piece) => (
                      <BookPassage
                        key={piece.id}
                        piece={piece}
                        selected={selected}
                        matching={visible.some((item) => item.id === piece.id)}
                        onSelect={(value) => {
                          setSelection(value);
                          setActiveDocumentId(value.piece.documentId);
                        }}
                      />
                    ))}
                  </>
                ) : collection.isFetching ? (
                  <p>读取来源中…</p>
                ) : current && currentSpread + side >= leaves.length ? (
                  <>
                    <SourceContextView value={current.context} />
                    <p className={styles.muted}>
                      {current.qualification === "readable"
                        ? "本篇已取得内容到此。"
                        : "当前来源内容无法阅读。"}
                    </p>
                  </>
                ) : (
                  <p className={styles.empty}>
                    {scope === "search" && !query
                      ? "检索后先预览原文，再加入本篇。"
                      : "本篇尚无可读材料，可搜寻更多。"}
                  </p>
                )}
                <footer className={styles.leafFooter}>
                  {current
                    ? currentSpread + side < leaves.length
                      ? `${currentSpread + side + 1} / ${leaves.length}`
                      : ""
                    : "—"}
                </footer>
              </article>
            ))}
          </div>
          <div className={styles.bookNav}>
            <button
              disabled={currentSpread <= 0}
              onClick={() => {
                setSelection(null);
                setSpread(Math.max(0, currentSpread - 2));
                if (current) setActiveDocumentId(current.documentId);
                releaseCitationFocus();
              }}
            >
              ← 上一展开页
            </button>
            <span>
              {current
                ? `${currentSpread + 1}–${Math.min(currentSpread + 2, leaves.length)} / ${leaves.length} 页`
                : "未选择"}
            </span>
            <button
              disabled={currentSpread + 2 >= leaves.length}
              onClick={() => {
                setSelection(null);
                setSpread(currentSpread + 2);
                if (current) setActiveDocumentId(current.documentId);
                releaseCitationFocus();
              }}
            >
              下一展开页 →
            </button>
          </div>
          <MaterialDistribution
            documents={docs}
            pieces={pieces}
            visible={visible}
            selectedId={selected?.piece.id ?? null}
            currentDocumentId={current?.documentId ?? null}
            countsReady={countsReady}
            onChoose={choose}
            onOpenDocument={(documentId) => {
              releaseCitationFocus();
              setActiveDocumentId(documentId);
              setSelection(null);
              setSpread(0);
            }}
          />
        </section>
        <section className={styles.snippetWall} aria-label="分组摘录纸条">
          <div className={styles.wallMeta}>
            <h2>片段</h2>
            <small>
              {countsReady ? visible.length : "—"} 个片段 ·{" "}
              {countsReady
                ? new Set(visible.map((piece) => piece.documentId)).size
                : "—"}{" "}
              份材料
            </small>
          </div>
          <div
            ref={paperScroller}
            data-material-paper-scroll
            className={styles.snippetGroups}
          >
            <div className={styles.snippetColumns}>
              {[...groups.entries()].map(([label, items], index) => (
                <section
                  className={styles.snippetGroup}
                  key={label}
                  data-tone={index % 3}
                >
                  <h3>
                    <button
                      onClick={() => {
                        if (label.startsWith("unknown:") || label.startsWith("comment:")) {
                          const first = items[0];
                          if (first) choose(first);
                        } else setFilter(items[0]?.source.authorLabel ?? "");
                      }}
                    >
                      {label.startsWith("unknown:") || label.startsWith("comment:")
                        ? (docs.find(
                            (doc) => doc.documentId === items[0]?.documentId,
                          )?.title ?? "未命名原篇")
                        : label}
                      {label.startsWith("comment:") ? <small> · 原声 {commentGroups.indexOf(label) + 1}</small> : null}
                    </button>{" "}
                    <small>{items.length}</small>
                  </h3>
                  <div>
                    {items.map((piece) => (
                      <button
                        type="button"
                        key={piece.id}
                        className={styles.paper}
                        ref={(node) => {
                          if (node) paperButtons.current.set(piece.id, node);
                          else paperButtons.current.delete(piece.id);
                        }}
                        aria-pressed={selected?.piece.id === piece.id}
                        data-fragment-id={piece.id}
                        data-collected={collected(piece)}
                        onClick={() => choose(piece)}
                      >
                        {piece.text}
                        {collected(piece) ? <small> ✓</small> : null}
                      </button>
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </div>
          {!visible.length && collection.isSuccess ? (
            <p className={styles.empty}>
              没有匹配片段。可更换筛选或搜寻更多材料。
            </p>
          ) : null}
          {scope === "search" && found.data ? (
            <div className={styles.row}>
              <ActionButton
                variant="outline"
                disabled={!cursor}
                onClick={() => {
                  releaseCitationFocus();
                  setSelection(null);
                  setActiveDocumentId(null);
                  setSpread(0);
                  setCursor(Math.max(0, cursor - 24));
                }}
              >
                上一页来源
              </ActionButton>
              <span>第 {Math.floor(cursor / 24) + 1} 页</span>
              <ActionButton
                variant="outline"
                disabled={found.data.nextCursor === null}
                onClick={() => {
                  releaseCitationFocus();
                  setSelection(null);
                  setActiveDocumentId(null);
                  setSpread(0);
                  setCursor(found.data?.nextCursor ?? 0);
                }}
              >
                更多来源
              </ActionButton>
            </div>
          ) : null}
        </section>
      </div>
      <div className={styles.selectedStrip}>
        <div>
          <small>
            {selected
              ? `${SOURCE_LABELS[selected.piece.source.sourceKind]} · ${selected.piece.source.authorLabel}`
              : (current?.title ?? "未选择片段")}
          </small>
          {selected ? <blockquote>{selected.text}</blockquote> : null}
          {error ? (
            <p role="alert" className={styles.error}>
              {error}
            </p>
          ) : null}
        </div>
        <div className={styles.selectedActions}>
          {scope === "search" ? (
            <ActionButton
              variant="outline"
              className={!added ? styles.primary : undefined}
              disabled={
                !current || added || current.qualification !== "readable"
              }
              onClick={addCurrent}
            >
              {added ? "已加入本篇" : "加入本篇材料"}
            </ActionButton>
          ) : null}
          <div className={styles.row}>
            <select
              aria-label="摘录归组"
              value={group}
              onChange={(event) => setGroup(event.target.value as CanvasGroup)}
            >
              {GROUPS.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.label}
                </option>
              ))}
            </select>
            <ActionButton
              variant="primary"
              busy={busyAction === "collect"}
              disabled={!canCollect || busy}
              onClick={() => void collect()}
            >
              {busyAction === "collect" ? "核验中…" : "摘入画布"}
            </ActionButton>
          </div>
          {onCanvas ? (
            <ActionButton variant="quiet" onClick={onCanvas}>
              查看画布 · {document.canvas.length}
            </ActionButton>
          ) : null}
        </div>
      </div>
    </div>
  );
}
