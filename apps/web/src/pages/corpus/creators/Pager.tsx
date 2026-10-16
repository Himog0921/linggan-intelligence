import { useState, type FormEvent } from 'react';
import { PAGE_SIZES } from './creatorsQuery';
import styles from './CreatorsPage.module.css';

interface PagerProps {
  /** 从 0 开始。 */
  page: number;
  pageSize: number;
  /** 当前条件命中的作者总数；还没读到时为 null，此时翻页控件全部不可用。 */
  total: number | null;
  onPage: (page: number) => void;
  onPageSize: (pageSize: number) => void;
}

/** 表格下方唯一的一套完整分页：当前页 / 总页数、上一页、下一页、指定页跳转、每页数量。 */
export function Pager({ page, pageSize, total, onPage, onPageSize }: PagerProps) {
  const pages = total === null ? 0 : Math.ceil(total / pageSize);
  const [jump, setJump] = useState('');
  const [seen, setSeen] = useState(page);
  const [error, setError] = useState<string | null>(null);
  // 页码变了（翻页、后退）：跳转框跟着变，旧的越界提示作废。
  if (seen !== page) {
    setSeen(page);
    setJump('');
    setError(null);
  }
  const shown = jump !== '' ? jump : pages > 0 ? String(page + 1) : '';

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const target = Number(shown);
    if (!Number.isSafeInteger(target) || target < 1 || target > pages) {
      setError(`请输入 1 至 ${pages} 页。`);
      return;
    }
    setError(null);
    onPage(target - 1);
  }

  return (
    <nav className={styles.pagination} aria-label="作者表格分页">
      <span className={styles.pageSummary} aria-live="polite">
        {total === null ? '' : pages > 0 ? `${page + 1} / ${pages} 页` : '0 页'}
      </span>
      <button type="button" disabled={pages === 0 || page === 0} onClick={() => onPage(page - 1)}>
        上一页
      </button>
      <button type="button" disabled={pages === 0 || page + 1 >= pages} onClick={() => onPage(page + 1)}>
        下一页
      </button>
      <form className={styles.jump} noValidate onSubmit={submit}>
        <label>
          第{' '}
          <input
            type="number"
            min={1}
            max={pages || undefined}
            step={1}
            inputMode="numeric"
            aria-label="跳转页码"
            aria-invalid={error ? true : undefined}
            disabled={pages === 0}
            value={shown}
            onChange={event => setJump(event.target.value)}
          />{' '}
          页
        </label>
        <button type="submit" disabled={pages === 0}>
          跳转
        </button>
        <span className={styles.jumpError} role="status">
          {error}
        </span>
      </form>
      <label>
        每页{' '}
        <select value={pageSize} onChange={event => onPageSize(Number(event.target.value))}>
          {PAGE_SIZES.map(size => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
      </label>
    </nav>
  );
}
