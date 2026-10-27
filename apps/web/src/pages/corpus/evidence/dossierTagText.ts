/** The canonical owner's pure-tag grammar. Never applied to ordinary body blocks. */
export function normalizedTagLabel(text: string): string { return text.trim().replace(/\[话题\]$/u, '').trim(); }
export function pureTagLabels(text: string): string[] | null {
  let rest = text.trim();
  const tags: string[] = [];
  while (rest) {
    const match = /^#([^#]+)#/u.exec(rest);
    if (!match?.[1]) return null;
    const label = normalizedTagLabel(match[1]);
    if (!label) return null;
    tags.push(label);
    rest = rest.slice(match[0].length).trimStart();
  }
  return tags.length ? tags : null;
}
