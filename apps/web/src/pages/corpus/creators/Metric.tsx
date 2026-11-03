import { formatCount } from './creatorsQuery';

/** 数字单元：完整千分位与等宽数字；未知是「—」，0 就是 0。 */
export function Metric({ value, className }: { value: number | null; className?: string }) {
  return (
    <span className={className} data-unknown={value === null || undefined}>
      {value === null ? '—' : formatCount(value)}
    </span>
  );
}
