import { TargetFeedback } from './TargetFeedback';
import { useCallback, useState } from 'react';
import { useSearchParams } from 'react-router';
import { CollectionFrame } from '../CollectionFrame';
import { decodeList } from './api';
import { useCommands, useRead, pendingTargetFields, targetCommandContext } from './useWorkspace';
import { TargetDrawer } from './TargetDrawer';
import { RuleModal, DeleteModal, Modal } from './TargetModals';
import { TargetTable } from './TargetTable';
import { Toolbar } from './TargetToolbar';
import { AssignDomain } from './TargetAssignment';
export function TargetsPage() {
  const [params, setParams] = useSearchParams(),
    [revision, setRevision] = useState(0),
    [selected, setSelected] = useState<string[]>(() => { const fields = pendingTargetFields('batch', {}, targetCommandContext(params)); return Array.isArray(fields?.target_ref) ? fields.target_ref : []; }),
    [batch, setBatch] = useState(() => pendingTargetFields('batch', {}, targetCommandContext(params)) !== null),
    [group, setGroup] = useState(() => { const value = pendingTargetFields('batch', {}, targetCommandContext(params))?.group_name; return typeof value === 'string' ? value : ''; });
  const onApplied = useCallback(
    (location: string | null) => {
      setRevision((v) => v + 1);
      if (location?.startsWith('/collection/targets'))
        setParams(new URL(location, window.location.origin).searchParams);
    },
    [setParams],
  );
  const listQuery = new URLSearchParams([...params].filter(([key]) => ['domain', 'filter', 'sort'].includes(key)));
  const commands = useCommands(onApplied, targetCommandContext(params)),
    read = useRead(`/api/collection/workspace/targets?${listQuery}`, decodeList, revision),
    list = read.data;
  const assign = params.get('assign_domain'),
    drawer = params.get('drawer'),
    rule = params.get('rule'),
    deletion = params.get('delete');
  return (
    <CollectionFrame
      section="targets"
      toolbar={
        <Toolbar
          list={list}
          params={params}
          selected={selected}
          commands={commands}
          openBatch={() => setBatch(true)}
        />
      }
    >
      <section className="c-tg-workspace">
        <TargetFeedback commands={commands} code={params.get('error')} ahead={params.get('ahead')} />
        {read.error ? (
          <section className="c-empty">
            <h2>当前读不到观察目标</h2>
            <p>{read.error}</p>
          </section>
        ) : !list ? (
          <section className="c-empty" aria-busy="true">
            <h2>正在读取观察目标</h2>
          </section>
        ) : list.rows.length === 0 ? (
          <section className="c-empty c-empty-known-view">
            <div className="c-empty-rule" />
            <h2>当前列表范围没有匹配的观察目标</h2>
            <p>当前筛选下没有观察目标。可以切换上方分类查看其他目标；这不表示平台上没有可观察对象。</p>
            <div className="c-empty-foot" />
          </section>
        ) : (
          <div className="c-tg-directory">
            {['creator', 'keyword'].map((kind) => (
              <TargetTable
                key={kind}
                kind={kind}
                rows={list.rows}
                params={params}
                selected={selected}
                setSelected={setSelected}
                commands={commands}
              />
            ))}
          </div>
        )}
        <div className="c-tg-batch">
        {batch && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void commands
                .run('batch', { action: 'set_group', group_name: group, target_ref: selected })
                .then((ok) => {
                  if (ok) {
                    setBatch(false);
                    setSelected([]);
                  }
                });
            }}
          >
            <Modal commands={commands} initialFocus='input[name="group_name"]' id="target-batch-title" className="c-tg-batch-dialog" onClose={() => setBatch(false)}>
              <div>
                <h2 id="target-batch-title">批量编辑观察目标</h2>
                <p>已选择 {selected.length} 个目标</p>
              </div>
              <label>
                分组
                <input
                  name="group_name"
                  type="text"
                  maxLength={80}
                  placeholder="例如：ADHD 主力"
                  required
                  value={group}
                  onChange={(e) => setGroup(e.target.value)}
                />
              </label>
              <p>
                保存后会为所选目标设置同一分组。备注将在具备独立事实字段后接入，当前不会把自由文本冒充为已有备注。
              </p>
              <div className="c-tg-batch-dialog-actions">
                <button className="c-btn-secondary" type="button" onClick={() => setBatch(false)}>
                  取消
                </button>
                <button className="c-btn-primary" disabled={commands.busy}>
                  保存分组
                </button>
              </div>
            </Modal>
          </form>
        )}
        </div>
        {assign && list && (
          <AssignDomain key={assign} targetRef={assign} list={list} params={params} commands={commands} />
        )}
      </section>
      {drawer && <TargetDrawer targetRef={drawer} params={params} commands={commands} revision={revision} />}
      {rule && (
        <RuleModal
          key={`${rule}:${params.get('rule_slot') ?? ''}`}
          targetRef={rule}
          params={params}
          commands={commands}
          revision={revision}
        />
      )}
      {deletion && (
        <DeleteModal targetRef={deletion} params={params} commands={commands} revision={revision} />
      )}
    </CollectionFrame>
  );
}
