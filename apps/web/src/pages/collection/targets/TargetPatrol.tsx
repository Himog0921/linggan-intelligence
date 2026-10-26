import { useNavigate } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import type { Drawer } from './api';
import { listHref, returnFields, type Commands } from './useWorkspace';
import { Section } from './TargetSection';
import { rankings } from './targetLabels';
export function Patrol({
  data,
  params,
  commands,
}: {
  data: Drawer;
  params: URLSearchParams;
  commands: Commands;
}) {
  const navigate=useNavigate();
  const rules = data.rules,
    t = data.target;
  return (
    <>
      <Section
        title="巡查规则"
        note={rules === null ? '读取暂不可用' : rules.length ? `${rules.length} 条` : '未设置'}
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
                            <ActionButton variant="quiet" onClick={()=>navigate(listHref(params,{rule:t.targetRef,rule_slot:r.slot}))}>编辑</ActionButton>
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
                              <ActionButton type="submit" variant="quiet" disabled={commands.busy}>
                                {r.enabled ? '暂停' : '启用'}
                              </ActionButton>
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
                                <ActionButton type="submit" variant="quiet" disabled={commands.busy}>
                                  停用
                                </ActionButton>
                              </form>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>

              </>
            ) : (
              <p className="c-dw-note">
                尚未设置巡查规则
              </p>
            )}
            {t.kind === 'keyword' && (
              <ActionButton onClick={()=>navigate(listHref(params,{rule:t.targetRef,rule_slot:'new'}))}>添加规则</ActionButton>
            )}
          </>
        )}
      </Section>
    </>
  );
}
