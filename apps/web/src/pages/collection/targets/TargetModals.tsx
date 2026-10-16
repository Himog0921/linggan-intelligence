import { createPortal } from 'react-dom';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Link } from 'react-router';
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
      className={rule ? 'c-rule-overlay' : 'c-tg-batch-overlay'}
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
            <h3 id="c-rule-cadence-title">自动观察</h3>
            <span>Asia/Shanghai · 从启用时刻开始计时</span>
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
              <b>按规则自动观察</b>
              <small>关闭后不再产生未来定时工单；人工观察仍可申请。</small>
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
                <b>巡查新增后补采详情</b>
                <small>
                  勾选并保存后自动跟进已接纳、尚无详情的巡查新作品；每篇一次详情、媒体及最多 30
                  条评论。关闭后不再派新单。
                </small>
              </span>
            </label>
          )}
          <div className="c-rule-grid">
            <label htmlFor="fixed_interval_seconds">
              <span>观察间隔（仅可选择下列 5 档）</span>
              <select
                id="fixed_interval_seconds"
                name="fixed_interval_seconds"
                value={form.fixed_interval_seconds}
                onChange={(e) => update('fixed_interval_seconds', e.target.value)}
                disabled={disabled}
              >
                {!INTERVALS.some(([v]) => v === form.fixed_interval_seconds) && (
                  <option value={form.fixed_interval_seconds}>
                    保持当前 · {form.fixed_interval_seconds} 秒
                  </option>
                )}
                {INTERVALS.map(([v, label]) => (
                  <option key={v} value={v}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <p className="c-rule-hint">
            {form.surface_key === 'keyword_search'
              ? '每次运行按下面的采样口径搜一轮。关键词观察不做作者资格核验——那是创作者档案的事。'
              : '每次运行固定执行：作者主页资格核验 + 最近 30 条作品观察。历史建档完整度不会阻断后续观察。'}
          </p>
        </section>
        {form.surface_key === 'keyword_search' && (
          <section className="c-rule-section" aria-labelledby="c-rule-sampling-title">
            <div className="c-rule-section-head">
              <h3 id="c-rule-sampling-title">采样口径</h3>
              <span>决定每一轮怎么取样，随样本一起留痕</span>
            </div>
            <div className="c-rule-grid">
              <label>
                <span>排序依据</span>
                {newRule ? (
                  <select
                    name="ranking_key"
                    value={form.ranking_key}
                    onChange={(e) => update('ranking_key', e.target.value)}
                    disabled={disabled}
                  >
                    {RANKINGS.filter(([v]) => panel.rankings.includes(v) || v === form.ranking_key).map(
                      ([v, label]) => (
                        <option key={v} value={v}>
                          {label}
                        </option>
                      ),
                    )}
                  </select>
                ) : (
                  <output>{RANKINGS.find(([v]) => v === form.ranking_key)?.[1] ?? form.ranking_key}</output>
                )}
              </label>
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
              <label htmlFor="published_within_days">
                <span>只要多新的内容</span>
                <select
                  id="published_within_days"
                  value={form.published_within_days}
                  onChange={(e) => update('published_within_days', e.target.value)}
                  disabled={disabled}
                >
                  {!WINDOWS.some(([v]) => v === form.published_within_days) && (
                    <option value={form.published_within_days}>
                      保持当前 · {form.published_within_days} 天（非平台档位）
                    </option>
                  )}
                  {WINDOWS.map(([v, label]) => (
                    <option key={v} value={v}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <p className="c-rule-hint">
              {newRule
                ? panel.rankings.length
                  ? '排序就是这条规则的身份：同一个关键词盯几个榜就是几条规则，各有各的周期。这里只列出还没有规则的榜。'
                  : '这个关键词的五个榜都已经有规则了。要换口径，先停用其中一条。'
                : '排序是这条规则的身份，不在编辑里改——改它等于把这条规则搬到另一个榜上，它采回来的材料会说不清是按什么口径取的。要按别的榜采，回规则台加一条。'}
            </p>
            <p className="c-rule-hint">
              按下拉次数控制，不按条数控制——页面每次加载出多少条不由我们决定，只有「拉了几次」是能说准的事实。时间范围只有这四档，因为平台就只给这四档；选了之后，回执里记的是页面上
              <b>实际生效</b>
              的那一档，不是这里选的值。同一批样本的点赞数不可跨时间比较。综合排序掺入个性化推荐，采回来的是平台认为这个账号会喜欢的内容，不是这个领域客观最好的内容。
            </p>
          </section>
        )}
      </div>
      <footer className="c-rule-actions">
        <div className="c-rule-secondary-actions">
          <Link className="c-btn-quiet" to={`/collection/tasks?once=1&target=${panel.targetRef}`}>
            立即采一次
          </Link>
          {!newRule && (
            <button
              className="c-btn-quiet"
              name="command_kind"
              value={panel.form.automatic_enabled ? 'pause' : 'resume'}
              disabled={disabled || commands.busy}
            >
              {panel.form.automatic_enabled ? '暂停未来自动调度' : '恢复自动调度'}
            </button>
          )}
        </div>
        <button
          className="c-btn-primary"
          name="command_kind"
          value="save_rule"
          disabled={disabled || commands.busy}
        >
          保存新版本
        </button>
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
    return <Modal key={`unavailable:${errorCode}`} commands={commands} rule id="c-rule-title" className="c-rule-modal c-rule-modal-compact" closeHref={close}>
      <RuleUnavailable code={errorCode} close={<Link className="c-btn-quiet c-rule-close" data-modal-close to={close} aria-label="关闭监控规则">关闭</Link>} />
    </Modal>;
  }
  return (
    <Modal commands={commands} rule id="c-rule-title" className="c-rule-modal" closeHref={close}>
      <header className="c-rule-head">
        <div>
          <p className="c-rule-context">
            {data?.kind === 'keyword' ? '关键词' : '创作者'} ·{' '}
            {data?.form.expected_revision ? `REV ${data.form.expected_revision}` : '尚无规则'}
          </p>
          <h2 id="c-rule-title">监控规则 · {data?.name ?? '读取中'}</h2>
          <p>保存只写入一版规则，不代表采集已经开始。暂停只停止未来自动调度，人工观察仍可申请。</p>
        </div>
        <Link className="c-btn-quiet c-rule-close" data-modal-close to={close}>
          关闭
        </Link>
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
