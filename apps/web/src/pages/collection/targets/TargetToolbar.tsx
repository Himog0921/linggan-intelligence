import { ActionButton } from '../../../shared/ActionButton';
import styles from './TargetToolbar.module.css';
import { TargetSelect } from './TargetSelect';
import { useState } from 'react';
import { Link } from 'react-router';
import type { TargetList } from './api';
import { Modal } from './TargetModals';
import { listHref, pendingTargetFields, targetCommandContext, type Commands } from './useWorkspace';
export function Toolbar({
  list,
  params,
  selected,
  commands,
  openBatch,
}: {
  list: TargetList | null;
  params: URLSearchParams;
  selected: string[];
  commands: Commands;
  openBatch: () => void;
}) {
  const pending = pendingTargetFields('new', {}, targetCommandContext(params));
  const [closing, setClosing] = useState(false);
  function requestClose() { if (identity.trim() || kind !== 'creator' || domain !== null) setClosing(true); else setOpen(false); }
  const [kind, setKind] = useState(typeof pending?.target_kind === 'string' ? pending.target_kind : 'creator'),
    [identity, setIdentity] = useState(typeof pending?.identity === 'string' ? pending.identity : ''),
    [open, setOpen] = useState(pending !== null),
    [domain, setDomain] = useState<string | null>(typeof pending?.domain === 'string' ? pending.domain : null);
  const activeDomains = list?.domains.filter((d) => d.status === 'active') ?? [];
  const selectedDomain = domain ?? activeDomains.find((d) => d.domainRef === params.get('domain'))?.domainRef ?? activeDomains[0]?.domainRef ?? '';
  const options = [
    ['', '全部来源', list?.counts?.total],
    ['creator', '创作者', list?.counts?.creator],
    ['keyword', '关键词', list?.counts?.keyword],
    ['archiving', '建档中', list?.counts?.archiving],
    ['monitoring', '巡逻中', list?.counts?.monitoring],
  ];
  return (
    <div className="c-toolbar">
      <div className="c-tabs c-tg-views">
        {options.map(([key, label, count]) => (
          <Link
            key={key}
            className={(params.get('filter') ?? '') === key ? 'c-on' : undefined}
            to={listHref(params, { filter: typeof key === 'string' && key ? key : null, drawer: null })}
          >
            {label}
            {count === undefined || count === null ? '' : ` ${count}`}
          </Link>
        ))}
      </div>
      <div className={styles.actions}>
        <Link className={styles.sort} to={listHref(params, { sort: 'last' })} aria-label="按最近观察排序">最近观察 ↓</Link>
        <ActionButton disabled={!selected.length || commands.busy} onClick={openBatch}>批量编辑{selected.length > 0 && <strong>{selected.length}</strong>}</ActionButton>
        <ActionButton variant="primary" disabled={commands.busy} onClick={() => setOpen(true)}>＋ 新建目标</ActionButton>
      </div>
      {open && <Modal commands={commands} sectionId="target-domain-modal" initialFocus='input[name="identity"]' id="target-domain-title" className={`c-tg-batch-dialog ${styles.dialog}`} onClose={requestClose}>
        <header><h2 id="target-domain-title">新建观察目标</h2><p>选择观察对象与所属领域。</p></header>
        <form hidden={closing}
          id="collection-target-create"
          className={styles.form}
          onSubmit={(e) => {
            e.preventDefault();
            void commands.run('new', { target_kind: kind, identity, domain: selectedDomain }).then((ok) => {
              if (ok) {
                setOpen(false);
                setIdentity('');
              }
            });
          }}
        >
          <TargetSelect
            className="c-tg-field c-tg-field-kind"
            name="target_kind"
            label="目标类型"
            value={kind}
            onChange={setKind}
            options={[
              { value: 'creator', label: '创作者' },
              { value: 'keyword', label: '关键词' },
            ]}
          />
          <label className={styles.identity}>
            <span className="v7-sr-only">主页链接、ID 或关键词</span>
            <input
              name="identity"
              required
              maxLength={120}
              aria-label="创作者主页链接、ID 或关键词"
              placeholder="粘贴主页链接、ID 或输入关键词"
              value={identity}
              onChange={(e) => setIdentity(e.target.value)}
            />
          </label>
              <label>
                领域
                <TargetSelect
                  name="domain"
                  label="领域"
                  required
                  value={selectedDomain}
                  onChange={setDomain}
                  options={[
                    ...activeDomains.map((d) => ({ value: d.domainRef, label: d.name })),
                  ]}
                />
              </label>
              {activeDomains.length === 0 && <p role="status">当前没有可用领域。请先<Link to="/collection/domains">管理领域</Link>，再建立观察目标。</p>}
              <div className="c-tg-batch-dialog-actions">
                <ActionButton onClick={requestClose}>取消</ActionButton>
                <ActionButton variant="primary" type="submit" busy={commands.busy} disabled={!selectedDomain}>建立目标</ActionButton>
              </div>
        </form>
        {closing && <section aria-label="保留目标输入"><p>目标尚未建立。收起后保留当前输入，可返回继续编辑。</p><div className="c-tg-batch-dialog-actions"><ActionButton onClick={() => setClosing(false)}>继续编辑</ActionButton><ActionButton onClick={() => { setClosing(false); setOpen(false); }}>收起并保留输入</ActionButton></div></section>}
      </Modal>}
    </div>
  );
}
