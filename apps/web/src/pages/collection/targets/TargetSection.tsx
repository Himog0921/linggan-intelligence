import type { ReactNode } from 'react';
export function Section({
  title,
  note,
  children,
  className = '',
  id,
}: {
  title: string;
  note: string;
  children: ReactNode;
  className?: string;
  id?: string;
}) {
  return (
    <section className={`c-dw-section ${className}`} id={id}>
      <div className="c-dw-section-head">
        <b>{title}</b>
        <span>{note}</span>
      </div>
      {children}
    </section>
  );
}
export function Readouts({ values }: { values: [string | number | null, string][] }) {
  return (
    <div className="c-dw-readouts c-dw-overview-readouts">
      {values.map(([v, label]) => (
        <div key={label}>
          <b>{v ?? '尚未取得'}</b>
          <span>{label}</span>
        </div>
      ))}
    </div>
  );
}
