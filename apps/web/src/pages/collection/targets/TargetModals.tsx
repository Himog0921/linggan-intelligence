import { createPortal } from 'react-dom';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Link,useNavigate } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { SelectField } from '../../../shared/SelectField';
import ruleStyles from './TargetRuleForm.module.css';
import { decodeDeletion, decodeRulePanel, type RulePanel } from './api';
import { listHref, returnFields, useRead, pendingTargetFields, type Commands } from './useWorkspace';
import { TargetFeedback } from './TargetFeedback';
import { RuleErrors, RuleReceipt, RuleUnavailable } from './RuleFeedback';

export function Modal({
  id,
  className,
  children,
  onClose,
  closeHref,
  rule = false,
  commands,
  sectionId,
  initialFocus,
}: {
  id: string;
  className: string;
  children: ReactNode;
  onClose?: () => void;
  closeHref?: string;
  rule?: boolean;
  commands?: Commands;
  sectionId?: string;
  initialFocus?: string | null;
}) {
  const ref = useRef<HTMLElement>(null);
  const closeCallback = useRef(onClose);
  useEffect(() => { closeCallback.current = onClose; }, [onClose]);
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = ref.current;
    const first = initialFocus === null ? null : dialog?.querySelector<HTMLElement>(initialFocus ?? 'h2');
    if (first) {
      if (first.tagName === 'H2') first.tabIndex = -1;
      first.focus();
    }
    // The original URL-based deletion preview did not focus its heading. Keep that
    // presentation while moving the first keyboard interaction into the dialog.
    const enterDialog = (event: KeyboardEvent) => {
      if (initialFocus !== null || document.activeElement !== previous) return;
      if (event.key === 'Tab') {
        const controls = [...(dialog?.querySelectorAll<HTMLElement>(
          'a[href],button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled)',
        ) ?? [])].filter(control => control.offsetParent !== null);
        const control = event.shiftKey ? controls.at(-1) : controls[0];
        if (control) { event.preventDefault(); control.focus(); }
      } else if (event.key === 'Escape') {
        event.preventDefault();
        if (closeCallback.current) closeCallback.current();
        else dialog?.querySelector<HTMLAnchorElement>('[data-modal-close]')?.click();
      }
    };
    document.addEventListener('keydown', enterDialog, true);
    return () => {
      document.removeEventListener('keydown', enterDialog, true);
      previous?.focus();
    };
  }, [initialFocus]);
  const content = (
    <div
      className={rule ? `c-rule-overlay ${ruleStyles.overlay}` : 'c-tg-batch-overlay'}
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          if (onClose) onClose();
          else ref.current?.querySelector<HTMLAnchorElement>('[data-modal-close]')?.click();
        }
      }}
    >
      <section
        id={sectionId}
        className={className}
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-labelledby={id}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.stopPropagation();
            if (onClose) onClose();
            else ref.current?.querySelector<HTMLAnchorElement>('[data-modal-close]')?.click();
          }
          if (e.key === 'Tab') {
            const all = [
                ...(ref.current?.querySelectorAll<HTMLElement>(
                  'a[href],button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled)',
                ) ?? []),
              ].filter((v) => v.offsetParent !== null),
              first = all[0],
              last = all[all.length - 1];
            if (!first) {
              e.preventDefault();
              return;
            }
            if (
              e.shiftKey &&
              (document.activeElement === first || document.activeElement?.tagName === 'H2')
            ) {
              e.preventDefault();
              last?.focus();
            } else if (!e.shiftKey && document.activeElement === last) {
              e.preventDefault();
              first.focus();
            }
          }
        }}
      >
        {commands?.feedback && !commands.feedback.unknown && !commands.feedback.applied && <TargetFeedback commands={commands} />}
        {children}
        {closeHref && (
          <Link className="v7-sr-only" data-modal-close to={closeHref}>
            关闭弹窗
          </Link>
        )}
      </section>
    </div>
  );
  return rule ? createPortal(content, document.body) : content;
}
export function DeleteModal({
  targetRef,
  params,
  commands,
  revision,
}: {
  targetRef: string;
  params: URLSearchParams;
  commands: Commands;
  revision: number;
}) {
  const { data, error } = useRead(
      `/api/collection/workspace/target/${encodeURIComponent(targetRef)}/deletion`,
      decodeDeletion,
      revision,
    ),
    [name, setName] = useState(() => { const value = pendingTargetFields('delete', { row_target_ref: targetRef })?.confirm_name; return typeof value === 'string' ? value : ''; }),
    close = listHref(params, { delete: null }, `target-${targetRef}`);
  return (
    <Modal commands={commands} id="target-delete-title" className="c-tg-batch-dialog c-tg-delete" closeHref={close} initialFocus={null}>
      <h2 id="target-delete-title">{data ? `彻底删除「${data.name}」` : '读取删除预览'}</h2>
      {!data ? (
        <p>{error ?? '正在核对会删除和保留的记录…'}</p>
      ) : (
        <>
          {(commands.feedback?.code === 'target_delete_name_mismatch' || params.get('error') === 'target_delete_name_mismatch') && <p className="c-tg-delete-blocked">名字对不上，没有删除任何东西。请原样输入上面那个名字。</p>}
          {data.blocked && (
            <p className="c-tg-delete-blocked">
              这个目标关联 {[
                data.blockingMaterialUsages > 0 ? `${data.blockingMaterialUsages} 条材料领域用途仍保留这个目标的准入血缘` : null,
                data.blockingWorkOrderUsages > 0 ? `${data.blockingWorkOrderUsages} 条工单领域用途是不可改写的执行快照` : null,
                data.blockingRiskSignals > 0 ? `${data.blockingRiskSignals} 条明确风险页信号仍属于安装安全记录` : null,
              ].filter(Boolean).join('、')}
              。删除目标不能抹掉不可变的领域用途或安装级安全事实；在有独立保留方案前，这里不提供删除。
            </p>
          )}
          <div className="c-tg-delete-cols">
            <div>
              <b>会删掉</b>
              <ul>
                <li>这个观察目标本身与它的 {data.rules} 个规则版本</li>
                <li>{data.requests} 次采集申请与准入决定</li>
                <li>
                  {data.workOrders} 张采集工单、{data.leases} 份租约、{data.tasks} 个执行任务
                </li>
                {data.retirements > 0 && <li>{data.retirements} 条只对这个目标目录生效的作品失效结论</li>}
              </ul>
            </div>
            <div>
              <b>不会删掉</b>
              <ul>
                <li>
                  {data.retainedWorks} 篇作品与 {data.retainedDetails} 份详情，以及它们的评论
                </li>
                <li>它们仍然属于这个博主：作者归属来自作品自己，不依赖观察目标是否存在</li>
                <li>采集包与回执——数据库层禁止删除已经发生的事实</li>
              </ul>
            </div>
          </div>
          {!data.blocked && (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void commands.run('delete', {
                  ...returnFields(params),
                  row_target_ref: targetRef,
                  confirm_name: name,
                });
              }}
            >
              <label>
                输入「{data.name}」以确认
                <input
                  name="confirm_name"
                  required
                  autoComplete="off"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </label>
              <button className="c-btn-primary c-tg-delete-go" disabled={commands.busy}>
                彻底删除
              </button>
            </form>
          )}
        </>
      )}
      <Link className="c-btn-quiet" data-modal-close to={close}>
        取消
      </Link>
    </Modal>
  );
}
const RANKINGS: [string, string][] = [
  ['most_liked', '最多点赞'],
  ['most_collected', '最多收藏'],
  ['most_commented', '最多评论'],
  ['latest', '最新'],
  ['comprehensive', '综合排序'],
];
const INTERVALS: [string, string][] = [
  ['21600', '6 小时'],
  ['43200', '12 小时'],
  ['86400', '24 小时'],
  ['172800', '2 天'],
  ['604800', '7 天'],
];
const WINDOWS: [string, string][] = [
  ['', '不限'],
  ['1', '一天内'],
  ['7', '一周内'],
  ['180', '半年内'],
];
function RuleForm({
  panel,
  params,
  commands,
}: {
  panel: RulePanel;
  params: URLSearchParams;
  commands: Commands;
}) {
  const navigate=useNavigate();
  const [form, setForm] = useState(() => ({
    ...panel.form,
    ...(params.has('error') ? {
      automatic_enabled: params.has('rule_automatic_enabled') ? ['1', 'true', 'on'].includes(params.get('rule_automatic_enabled') ?? '') : panel.form.automatic_enabled,
      creator_follow_details: params.has('rule_creator_follow_details') ? ['1', 'true', 'on'].includes(params.get('rule_creator_follow_details') ?? '') : panel.form.creator_follow_details,
      fixed_interval_seconds: params.get('rule_fixed_interval_seconds') ?? panel.form.fixed_interval_seconds,
      surface_key: params.get('rule_surface_key') ?? panel.form.surface_key,
      ranking_key: params.get('rule_ranking_key') ?? panel.form.ranking_key,
      task_contract_version: params.get('rule_task_contract_version') ?? panel.form.task_contract_version,
    } : {}),
    ...Object.fromEntries(Object.entries(pendingTargetFields('rules', { target_ref: panel.targetRef, rule_slot: panel.slot ?? 'new' }) ?? {}).filter(([key]) => key in panel.form)),
  })),
    disabled = panel.lifecycle === 'dismissed',
    newRule = form.expected_revision === 0;
  const update = (key: keyof typeof form, value: string | boolean) =>
    setForm((current) => ({ ...current, [key]: value }));
  const fields = {
    ...form,
    ...returnFields(params),
    target_ref: panel.targetRef,
    rule_slot: panel.slot ?? 'new',
    return_drawer: params.get('drawer') ?? '',
    return_dtab: params.get('dtab') ?? '',
  };
  return (
    <form
      className="c-rule-form"
      onSubmit={(e) => {
        e.preventDefault();
        const submitter = e.nativeEvent instanceof SubmitEvent ? e.nativeEvent.submitter : null;
        const command_kind = submitter instanceof HTMLButtonElement ? submitter.value : 'save_rule';
        void commands.run('rules', { ...fields, command_kind: command_kind || 'save_rule' });
      }}
    >
      <div className="c-rule-body">
        <RuleErrors code={commands.feedback?.action === 'rules' && !commands.feedback.unknown && !commands.feedback.applied ? commands.feedback.code : params.get('error')} />
        <section className="c-rule-section" aria-labelledby="c-rule-cadence-title">
          <div className="c-rule-section-head">
            <h3 id="c-rule-cadence-title">自动巡查</h3>
            <span>UTC+08</span>
          </div>
          <label className="c-rule-switch">
            <input
              type="checkbox"
              name="automatic_enabled"
              checked={form.automatic_enabled}
              onChange={(e) => update('automatic_enabled', e.target.checked)}
              disabled={disabled}
            />
            <span>
              <b>定时巡查</b>
            </span>
          </label>
          {panel.kind === 'creator' && (
            <label className="c-rule-switch">
              <input
                type="checkbox"
                name="creator_follow_details"
                checked={form.creator_follow_details}
                onChange={(e) => update('creator_follow_details', e.target.checked)}
                disabled={disabled}
              />
              <span>
                <b>补采新作品详情</b>
                <small>
                  每篇含详情、媒体和最多 30 条评论。
                </small>
              </span>
            </label>
          )}
          <div className="c-rule-grid">
            <div className={ruleStyles.field}>
              <span>观察间隔</span>
              <SelectField label="观察间隔" value={form.fixed_interval_seconds} onChange={value=>update('fixed_interval_seconds',value)} disabled={disabled}
                options={[...(!INTERVALS.some(([value])=>value===form.fixed_interval_seconds)?[{value:form.fixed_interval_seconds,label:`当前间隔 · ${form.fixed_interval_seconds} 秒`}]:[]),...INTERVALS.map(([value,label])=>({value,label}))]}/>
            </div>
          </div>
          <p className="c-rule-hint">
            {form.surface_key === 'keyword_search'
              ? '按以下条件搜索一轮。'
              : '每轮检查主页与最近 30 篇作品。'}
          </p>
        </section>
        {form.surface_key === 'keyword_search' && (
          <section className="c-rule-section" aria-labelledby="c-rule-sampling-title">
            <div className="c-rule-section-head">
              <h3 id="c-rule-sampling-title">采样口径</h3>

            </div>
            <div className="c-rule-grid">
              <div className={ruleStyles.field}>
                <span>排序依据</span>
                {newRule ? (
                  <SelectField label="排序依据" value={form.ranking_key} onChange={value=>update('ranking_key',value)} disabled={disabled}
                    options={RANKINGS.filter(([value])=>panel.rankings.includes(value)||value===form.ranking_key).map(([value,label])=>({value,label}))}/>
                ) : (
                  <output>{RANKINGS.find(([v]) => v === form.ranking_key)?.[1] ?? form.ranking_key}</output>
                )}
              </div>
              <label htmlFor="scroll_rounds">
                <span>下拉刷新次数</span>
                <input
                  id="scroll_rounds"
                  type="number"
                  min={0}
                  max={20}
                  value={form.scroll_rounds}
                  onChange={(e) => update('scroll_rounds', e.target.value)}
                  disabled={disabled}
                />
              </label>
              <label htmlFor="top_by_likes">
                <span>取点赞前几篇</span>
                <input
                  id="top_by_likes"
                  type="number"
                  min={1}
                  max={200}
                  value={form.top_by_likes}
                  onChange={(e) => update('top_by_likes', e.target.value)}
                  disabled={disabled}
                />
              </label>
              <div className={ruleStyles.field}>
                <span>发布时间</span>
                <SelectField label="发布时间" value={form.published_within_days} onChange={value=>update('published_within_days',value)} disabled={disabled}
                  options={[...(!WINDOWS.some(([value])=>value===form.published_within_days)?[{value:form.published_within_days,label:`当前范围 · ${form.published_within_days} 天`}]:[]),...WINDOWS.map(([value,label])=>({value,label}))]}/>
              </div>
            </div>
            <p className="c-rule-hint">
              {newRule
                ? panel.rankings.length
                  ? '可添加尚未设置的排序规则。'
                  : '所有排序均已设置；停用一条后可重新添加。'
                : '修改排序需另建规则。'}
            </p>
            <details className={ruleStyles.scope}><summary>采样范围</summary><p>下拉次数控制取样范围，发布时间以平台实际生效值为准。综合排序包含个性化推荐。</p></details>
          </section>
        )}
      </div>
      <footer className="c-rule-actions">
        <div className="c-rule-secondary-actions">
          <ActionButton disabled={disabled} onClick={()=>navigate(`/collection/tasks?once=1&target=${panel.targetRef}&domain=${encodeURIComponent(params.get('domain')??'all')}&returnTo=${encodeURIComponent(listHref(params))}`)}>立即采一次</ActionButton>
          {!newRule&&<ActionButton type="submit" name="command_kind" value={panel.form.automatic_enabled?'pause':'resume'} disabled={disabled||commands.busy}>
            {panel.form.automatic_enabled?'暂停自动巡查':'恢复自动巡查'}
          </ActionButton>}
        </div>
        <ActionButton variant="primary" type="submit" name="command_kind" value="save_rule" disabled={disabled||commands.busy}>保存新版本</ActionButton>
      </footer>
    </form>
  );
}
export function RuleModal({
  targetRef,
  params,
  commands,
  revision,
}: {
  targetRef: string;
  params: URLSearchParams;
  commands: Commands;
  revision: number;
}) {
  const navigate=useNavigate();
  const { data, error, errorCode } = useRead(
      `/api/collection/workspace/target/${encodeURIComponent(targetRef)}/rules?${params}`,
      decodeRulePanel,
      revision,
    ),
    close = listHref(params, { rule: null, rule_slot: null, rule_receipt: null });
  const returned = commands.feedback?.action === 'rules' ? commands.feedback.location : null;
  const receiptRef = returned ? new URL(returned, window.location.origin).searchParams.get('rule_receipt') : null;
  const receiptQuery = new URLSearchParams(params);
  if (receiptRef) receiptQuery.set('rule_receipt', receiptRef);
  const latest = useRead(receiptRef ? `/api/collection/workspace/target/${encodeURIComponent(targetRef)}/rules?${receiptQuery}` : null, decodeRulePanel);

  if (error) {
    return <Modal key={`unavailable:${errorCode}`} commands={commands} rule id="c-rule-title" className={`c-rule-modal c-rule-modal-compact ${ruleStyles.modal}`} closeHref={close}>
      <RuleUnavailable code={errorCode} close={<ActionButton data-modal-close onClick={()=>navigate(close)} aria-label="关闭监控规则">关闭</ActionButton>} />
    </Modal>;
  }
  return (
    <Modal commands={commands} rule id="c-rule-title" className={`c-rule-modal ${ruleStyles.modal}`} closeHref={close}>
      <header className="c-rule-head">
        <div>
          <h2 id="c-rule-title">巡查设置 · {data?.name ?? '读取中'}</h2>
        </div>
        <ActionButton data-modal-close onClick={()=>navigate(close)} aria-label="关闭监控规则">关闭</ActionButton>
      </header>
      <RuleReceipt receipt={latest.data?.receipt ?? data?.receipt ?? null} />
      {data?.lifecycle === 'dismissed' && <p className="c-rule-disabled" data-control-reason="target_not_requestable"><b>目标已停止</b>历史与规则仍可查看，但不能保存、恢复或发起新的观察。</p>}
      {data ? (
        <RuleForm
          key={`${data.targetRef}:${data.slot}:${data.form.expected_revision}`}
          panel={data}
          params={params}
          commands={commands}
        />
      ) : (
        <div className="c-rule-body">
          <p>{error ?? '正在读取监控规则…'}</p>
        </div>
      )}
    </Modal>
  );
}
