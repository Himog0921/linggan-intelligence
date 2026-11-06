import type { ReactNode } from 'react';
import styles from './ViewSwitch.module.css';

export interface ViewSwitchOption<K extends string> {
  key: K;
  label: ReactNode;
  /** 可选读数；它是范围或数量，不冒充筛选命中。 */
  count?: ReactNode;
  disabled?: boolean;
  /** 不可用原因；仅在 disabled 时作为可读说明。 */
  reason?: string;
}

export interface ViewSwitchProps<K extends string> {
  /** 这组切换的共同对象，例如「结果排版」「观察数据视图」。 */
  label: string;
  value: K;
  options: readonly ViewSwitchOption<K>[];
  onChange: (key: K) => void;
  className?: string;
}

/**
 * 同一输入集合的展示方式切换（LIDS C26 / ADR-11）：文字页签 + 3px 信号线。
 * 它只换看法，不执行命令；范围、查询与数据仍由页面持有。
 */
export function ViewSwitch<K extends string>({ label, value, options, onChange, className }: ViewSwitchProps<K>) {
  return (
    <div role="group" aria-label={label} className={[styles.switch, className].filter(Boolean).join(' ')} data-lids-component="C26">
      {options.map(option => (
        <button
          key={option.key}
          type="button"
          aria-pressed={option.key === value}
          disabled={option.disabled}
          title={option.disabled ? option.reason : undefined}
          onClick={() => {
            if (option.key !== value) onChange(option.key);
          }}
        >
          <span>{option.label}</span>
          {option.count === undefined ? null : <small className={styles.count}>{option.count}</small>}
        </button>
      ))}
    </div>
  );
}
