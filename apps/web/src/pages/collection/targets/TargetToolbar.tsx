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
      <div className="c-actions c-tg-toolbar">
        <Link
          className="c-tg-ctl c-tg-sort"
          to={listHref(params, { sort: 'last' })}
          aria-label="按最近观察排序"
        >
          <span>最近观察</span>
          <i className="c-tg-caret" aria-hidden="true" />
        </Link>
        <form
          id="collection-target-create"
          className="c-target-add"
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
          <label className="c-tg-field c-tg-field-query">
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
          <button
            className="c-btn-secondary c-tg-batch-open"
            type="button"
            disabled={!selected.length || commands.busy}
            onClick={openBatch}
          >
            <span>批量编辑</span>
            {selected.length > 0 && <strong>{selected.length}</strong>}
          </button>
          <button
            className="c-btn-primary"
            type="button"
            disabled={commands.busy}
            onClick={(e) => {
              const form = e.currentTarget.form;
              if (form?.reportValidity()) setOpen(true);
            }}
          >
            ＋ 新建目标
          </button>
          {open && (
            <Modal commands={commands} sectionId="target-domain-modal" initialFocus='select[name="domain"]' id="target-domain-title" className="c-tg-batch-dialog" onClose={() => setOpen(false)}>
              <div>
                <h2 id="target-domain-title">这个目标参与哪个领域</h2>
                <p>
                  同一 Target 可以关联多个领域；每个领域分别设置 primary 或
                  reference。材料统一进入共享材料链。
                </p>
              </div>
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
              <p>
                需要新建领域或调整角色？<Link to="/collection/domains">打开领域管理</Link>。
              </p>
              <div className="c-tg-batch-dialog-actions">
                <button className="c-btn-secondary" type="button" onClick={() => setOpen(false)}>
                  取消
                </button>
                <button className="c-btn-primary" disabled={commands.busy}>
                  建立目标
                </button>
              </div>
            </Modal>
          )}
        </form>
      </div>
    </div>
  );
}
