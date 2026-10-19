export interface DiffPart {
  type: "same" | "delete" | "insert";
  text: string;
}
export interface TextComparison {
  parts: DiffPart[];
  coarse: boolean;
}
// Bound both retained paths and comparisons before falling back to whole-span markup.
// The original and edited strings are always retained verbatim.
const MAX_PATH_DEPTH = 256;
const MAX_COMPARISONS = 60000;
/** Myers' shortest edit path operates on Unicode scalar values, never UTF-16 halves. */
function shortestDiff(before: string, after: string): DiffPart[] | null {
  const a = Array.from(before),
    b = Array.from(after),
    trace: Map<number, number>[] = [];
  let frontier = new Map<number, number>([[1, 0]]);
  let finish = 0, operations = 0;
  outer: for (let depth = 0; depth <= a.length + b.length; depth++) {
    if (depth > MAX_PATH_DEPTH) return null;
    trace.push(new Map(frontier));
    for (let diagonal = -depth; diagonal <= depth; diagonal += 2) {
      if (++operations > MAX_COMPARISONS) return null;
      let x =
        diagonal === -depth ||
        (diagonal !== depth &&
          (frontier.get(diagonal - 1) ?? -1) <
            (frontier.get(diagonal + 1) ?? -1))
          ? (frontier.get(diagonal + 1) ?? 0)
          : (frontier.get(diagonal - 1) ?? 0) + 1;
      let y = x - diagonal;
      while (x < a.length && y < b.length && a[x] === b[y]) {
        if (++operations > MAX_COMPARISONS) return null;
        x++;
        y++;
      }
      frontier.set(diagonal, x);
      if (x >= a.length && y >= b.length) {
        finish = depth;
        break outer;
      }
    }
    frontier = new Map(frontier);
  }
  let x = a.length,
    y = b.length;
  const atoms: { type: DiffPart["type"]; text: string }[] = [];
  for (let depth = finish; depth >= 0; depth--) {
    const previous = trace[depth]!;
    const diagonal = x - y;
    const priorDiagonal =
      diagonal === -depth ||
      (diagonal !== depth &&
        (previous.get(diagonal - 1) ?? -1) < (previous.get(diagonal + 1) ?? -1))
        ? diagonal + 1
        : diagonal - 1;
    const priorX = previous.get(priorDiagonal) ?? 0,
      priorY = priorX - priorDiagonal;
    while (x > priorX && y > priorY) {
      atoms.push({ type: "same", text: a[--x]! });
      y--;
    }
    if (!depth) break;
    if (x === priorX) atoms.push({ type: "insert", text: b[--y]! });
    else atoms.push({ type: "delete", text: a[--x]! });
  }
  const parts: DiffPart[] = [];
  for (const atom of atoms.reverse()) {
    const last = parts.at(-1);
    if (last?.type === atom.type) last.text += atom.text;
    else parts.push({ ...atom });
  }
  return parts;
}

export function compareText(before: string, after: string): TextComparison {
  const a = Array.from(before),
    b = Array.from(after);
  let prefix = 0,
    suffix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix])
    prefix++;
  while (
    suffix < a.length - prefix &&
    suffix < b.length - prefix &&
    a[a.length - suffix - 1] === b[b.length - suffix - 1]
  )
    suffix++;
  const left = a.slice(prefix, a.length - suffix).join(""),
    right = b.slice(prefix, b.length - suffix).join("");
  const chars = new Set(Array.from(left));
  const shared = Array.from(right).some((char) => chars.has(char));
  const exact = shared ? shortestDiff(left, right) : null;
  const middle: DiffPart[] = exact ?? [
    ...(left ? [{ type: "delete" as const, text: left }] : []),
    ...(right ? [{ type: "insert" as const, text: right }] : []),
  ];
  return { coarse: shared && exact === null, parts: [
    ...(prefix
      ? [{ type: "same" as const, text: a.slice(0, prefix).join("") }]
      : []),
    ...middle,
    ...(suffix
      ? [{ type: "same" as const, text: a.slice(a.length - suffix).join("") }]
      : []),
  ] };
}

export function textDiff(before: string, after: string): DiffPart[] {
  return compareText(before, after).parts;
}
