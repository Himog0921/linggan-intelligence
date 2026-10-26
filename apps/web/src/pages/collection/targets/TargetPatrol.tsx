import { Link } from 'react-router';
import type { Drawer } from './api';
import { listHref, returnFields, type Commands } from './useWorkspace';
import { TargetMaintenance } from './TargetOverview';
import { Section } from './TargetSection';
import { patrolLabels, rankings } from './targetLabels';
export function Patrol({
  data,
  params,
  commands,
}: {
  data: Drawer;
  params: URLSearchParams;
  commands: Commands;
}) {
  const i = data.inspector,
    rules = data.rules,
    t = data.target;
  return (
    <>
      <details><summary>资料维护</summary><TargetMaintenance data={data} params={params} commands={commands}/></details>
      {i ? (
        <Section title="巡查变化" note="最近记录" className="c-dw-patrol">
          <ol className="c-dw-timeline">
            <li>
              <time>{i.next ?? (i.patrol === 'disabled' ? '未开启' : '待排定')}</time>
              <div>
                <b>下次巡查</b>
                <p>{patrolLabels[i.patrol] ?? i.patrol}</p>
              </div>
            </li>
            <li>
              <time>{i.last ?? '尚未取得成功结果'}</time>
              <div>
                <b>最近一次成功巡查</b>
                <p>
                  命中 {i.hits ?? '尚未取得'} · 本目标首次发现 {i.latestNew ?? '尚未取得'}
                </p>
              </div>
            </li>
          </ol>
          <p className="c-dw-note">
            历史作品与变化见「最近观察」。
          </p>
          <Link className="c-btn-secondary" to={listHref(params, { rule: t.targetRef })}>
            管理巡查
          </Link>
        </Section>
      ) : (
        <Section title="巡查变化" note="读取暂不可用">
          <div className="life-state">
            <b>当前读不到巡查状态</b>
            <p>未知不表示没有巡查结果，也不表示规则正常。</p>
          </div>
        </Section>
      )}
      <Section
        title="巡检规则"
        note={rules === null ? '读取暂不可用' : rules.length ? `${rules.length} 条` : '还没有'}
      >
        {rules === null ? (
          <p className="c-dw-note">规则暂不可读。</p>
        ) : (
          <>
            {rules.length ? (
              <>
                <div className="c-dw-catalog-table-wrap c-dw-rule-table-wrap">
                  <table className="c-dw-catalog-table c-dw-rule-table">
                    <caption className="v7-sr-only">这个目标的巡检规则，一行一条口径</caption>
                    <thead>
                      <tr>
                        {['口径', '状态', '周期', '取样', '下次巡查', '最近成功', '操作'].map((h) => (
                          <th scope="col" key={h}>
                            {h}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {rules.map((r) => (
                        <tr key={r.ruleRef}>
                          <th scope="row">
                            {r.slot === 'primary'
                              ? t.kind === 'creator'
                                ? '主页目录'
                                : '当前口径'
                              : (rankings[r.slot] ?? r.slot)}
                          </th>
                          <td>
                            <span className={`c-tg-truth c-tg-${r.enabled ? 'ok' : 'neutral'}`}>
                              {r.enabled ? '巡查中' : '已暂停'}
                            </span>
                          </td>
                          <td>
                            {r.interval === null ? '周期读不到' : `每 ${Math.trunc(r.interval / 3600)} 小时`}
                          </td>
                          <td>
                            {[
                              r.scroll === null ? '' : `下拉 ${r.scroll} 次`,
                              r.top === null ? '' : `取赞前 ${r.top}`,
                              r.days === null ? '' : `近 ${r.days} 天`,
                            ]
                              .filter(Boolean)
                              .join(' · ') || '未设取样上限'}
                          </td>
                          <td>{r.next ?? '未排定'}</td>
                          <td>{r.last ?? '尚未成功巡查'}</td>
                          <td className="c-dw-rule-actions">
                            <Link
                              className="c-btn-quiet"
                              to={listHref(params, { rule: t.targetRef, rule_slot: r.slot })}
                            >
                              编辑
                            </Link>
                            <form
                              onSubmit={(e) => {
                                e.preventDefault();
                                void commands.run('rules', {
                                  ...returnFields(params),
                                  target_ref: t.targetRef,
                                  rule_slot: r.slot,
                                  expected_revision: r.revision,
                                  idempotency_key: crypto.randomUUID(),
                                  command_kind: r.enabled ? 'pause' : 'resume',
                                  return_drawer: t.targetRef,
                                  return_dtab: 'patrol',
                                });
                              }}
                            >
                              <button className="c-btn-quiet" disabled={commands.busy}>
                                {r.enabled ? '暂停' : '启用'}
                              </button>
                            </form>
                            {rules.length > 1 && (
                              <form
                                onSubmit={(e) => {
                                  e.preventDefault();
                                  void commands.run('monitor-rules-retire', {
                                    ...returnFields(params),
                                    row_target_ref: t.targetRef,
                                    rule_ref: r.ruleRef,
                                    return_drawer: t.targetRef,
                                    return_dtab: 'patrol',
                                  });
                                }}
                              >
                                <button className="c-btn-quiet" disabled={commands.busy}>
                                  停用
                                </button>
                              </form>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <p className="c-dw-note">
                  暂停和停用只影响后续排期，历史材料保留。
                </p>
              </>
            ) : (
              <p className="c-dw-note">
                这个目标还没有巡检规则。建档是一次性的历史挖掘，巡检是此后每周看新增——两件事，需要各自的口径。
              </p>
            )}
            {t.kind === 'keyword' && (
              <Link
                className="c-btn-secondary"
                to={listHref(params, { rule: t.targetRef, rule_slot: 'new' })}
              >
                加一条规则
              </Link>
            )}
          </>
        )}
      </Section>
    </>
  );
}
