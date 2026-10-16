import { Link } from 'react-router';
import type { Target } from './api';
import { Action, Avatar } from './TargetControls';
import { listHref, returnFields, type Commands } from './useWorkspace';
function nextMoment(target: Target) {
  if (!target.monitoring) return '—';
  if (!target.next) return '尚未安排';
  if (!/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/.test(target.next)) return target.next;
  const remaining =
    Math.floor(Date.parse(`${target.next.replace(' ', 'T')}+08:00`) / 60000) - Math.floor(Date.now() / 60000);
  if (!Number.isFinite(remaining)) return target.next;
  if (remaining <= -60) return `已逾期约 ${Math.floor((-remaining + 30) / 60)} 小时`;
  if (remaining < 0) return `已逾期约 ${-remaining} 分钟`;
  if (remaining < 60) return `约 ${remaining} 分钟后`;
  if (remaining < 2880) return `约 ${Math.floor((remaining + 30) / 60)} 小时后`;
  return `约 ${Math.floor((remaining + 720) / 1440)} 天后`;
}
export function TargetTable({
  kind,
  rows,
  params,
  selected,
  setSelected,
  commands,
}: {
  kind: string;
  rows: Target[];
  params: URLSearchParams;
  selected: string[];
  setSelected: (v: string[]) => void;
  commands: Commands;
}) {
  const creator = kind === 'creator',
    title = creator ? '创作者档案' : '关键词观察',
    matching = rows.filter((r) => r.kind === kind),
    grid = creator ? 'c-tg-creator-grid' : 'c-tg-keyword-grid';
  if (!matching.length) return null;
  const toggle = (ref: string, checked: boolean) =>
    setSelected(checked ? [...new Set([...selected, ref])] : selected.filter((v) => v !== ref));
  return (
    <section className="c-tg-kind" aria-labelledby={`target-kind-${kind}`}>
      <div className="c-tg-kind-head">
        <h2 id={`target-kind-${kind}`}>{title}</h2>
        <span>{matching.length} 项</span>
      </div>
      <div className="c-tg-table-scroll" role="table" aria-label={title}>
        <div className={`c-tg-table-head ${grid}`} role="row">
          <span role="columnheader">
            <label className="c-tg-select-all">
              <input
                type="checkbox"
                data-target-select-all
                aria-label={`选择全部${creator ? '创作者' : '关键词'}目标`}
                checked={matching.every((r) => selected.includes(r.targetRef))}
                onChange={(e) =>
                  setSelected(
                    e.target.checked
                      ? [...new Set([...selected, ...matching.map((r) => r.targetRef)])]
                      : selected.filter((v) => !matching.some((r) => r.targetRef === v)),
                  )
                }
              />
            </label>
          </span>
          {[
            '编号',
            creator ? '创作者' : '关键词',
            '平台',
            '分组',
            '档案状态',
            creator ? '作品目录' : '命中作品',
            '详情进度',
            '巡查状态',
            '最近新增',
            '上次巡查',
            '下次巡查',
            '操作',
          ].map((h, i) => (
            <span
              key={h}
              role="columnheader"
              className={i === 5 ? 'c-tg-head-num' : i === 6 ? 'c-tg-head-num c-tg-head-end' : undefined}
            >
              {h}
            </span>
          ))}
        </div>
        <div className="c-tg-list" role="rowgroup">
          {matching.map((target) => (
            <article className={`c-tg-item ${grid}`} role="row" key={target.targetRef}>
              <label className="c-tg-select" role="cell">
                <input
                  type="checkbox"
                  aria-label={`选择${target.name}`}
                  checked={selected.includes(target.targetRef)}
                  onChange={(e) => toggle(target.targetRef, e.target.checked)}
                />
              </label>
              <div className="c-tg-number" role="cell">
                <span className="c-tg-index">{String(rows.indexOf(target) + 1).padStart(3, '0')}</span>
              </div>
              <div className="c-tg-object" role="cell">
                {creator && <Avatar target={target} />}
                <div className="c-tg-object-text">
                  <Link
                    id={`target-${target.targetRef}`}
                    className="c-tg-title c-tg-object-link"
                    to={listHref(params, { drawer: target.targetRef })}
                  >
                    {target.name}
                  </Link>
                  {creator && (
                    <span className="c-tg-meta" title={target.handle}>
                      {target.handle}
                    </span>
                  )}
                </div>
              </div>
              <div className="c-tg-cell c-tg-platform" role="cell">
                {target.platform === 'xhs'
                  ? '小红书'
                  : target.platform === 'douyin'
                    ? '抖音'
                    : target.platform}
              </div>
              <div className="c-tg-cell c-tg-group" role="cell" title={target.group}>
                {target.group}
              </div>
              <div className="c-tg-cell" role="cell">
                <span className={`c-tg-truth c-tg-${target.archiveTone}`}>{target.archiveLabel}</span>
              </div>
              <div className="c-tg-cell c-tg-number-value" role="cell">
                {target.works}
              </div>
              <div className="c-tg-cell c-tg-number-value c-tg-cell-end" role="cell">
                {target.details}
              </div>
              <div className="c-tg-cell" role="cell">
                <span className={`c-tg-truth c-tg-${target.patrolTone}`}>{target.patrolLabel}</span>
              </div>
              <div className="c-tg-cell c-tg-change" role="cell">
                {target.recentNew === null
                  ? '尚无成功巡查'
                  : creator
                    ? target.recentNew === 0
                      ? '无新增'
                      : `+${target.recentNew} 新发现`
                    : `+${target.recentNew}`}
              </div>
              <time className="c-tg-cell c-tg-time" role="cell">
                {target.last?.replace(/^\d{4}-/, '') ?? '尚未巡查'}
              </time>
              <time className="c-tg-cell c-tg-time" role="cell">
                {nextMoment(target)}
              </time>
              <div className="c-tg-actions" role="cell">
                <Action target={target} params={params} commands={commands} />
                <form
                  className="c-tg-toggle"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void commands.run('patrol-toggle', {
                      ...returnFields(params),
                      row_target_ref: target.targetRef,
                      enable: !target.monitoring,
                    });
                  }}
                >
                  <button
                    className="c-tg-switch"
                    disabled={commands.busy}
                    aria-pressed={target.monitoring}
                    aria-label={target.monitoring ? '正在观察，点击停止观察' : '已停止观察，点击恢复观察'}
                    title={target.monitoring ? '正在观察，点击停止观察' : '已停止观察，点击恢复观察'}
                  />
                </form>
                <Link
                  className="c-tg-act c-tg-act-danger"
                  to={listHref(params, { delete: target.targetRef })}
                >
                  删除
                </Link>
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
