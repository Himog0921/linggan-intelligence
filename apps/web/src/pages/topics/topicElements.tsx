import type { ButtonHTMLAttributes, ReactNode } from "react";
import { TopicButton } from "./TopicButton";
import { attributes } from "./elementProps";
import { c as cls } from "./styles";
import { esc } from "./topicModel";
export const btn = (
  text: ReactNode,
  action: string,
  id = "",
  buttonClass = "",
  extra: string | ButtonHTMLAttributes<HTMLButtonElement> = "",
) => {
  const props = typeof extra === "string" ? attributes(extra) : extra;
  const Button = Object.prototype.hasOwnProperty.call(props, "data-mutation")
    ? TopicButton
    : "button";
  return (
    <>
      <Button
        type="button"
        className={cls("lgi-tm-button " + String(buttonClass ?? ""))}
        data-action={action}
        data-id={id}
        {...props}
      >
        {text}
      </Button>
    </>
  );
};
export const tab = (
  text: ReactNode,
  action: string,
  id: string | undefined,
  selected: boolean,
) =>
  btn(
    esc(text),
    action,
    id || "",
    `lgi-tm-tab ${selected ? "is-active" : ""}`,
    `aria-pressed="${selected}"`,
  );
/** LIDS badge (Mog 2026-11-27): coloured text + a leading square, no fill, no frame. `true` keeps the old signal call sites. */
export type BadgeTone = "category" | "signal" | "unknown" | "warning" | "success" | "danger" | "info";
export const badge = (text: unknown, tone: boolean | BadgeTone = false) => {
  const resolved: BadgeTone = tone === true ? "signal" : tone === false ? "category" : tone;
  return (
    <>
      <span className={cls("lgi-tm-badge is-" + resolved)}>{esc(text)}</span>
    </>
  );
};
export const empty = (text: ReactNode, detail = "") => (
  <>
    <div className={cls("lgi-tm-empty")}>
      <strong>{esc(text)}</strong>
      {detail ? (
        <>
          <p>{esc(detail)}</p>
        </>
      ) : (
        ""
      )}
    </div>
  </>
);
/** JSX arrays are truthy even when empty; preserve the old empty-result state explicitly. */
export const withEmpty = (rows: ReactNode[], fallback: ReactNode) =>
  rows.length ? rows : fallback;
