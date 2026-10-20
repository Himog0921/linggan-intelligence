import { useEffect, useImperativeHandle, useRef, useState, type Ref } from 'react';
import { CreatorSelect } from './CreatorSelect';
import styles from './CreatorsPage.module.css';

export interface SearchFieldHandle {
  /** 立即提交还在等待防抖的输入（例如打开筛选之前）。 */
  flush: () => void;
  focus: () => void;
}

interface SearchFieldProps {
  mode: 'author' | 'work';
  /** 已提交（出现在地址里）的搜索词。 */
  value: string;
  onCommit: (text: string) => void;
  onModeChange: (mode: 'author' | 'work', text: string) => void;
  handle?: Ref<SearchFieldHandle>;
}

const DEBOUNCE_MS = 250;

/**
 * 搜索框：停止输入 250ms 后提交；中文输入法组词期间不发送查询，组词结束后才提交。
 * 地址里的搜索词变化（后退、移除条件、快捷视图）时，输入框跟着变。
 */
export function SearchField({ mode, value, onCommit, onModeChange, handle }: SearchFieldProps) {
  const [text, setText] = useState(value);
  // 外部（地址）改变了搜索词：输入框同步，不在 effect 里补一次渲染。
  const [seen, setSeen] = useState(value);
  if (value !== seen) {
    setSeen(value);
    setText(value);
  }
  const input = useRef<HTMLInputElement>(null);
  const timer = useRef<number | undefined>(undefined);
  const composing = useRef(false);
  // 防抖定时器触发时要比较的是「此刻」地址里的搜索词，而不是排队时那一版。
  const latest = useRef({ value, onCommit });
  useEffect(() => {
    latest.current = { value, onCommit };
  });
  useEffect(() => () => window.clearTimeout(timer.current), []);
  useEffect(() => { window.clearTimeout(timer.current); }, [value, mode]);

  const commit = (next: string) => {
    window.clearTimeout(timer.current);
    const trimmed = next.trim();
    if (trimmed !== latest.current.value) latest.current.onCommit(trimmed);
  };
  const schedule = (next: string) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => commit(next), DEBOUNCE_MS);
  };

  useImperativeHandle(handle, () => ({
    flush: () => { if (!composing.current) commit(input.current?.value ?? text); },
    focus: () => input.current?.focus(),
  }));

  return (
    <form
      className={styles.search}
      data-find
      role="search"
      onSubmit={event => {
        event.preventDefault();
        if (!composing.current) commit(text);
      }}
    >
      <span className={styles.findMark} data-find-mark aria-hidden="true">
        FIND
      </span>
      <CreatorSelect label="搜索范围" compact value={mode}
        options={[{ value: 'author', label: '创作者资料' }, { value: 'work', label: '作品内容' }]}
        onChange={next => { window.clearTimeout(timer.current); onModeChange(next, text.trim()); }} />
      <label className="v7-sr-only" htmlFor="creator-search-input">
        搜索创作者或作品
      </label>
      <input
        id="creator-search-input"
        ref={input}
        type="search"
        value={text}
        maxLength={200}
        autoComplete="off"
        placeholder={mode === 'work' ? '标题、正文、图文识别或内容方向' : '昵称、平台账号、公开简介'}
        onChange={event => {
          setText(event.target.value);
          if (!composing.current) schedule(event.target.value);
        }}
        onCompositionStart={() => {
          composing.current = true;
          window.clearTimeout(timer.current);
        }}
        onCompositionEnd={event => {
          composing.current = false;
          schedule(event.currentTarget.value);
        }}
      />
      {text !== '' ? (
        <button type="button" className={styles.textButton} aria-label="清空搜索" onClick={() => { window.clearTimeout(timer.current); setText(''); onCommit(''); input.current?.focus(); }}>
          清空
        </button>
      ) : (
        <kbd className={styles.shortcut} aria-hidden="true">
          /
        </kbd>
      )}
    </form>
  );
}
