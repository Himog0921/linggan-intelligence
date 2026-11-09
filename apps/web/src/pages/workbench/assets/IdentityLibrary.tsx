import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import * as api from './assetsApi';
import { identitySummary } from './assetsText';
import { IdentityForm } from './IdentityForm';
import styles from './Assets.module.css';

/** Every Xiaohongshu account that can be written for, with what is known about it. */
export function IdentityLibrary({ domainRef }: { domainRef: string }) {
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<string | null>(null);
  const accounts = useQuery({ queryKey: ['creation-assets', 'accounts', domainRef], queryFn: ({ signal }) => api.listAccounts(domainRef, signal), gcTime: 0 });
  const items = accounts.data ?? [];
  const current = items.find(item => item.accountRef === selected) ?? null;
  const reload = () => void queryClient.invalidateQueries({ queryKey: ['creation-assets'] });
  return <section aria-label="账号身份">
    <header className="as-head">
      <div className="grow">
        <h1 className="h-obj">账号身份</h1>
        <p>衔接句只会出现在“关系已确认且简介已写明”的账号上。没有填写的账号不会被当成“无关系”。</p>
      </div>
      <span className="asset-count">{accounts.isSuccess ? `${items.length} 个小红书账号` : ''}</span>
    </header>
    {accounts.isPending ? <p role="status">正在读取账号…</p> : null}
    {accounts.isError ? <p role="alert" className={styles.problem}>账号读取失败，可以<ActionButton variant="quiet" onClick={() => void accounts.refetch()}>重试</ActionButton></p> : null}
    {accounts.isSuccess && items.length === 0 ? <p className="lib-empty">这个领域下没有可写稿的小红书账号（在用或养号中）。</p> : null}
    {items.length ? <div className="lib">
      <div className="lib-table">
        <table className="ledger">
          <thead><tr><th scope="col">账号</th><th scope="col">已知</th></tr></thead>
          <tbody>{items.map(item => {
            const on = item.accountRef === selected;
            return <tr key={item.accountRef} className={on ? 'lib-row on' : 'lib-row'}>
              <td><button type="button" aria-current={on ? 'true' : undefined} onClick={() => setSelected(item.accountRef)}><b>{item.accountName}</b></button></td>
              <td>{identitySummary(item.identity)}{item.operatorName ? ` · 负责：${item.operatorName}` : ''}</td>
            </tr>;
          })}</tbody>
        </table>
      </div>
      <aside className="lib-insp" aria-label="账号身份详情">
        {current ? <IdentityForm key={`${current.accountRef}:${current.identity?.revision ?? 0}`} accountRef={current.accountRef}
          accountName={current.accountName} identity={current.identity} onSaved={reload} onStale={reload} />
          : <p className={styles.hint}>在左边选一个账号，查看或填写它的身份。</p>}
      </aside>
    </div> : null}
  </section>;
}
