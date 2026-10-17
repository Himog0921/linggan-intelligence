import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from 'react';
import {
  LANES,
  LANE_STATES,
  LAYOUTS,
  MEDIA_KINDS,
  VIEWS,
  activeFilterCount,
  type EvidenceQuery,
  type LayoutKey,
  type ViewKey,
} from './evidenceQuery';
import page from './EvidencePage.module.css';
import styles from './Controls.module.css';

/** 来自页面的改动入口：`replace` 为真时替换当前历史，否则新增一条。 */
export type Change = (update: (current: EvidenceQuery) => EvidenceQuery, replace?: boolean) => void;

interface EvidenceControlsProps {
  /** 控件显示的查询状态；没有领域时是默认值（控件置灰，不读不写）。 */
  query: EvidenceQuery;
  hasDomain: boolean;
  onChange: Change;
  /** 提交检索（Enter 或「读取材料」）：条件没变也要重新读取，所以交给页面处理。 */
  onSubmit: (text: string) => void;
  onClearFilters: () => void;
  /** 查询区顶部的返回横幅。 */
  banners?: ReactNode;
  /** 查询区底部的勾选与批量动作。 */
  children?: ReactNode;
}

const NO_DOMAIN = '请先选择研究领域，再读取统一作品材料。';

type FilterName = 'lane' | 'laneState' | 'mediaKind';

const FILTERS: readonly { name: FilterName; label: string; allLabel: string; options: readonly (readonly [string, string])[] }[] = [
  { name: 'lane', label: '材料通道', allLabel: '全部材料通道', options: LANES },
  { name: 'laneState', label: '通道状态', allLabel: '全部状态', options: LANE_STATES },
  { name: 'mediaKind', label: '媒体类型', allLabel: '全部类型', options: MEDIA_KINDS },
];

/**
 * 检索框、筛选浮层、系统视图与排版切换。检索只在提交（Enter 或「读取材料」）时进入地址，没有自动搜索和防抖；
 * 只检索本机已接纳的材料，不触发平台搜索。下拉是原生 `<select>`：键盘与读屏由浏览器承担，变更即重新读取。
 * 浮层一次只开一个：Esc 或点击外部关闭，关闭后焦点回到筛选按钮。
 */
export function EvidenceControls({ query, hasDomain, onChange, onSubmit, onClearFilters, banners, children }: EvidenceControlsProps) {
  const panelId = useId();
  const [text, setText] = useState(query.q);
  const [seen, setSeen] = useState(query.q);
  // 地址里的检索词变了（后退、前进、清空）：输入框跟着变，而不是留着旧的文字。
  if (seen !== query.q) {
    setSeen(query.q);
    setText(query.q);
  }
  const [open, setOpen] = useState(false);
  const toggle = useRef<HTMLButtonElement>(null);
  const anchor = useRef<HTMLDivElement>(null);
  const reason = hasDomain ? undefined : NO_DOMAIN;

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !anchor.current?.contains(event.target)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);

  function submit(event: FormEvent) {
    event.preventDefault();
    if (!hasDomain) return;
    setOpen(false);
    onSubmit(text);
  }

  const count = activeFilterCount(query);
  return (
    <section aria-label="材料检索与筛选" className={styles.deck}>
      {banners}
      <form className={styles.form} onSubmit={submit}>
        <div className={styles.find}>
          <span className={styles.findMark} aria-hidden="true">
            FIND
          </span>
          <input
            type="search"
            name="q"
            autoComplete="off"
            aria-label="检索作品与证据"
            placeholder="标题、作者或原声片段……"
            value={text}
            disabled={!hasDomain}
            title={reason}
            onChange={event => setText(event.currentTarget.value)}
          />
        </div>
        <div className={styles.tools}>
          <div
            ref={anchor}
            className={styles.anchor}
            onKeyDown={event => {
              if (event.key === 'Escape' && open) {
                event.preventDefault();
                event.stopPropagation();
                setOpen(false);
                toggle.current?.focus();
              }
            }}
          >
            <button
              ref={toggle}
              type="button"
              className={styles.filterToggle}
              aria-expanded={open}
              aria-controls={panelId}
              data-active={count > 0}
              disabled={!hasDomain}
              title={reason}
              onClick={() => setOpen(value => !value)}
            >
              <span>筛选</span>
              <b aria-label={`已选 ${count} 项筛选`}>{count}</b>
            </button>
            {open ? (
              <div id={panelId} role="group" aria-label="筛选条件" className={styles.panel}>
                {FILTERS.map(({ name, label, allLabel, options }) => (
                  <label key={name} className={styles.field} data-active={query[name] !== ''}>
                    <span className={styles.fieldLabel}>{label}</span>
                    <select
                      aria-label={label}
                      value={query[name]}
                      onChange={event => {
                        const value = event.currentTarget.value;
                        onChange(current => ({ ...current, [name]: value, work: null }));
                      }}
                    >
                      <option value="">{allLabel}</option>
                      {options.map(([value, optionLabel]) => (
                        <option key={value} value={value}>
                          {optionLabel}
                        </option>
                      ))}
                    </select>
                  </label>
                ))}
                <div className={styles.panelFoot}>
                  <button type="button" onClick={onClearFilters}>
                    清空筛选
                  </button>
                </div>
              </div>
            ) : null}
          </div>
          <button type="submit" className={page.primary} disabled={!hasDomain} title={reason}>
            读取材料
          </button>
        </div>
      </form>
      <div className={styles.foot}>
        <div className={styles.views} role="group" aria-label="按材料状态快速筛选">
          <span className={styles.deckLabel}>系统视图</span>
          {VIEWS.map(([key, label]) => (
            <button
              key={key}
              type="button"
              aria-pressed={query.view === key}
              disabled={!hasDomain}
              title={reason}
              onClick={() => onChange(current => ({ ...current, view: key satisfies ViewKey, work: null }))}
            >
              {label}
            </button>
          ))}
        </div>
        <div className={styles.layouts} role="group" aria-label="结果排版">
          {LAYOUTS.map(([key, label]) => (
            <button key={key} type="button" aria-pressed={query.layout === key} onClick={() => onChange(current => ({ ...current, layout: key satisfies LayoutKey }))}>
              {label}
            </button>
          ))}
        </div>
      </div>
      {children}
    </section>
  );
}
