import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import {
  readLegacyTaskDiagnostics,
  type LegacyTask,
  type LegacyTaskDiagnosticsData,
} from './legacyTaskDiagnosticsApi';
const short = (ref: string | null) => ref?.slice(0, 8) ?? '—';
const tabs = [
  ['overview', '概览'],
  ['attempt', 'Attempt'],
  ['package', 'Package'],
  ['receipt', 'Receipt'],
  ['frozen', '冻结资源'],
] as const;
function Frozen({ task, data }: { task: LegacyTask; data: LegacyTaskDiagnosticsData }) {
  const works = data.frozen?.filter((w) => w.taskId === task.taskId);
  return (
    <section className="c-control-surface c-frozen-works">
      <div className="c-control-section-head">
        <div>
          <p>冻结来源链</p>
          <h2>工单冻结资源</h2>
        </div>
        <span>当前任务 {works?.length ?? '未知'} 张</span>
      </div>
      <p className="c-control-boundary">
        冻结引用说明准入当时依赖什么；“当前不同”不会覆盖旧 Work、Attempt 或 Receipt。
      </p>
      <div className="c-control-list">
        {works === undefined ? (
          <p className="c-control-none">当前读不到冻结 Work 投影，未知不表示没有冻结资源。</p>
        ) : works.length === 0 ? (
          <p className="c-control-none">该任务没有关联的冻结 Work 投影。</p>
        ) : (
          works.map((w) => (
            <article className="c-frozen-row" key={w.workRef}>
              <div className="c-frozen-head">
                <div>
                  <p>
                    {w.lane} · {w.created}
                  </p>
                  <h3>{w.target}</h3>
                </div>
                <span>
                  {w.leaseLabel}
                  {w.taskState ? ` · ${w.taskState}` : ''}
                </span>
              </div>
              <dl className="c-frozen-grid">
                {w.resources.map((r) => (
                  <div key={r.label}>
                    <dt>{r.label}</dt>
                    <dd>{short(r.ref)}</dd>
                    <span>{r.state}</span>
                  </div>
                ))}
              </dl>
              <p className="c-frozen-links">
                Work {short(w.workRef)} · Lease {short(w.leaseRef)} · Task {short(w.taskId)}
              </p>
            </article>
          ))
        )}
      </div>
    </section>
  );
}
function Inspector({ task: t, data }: { task: LegacyTask; data: LegacyTaskDiagnosticsData }) {
  const [tab, setTab] = useState('overview');
  const failure = t.failure
    ? `此前启动失败：${t.failure} · ${t.failureAt ?? '时间未知'}；${t.receiptRef ? '后续回执已保留' : '当前尚未有 Receipt'}。`
    : '没有已记录的启动失败。';
  return (
    <aside className="c-task-inspector" aria-live="polite">
      <div className="c-task-inspector-head">
        <span>任务 #{short(t.taskId)}</span>
        <h2>{t.target}</h2>
        <p>
          {t.source} · {t.platform} · {t.pageType}
        </p>
      </div>
      <nav className="c-task-inspector-tabs" aria-label="任务详情" role="tablist">
        {tabs.map(([key, label], index) => (
          <button
            key={key}
            type="button"
            className={tab === key ? 'is-active' : undefined}
            id={`task-tab-${key}`}
            role="tab"
            aria-selected={tab === key}
            aria-controls={`task-panel-${key}`}
            tabIndex={tab === key ? 0 : -1}
            onClick={() => setTab(key)}
            onKeyDown={(e) => {
              if (['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(e.key)) {
                e.preventDefault();
                const next =
                  e.key === 'Home'
                    ? tabs[0]
                    : e.key === 'End'
                      ? tabs[tabs.length - 1]
                      : tabs[(index + (e.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length];
                if (next) {
                  setTab(next[0]);
                  document.getElementById(`task-tab-${next[0]}`)?.focus();
                }
              }
            }}
          >
            {label}
          </button>
        ))}
      </nav>
      <div className="c-task-inspector-body">
        <section
          className={`c-task-panel${tab === 'overview' ? ' is-active' : ''}`}
          id="task-panel-overview"
          role="tabpanel"
          aria-labelledby="task-tab-overview"
          hidden={tab !== 'overview'}
        >
          <div className={`c-state-line ${t.stateClass}`}>
            <span>
              <i />
              <b>{t.state}</b>
            </span>
            <span>{t.stateNote}</span>
          </div>
          <dl>
            <div>
              <dt>阶段</dt>
              <dd>{t.capabilities}</dd>
            </div>
            <div>
              <dt>冻结领域用途</dt>
              <dd>{t.domains}</dd>
            </div>
            <div>
              <dt>创建时间</dt>
              <dd>{t.created}</dd>
            </div>
            <div>
              <dt>历史</dt>
              <dd>{failure}</dd>
            </div>
          </dl>
        </section>
        <section
          className={`c-task-panel${tab === 'attempt' ? ' is-active' : ''}`}
          id="task-panel-attempt"
          role="tabpanel"
          aria-labelledby="task-tab-attempt"
          hidden={tab !== 'attempt'}
        >
          <h3>Attempt</h3>
          <p>
            {t.attemptId
              ? `Attempt ${short(t.attemptId)} · ${t.attemptStarted ?? '时间未知'}`
              : '尚未形成 Attempt'}
          </p>
        </section>
        <section
          className={`c-task-panel${tab === 'package' ? ' is-active' : ''}`}
          id="task-panel-package"
          role="tabpanel"
          aria-labelledby="task-tab-package"
          hidden={tab !== 'package'}
        >
          <h3>Package</h3>
          <p>
            {t.packageRef
              ? `${t.packageKind ?? '类型未知'} · Package ${short(t.packageRef)} · ${t.packageAccepted ?? '接纳时间未知'}`
              : '尚未形成 Package'}
          </p>
        </section>
        <section
          className={`c-task-panel${tab === 'receipt' ? ' is-active' : ''}`}
          id="task-panel-receipt"
          role="tabpanel"
          aria-labelledby="task-tab-receipt"
          hidden={tab !== 'receipt'}
        >
          <h3>Receipt</h3>
          <p>
            {t.receiptRef
              ? `Receipt ${short(t.receiptRef)} · ${t.admission} · ${t.receiptReceived ?? '接收时间未知'}`
              : '尚未收到 Receipt'}
          </p>
          <p>
            <span>{t.effect}</span>
            <code>{t.effectCode}</code>
          </p>
        </section>
        <section
          className={`c-task-panel${tab === 'frozen' ? ' is-active' : ''}`}
          id="task-panel-frozen"
          role="tabpanel"
          aria-labelledby="task-tab-frozen"
          hidden={tab !== 'frozen'}
        >
          <Frozen task={t} data={data} />
        </section>
      </div>
    </aside>
  );
}
function Delivery({ data }: { data: LegacyTaskDiagnosticsData }) {
  return (
    <section className="c-task-delivery">
      <h2>交付对账</h2>
      <p className="c-task-boundary">
        按最近进度读取最多 100
        个已消费导航的详情页会话，逐条核对冻结通道的包有没有拿到服务端回执。已有回执的会话不再算待交付；读不到冻结通道的会话只说「恢复待核实」和最后观察时间——包没到不等于没采到，等得久也不等于采不到。
      </p>
      <div className="c-task-delivery-list">
        {data.delivery === null ? (
          <p className="c-control-none">
            交付对账本次读取失败。这不表示没有待交付，也不表示本地包丢了；请恢复本机数据库读取后重试。
          </p>
        ) : data.delivery.length === 0 ? (
          <p className="c-control-none">
            最近 100 个已消费导航的详情页会话里没有可对账的行。这不表示历史上没有会话，也不触发任何平台访问。
          </p>
        ) : (
          data.delivery.map((s) => (
            <div className="c-task-delivery-row" key={s.sessionRef} data-delivery-conclusion={s.slug}>
              <span className="c-task-delivery-target">
                <b>{s.target}</b>
                <small>
                  {s.content} · {s.platform}
                </small>
              </span>
              <span className={`c-task-state ${s.className}`}>
                <b>{s.label}</b>
                <small>{s.note}</small>
              </span>
              <span className="c-task-delivery-lanes">
                {s.prepared === 0 ? '没有可核对的冻结通道' : `${s.delivered} / ${s.prepared} 通道已有回执`}
                {s.closedReason && (
                  <small>
                    {s.closedReason}
                    <code>{s.reasonCode}</code>
                  </small>
                )}
              </span>
              <time>
                {s.slug === 'closed' && s.closed ? `终结于 ${s.closed} · ` : ''}最后观察 {s.observed}
              </time>
            </div>
          ))
        )}
      </div>
    </section>
  );
}
export function LegacyTaskDiagnostics() {
  const read = useQuery({
      queryKey: ['collection-legacy-task-diagnostics'],
      queryFn: ({ signal }) => readLegacyTaskDiagnostics(signal),
      gcTime: 0,
    }),
    [selected, setSelected] = useState<string | null>(null),
    data = read.data;
  if (!data)
    return (
      <p className="c-control-none">
        {read.error
          ? '历史执行记录本次读取失败；这不表示没有任务、没有交付或没有冻结资源。'
          : '正在读取本机历史执行记录…'}
      </p>
    );
  const current = data.tasks.find((t) => t.taskId === selected) ?? data.tasks[0];
  return (
    <>
      {data.tasks.length === 0 ? (
        <section className="c-task-empty">
          <h2>当前读取范围没有采集任务</h2>
          <p>任务投影读取成功，最近 100 条范围返回零项；这不表示历史从未存在，也不会触发任何平台访问。</p>
          <p className="c-control-none">当前没有任务，因此没有可核对的冻结 Work 引用。</p>
        </section>
      ) : (
        <section className="c-tasks c-tasks-v4">
          <p className="c-task-boundary">
            按最近状态读取最多 100 条本机任务。队列资格、Attempt、Package 与 Receipt
            分列显示；读取本身不会领取、重试或新建任务。
          </p>
          <div className="c-task-workspace">
            <div className="c-task-ledger">
              <div className="c-task-ledger-head">
                {['任务', '目标 / 来源', '阶段', '状态', '回执', '尝试', '创建时间'].map((v) => (
                  <span key={v}>{v}</span>
                ))}
              </div>
              <div className="c-task-list">
                {data.tasks.map((t) => (
                  <button
                    type="button"
                    key={t.taskId}
                    className={`c-task-row${current?.taskId === t.taskId ? ' is-selected' : ''}`}
                    aria-pressed={current?.taskId === t.taskId}
                    onClick={() => setSelected(t.taskId)}
                  >
                    <span className="c-task-id">#{short(t.taskId)}</span>
                    <span className="c-task-target">
                      <b>{t.target}</b>
                      <small>{t.source}</small>
                      <small>{t.domains}</small>
                    </span>
                    <span className="c-task-stage">{t.capabilities}</span>
                    <span className={`c-task-state ${t.stateClass}`}>
                      <b>{t.state}</b>
                      <small>{t.stateNote}</small>
                    </span>
                    <span className="c-task-receipt">{t.receiptLabel}</span>
                    <span className="c-task-attempt">{short(t.attemptId)}</span>
                    <time>{t.created}</time>
                  </button>
                ))}
              </div>
            </div>
            {current && <Inspector task={current} data={data} />}
          </div>
        </section>
      )}
      <Delivery data={data} />
    </>
  );
}
