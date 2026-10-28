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
  return <section className={styles.assetBody} aria-label="账号身份">
    <div>
      <div className={styles.toolbar}>
        <h2>账号身份</h2>
        <small>{accounts.isSuccess ? `${items.length} 个小红书账号` : ''}</small>
      </div>
      <p className={styles.hint}>衔接句只会出现在“关系已确认且简介已写明”的账号上。没有填写的账号不会被当成“无关系”。</p>
      {accounts.isPending ? <p role="status">正在读取账号…</p> : null}
      {accounts.isError ? <p role="alert" className={styles.problem}>账号读取失败，可以<ActionButton variant="quiet" onClick={() => void accounts.refetch()}>重试</ActionButton></p> : null}
      {accounts.isSuccess && items.length === 0 ? <p>这个领域下没有可写稿的小红书账号（在用或养号中）。</p> : null}
      <ul className={styles.list}>{items.map(item => <li key={item.accountRef}>
        <button type="button" aria-current={item.accountRef === selected ? 'true' : undefined} onClick={() => setSelected(item.accountRef)}>
          <b>{item.accountName}</b>
          <small>{identitySummary(item.identity)}{item.operatorName ? ` · 负责：${item.operatorName}` : ''}</small>
        </button>
      </li>)}</ul>
    </div>
    <div className={styles.detail}>
      {current ? <IdentityForm key={`${current.accountRef}:${current.identity?.revision ?? 0}`} accountRef={current.accountRef}
        accountName={current.accountName} identity={current.identity} onSaved={reload} onStale={reload} />
        : <p className={styles.hint}>在左边选一个账号，查看或填写它的身份。</p>}
    </div>
  </section>;
}
