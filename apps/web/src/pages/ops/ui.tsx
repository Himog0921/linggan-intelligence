import { ActionButton } from '../../shared/ActionButton';
import { useEffect, useRef, type ReactNode } from "react";
import { c } from "./styles";
import { count, safeHref } from "./format";
export function Missing({
  title = "没有值或未知",
  children = "—",
}: {
  title?: string;
  children?: ReactNode;
}) {
  return (
    <span className={c("ops-missing")} title={title}>
      {children}
    </span>
  );
}
export function Tag({
  children,
  tone,
  title,
}: {
  children: ReactNode;
  tone?: string;
  title?: string;
}) {
  return (
    <span className={c("ops-tag")} data-tone={tone} title={title}>
      {children}
    </span>
  );
}
export function NumberCell({ value }: { value: number | null }) {
  const f = count(value);
  return (
    <td className={c("ops-number")}>
      {f.missing ? (
        <Missing />
      ) : (
        <span title={f.title || undefined}>{f.text}</span>
      )}
    </td>
  );
}
export function TextCell({
  value,
  className = "ops-ellipsis",
}: {
  value: string | null;
  className?: string;
}) {
  return (
    <td>
      {value ? (
        <span className={c(className)} title={value}>
          {value}
        </span>
      ) : (
        <Missing />
      )}
    </td>
  );
}
export function External({
  href,
  children,
  title,
}: {
  href: string;
  children: ReactNode;
  title?: string;
}) {
  const safe = safeHref(href);
  return safe ? (
    <a
      className={c("ops-ellipsis")}
      href={safe}
      target="_blank"
      rel="noopener noreferrer"
      title={title}
    >
      {children}
    </a>
  ) : (
    <span className={c("ops-ellipsis")} title={title}>
      {children}
    </span>
  );
}
import { readonly } from "./styles";
export function DisabledActions() {
  return (
    <td>
      <div className={c("ops-row-actions")}>
        <ActionButton type="button" disabled title={readonly}>
          编辑
        </ActionButton>
        <ActionButton type="button" disabled title={readonly}>
          删除
        </ActionButton>
      </div>
    </td>
  );
}
export function Tabs({
  label,
  values,
  current,
  onSelect,
  id,
  compact = false,
}: {
  label: string;
  values: readonly (readonly [string, string])[];
  current: string;
  onSelect: (value: string) => void;
  id?: string;
  compact?: boolean;
}) {
  return (
    <nav
      className={c("ops-tabs", ...(compact ? ["ops-compact-tabs"] : []))}
      aria-label={label}
      id={id}
    >
      {values.map(([value, text]) => (
        <button
          key={value}
          type="button"
          aria-pressed={current === value}
          onClick={() => onSelect(value)}
        >
          {text}
        </button>
      ))}
    </nav>
  );
}
export function Status({
  children,
  error = false,
  retry,
}: {
  children?: ReactNode;
  error?: boolean;
  retry?: () => void;
}) {
  return (
    <p
      id="ops-status"
      role="status"
      aria-live="polite"
      data-error={error ? "true" : "false"}
    >
      {children}
      {retry ? (
        <>
          {" "}
          <ActionButton type="button" onClick={retry}>
            重试
          </ActionButton>
        </>
      ) : null}
    </p>
  );
}
export function Empty({
  columns,
  text,
  action,
}: {
  columns: number;
  text: string;
  action?: ReactNode;
}) {
  return (
    <tr>
      <td colSpan={columns} className={c("ops-empty")}>
        <p>{text}</p>
        {action}
      </td>
    </tr>
  );
}
export function Dialog({
  title,
  label,
  children,
  onClose,
  className,
  outsideClose = false,
}: {
  title?: string;
  label?: string;
  children: ReactNode;
  onClose: () => void;
  className?: string;
  outsideClose?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const el = ref.current;
    const prior = document.activeElement;
    el?.showModal();
    return () => {
      el?.close();
      if (prior instanceof HTMLElement && prior.isConnected) prior.focus();
    };
  }, []);
  return (
    <dialog
      ref={ref}
      className={c("ops-dialog", ...(className ? [className] : []))}
      aria-labelledby={title}
      aria-label={label}
      onClick={event => {
        if (!outsideClose || event.target !== event.currentTarget) return;
        const rect = event.currentTarget.getBoundingClientRect();
        if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) onClose();
      }}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      {children}
    </dialog>
  );
}
