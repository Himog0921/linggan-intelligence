import type { CSSProperties } from "react";
const permitted =
  /^(aria-[a-z-]+|data-[a-z-]+|title|disabled|checked|selected|open|required|hidden|value)$/;
/** Authored attribute fragments only; values still become escaped React props, never HTML. */
export function attributes(source: string | boolean | undefined) {
  const props: Record<string, string | boolean | CSSProperties> = {};
  if (typeof source !== "string") return props;
  for (const match of source.matchAll(
    /([^\s=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s]+)))?/g,
  )) {
    const name = match[1] || "";
    const value = match[2] ?? match[3] ?? match[4];
    if (name === "style") props.style = inlineStyle(value || "");
    else if (permitted.test(name))
      props[name === "checked" ? "defaultChecked" : name] =
        value === undefined ? true : value;
  }
  return props;
}
export function inlineStyle(source: string): CSSProperties {
  const props: Record<string, string> = {};
  for (const rule of source.split(";")) {
    const i = rule.indexOf(":");
    if (i < 0) continue;
    const key = rule.slice(0, i).trim();
    const value = rule.slice(i + 1).trim();
    if (!key || !value) continue;
    props[
      key.startsWith("--")
        ? key
        : key.replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase())
    ] = value;
  }
  return props;
}
