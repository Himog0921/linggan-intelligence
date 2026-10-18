import type { MaterialDocument, SourceItem } from "./workbenchModel";
export const FRAGMENT_TYPES = [
  "场景",
  "观点",
  "做法",
  "原声",
  "边界",
  "待分类",
] as const;
export type FragmentType = (typeof FRAGMENT_TYPES)[number];
export interface ReadingPiece {
  id: string;
  documentId: string;
  source: SourceItem;
  start: number;
  end: number;
  text: string;
  type: FragmentType;
}
export function fragmentType(source: SourceItem, text: string): FragmentType {
  if (source.sourceKind === "comment") return "原声";
  if (source.field === "title" || source.field === "cover_headline")
    return "待分类";
  if (/并非|不代表|不一定|不能|但是|不过|需要注意|边界|例外|未必/u.test(text))
    return "边界";
  if (
    /可以试|先把|第一步|第二步|步骤|试着|建议|做法|让孩子|设定|拆成|然后/u.test(
      text,
    )
  )
    return "做法";
  if (
    /昨天|今天|每次|有一次|放学|晚上|早上|当.*时|一到|坐在|看见|场景/u.test(
      text,
    )
  )
    return "场景";
  if (/我认为|我觉得|关键在于|意味着|真正的问题|本质是|不是.*而是/u.test(text))
    return "观点";
  return "待分类";
}
/** Exact scalar ranges are never normalised; paper and book share these identities. */
export function materialPieces(document: MaterialDocument): ReadingPiece[] {
  return document.fragments.flatMap((source) => {
    const chars = Array.from(source.text);
    const straightQuoteParity = { '"': new Uint8Array(chars.length + 1), "'": new Uint8Array(chars.length + 1) };
    for (const quote of ['"', "'"] as const) {
      const parity = straightQuoteParity[quote];
      for (let i = 0; i < chars.length; i++) parity[i + 1] = parity[i]! ^ (chars[i] === quote ? 1 : 0);
    }
    const pieces: ReadingPiece[] = [];
    let start = 0;
    for (let end = 1; end <= chars.length; end++) {
      if (!/[。！？\n]/u.test(chars[end - 1] ?? "") && end !== chars.length) continue;
      while (end < chars.length && /[。！？”’」』）)\]】》〉〕｝}"']/u.test(chars[end] ?? "")) {
        const mark = chars[end]!;
        if ((mark === '"' || mark === "'") && straightQuoteParity[mark][end] === 0) break;
        end++;
      }
      while (end < chars.length && /\s/u.test(chars[end] ?? "")) end++;
      const text = chars.slice(start, end).join("");
      if (text.trim())
        pieces.push({
          id: `${source.fragmentId}:${start}:${end}`,
          documentId: document.documentId,
          source,
          start,
          end,
          text,
          type: fragmentType(source, text),
        });
      start = end;
    }
    return pieces;
  });
}
/** Flow consecutive fields into reading leaves; a field boundary is never a document end. */
export function readingLeaves(
  document: MaterialDocument,
  capacity = 400,
): ReadingPiece[][] {
  const leaves: ReadingPiece[][] = [];
  let leaf: ReadingPiece[] = [],
    size = 0;
  for (const piece of materialPieces(document)) {
    const length = Array.from(piece.text).length;
    if (leaf.length && size + length > capacity) {
      leaves.push(leaf);
      leaf = [];
      size = 0;
    }
    leaf.push(piece);
    size += length;
  }
  if (leaf.length) leaves.push(leaf);
  return leaves;
}
