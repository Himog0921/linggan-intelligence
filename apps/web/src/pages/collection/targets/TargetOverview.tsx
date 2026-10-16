import { useState } from 'react';
import type { Drawer } from './api';
import { returnFields, pendingTargetFields, type Commands } from './useWorkspace';
import { Section, Readouts } from './TargetSection';
import { Action } from './TargetControls';
import { archiveLabels, executionLabels, patrolLabels } from './targetLabels';
export function Overview({
  data,
  params,
  commands,
}: {
  data: Drawer;
  params: URLSearchParams;
  commands: Commands;
}) {
  const i = data.inspector,
    creator = data.target.kind === 'creator',
    [retire, setRetire] = useState<string[] | null>(() => { const value = pendingTargetFields('retire-materials', { row_target_ref: data.target.targetRef })?.content_public_ref; return Array.isArray(value) ? value : null; }),
    selection = retire ?? data.blocked?.map((b) => b.publicRef) ?? [];
  if (!i)
    return <>
      <Section title="系统现在在做什么" note="读取暂不可用" className="c-dw-now">
        <div className="life-state"><b>目标状态暂时读不到</b><p>当前无法判断{creator ? '档案、执行与巡查' : '巡查与最近结果'}；这不表示没有任务、没有作品或运行正常。</p></div>
      </Section>
      <section className="c-dw-section c-dw-decision" id={creator ? undefined : 'required-action'}>
        <div className="c-dw-decision-copy"><span>是否需要处理</span><strong>{creator ? '当前无法判断' : data.decision.title}</strong><p>{creator ? '读取恢复前不提供新的写操作，避免用未知状态触发重复任务。' : data.decision.note}</p></div>
        {!creator && data.decision.action && <Action primary target={data.target} action={data.decision.action} params={params} commands={commands} />}
      </section>
    </>;
  const last = i.last ?? '尚未取得成功结果',
    next = i.next ?? (i.patrol === 'disabled' ? '未开启' : '待排定');
  const abnormal =
    i.quarantined === null || i.blocked === null
      ? '当前读不到'
      : i.quarantined + i.blocked === 0
        ? '无已知阻塞'
        : `${i.quarantined + i.blocked} 条待处理`;
  return (
    <>
      <Section
        title="系统现在在做什么"
        note="状态分别判断"
        className="c-dw-now"
        id={creator ? 'archive-task' : undefined}
      >
        <dl className="c-dw-state-list">
          {(creator
            ? [
                ['建档', archiveLabels[i.archive] ?? i.archive],
                ['执行', executionLabels[i.execution] ?? i.execution],
                ['巡查', patrolLabels[i.patrol] ?? i.patrol],
                ['异常', abnormal],
              ]
            : [
                ['巡查', patrolLabels[i.patrol] ?? i.patrol],
                ['最近结果', last],
              ]
          ).map(([label, value]) => (
            <div key={label}>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      </Section>
      <Section
        title="已经取得什么"
        note={creator ? '同一时点的目标事实' : '最近一次成功巡查'}
        className="c-dw-facts"
      >
        <Readouts
          values={
            creator
              ? [
                  [i.works, '作品目录'],
                  [i.details === null || i.works === null ? null : `${i.details}/${i.works}`, '详情进度'],
                  [i.missing, '待取得详情'],
                  [i.latestNew, '最近巡查新增'],
                  [last, '上次巡查'],
                  [next, '下次巡查'],
                ]
              : [
                  [i.hits, '最近命中'],
                  [i.latestNew, '其中新增'],
                  [last, '上次巡查'],
                  [next, '下次巡查'],
                ]
          }
        />
      </Section>
      <section className="c-dw-section c-dw-decision" id={creator ? 'archive-problems' : 'required-action'}>
        <div className="c-dw-decision-copy">
          <span>是否需要处理</span>
          <strong>{data.decision.title}</strong>
          <p>{data.decision.note}</p>
        </div>
        {data.decision.action && (
          <Action
            primary
            target={data.target}
            action={data.decision.action}
            params={params}
            commands={commands}
          />
        )}
        {creator && data.blocked && data.blocked.length > 0 && (
          <form
            className="c-dw-retire"
            onSubmit={(e) => {
              e.preventDefault();
              void commands.run('retire-materials', {
                ...returnFields(params),
                row_target_ref: data.target.targetRef,
                content_public_ref: selection,
                return_drawer: data.target.targetRef,
                return_dtab: 'overview',
              });
            }}
          >
            <b>这些作品连续读不到，等你判断</b>
            <p>
              确认之后它们仍留在作品目录里，只是不再计入待补齐——博主当时确实发过，抹掉分母会让「档案完成」建立在一个修饰过的数字上。判断错了可以重新采集，记录不会挡住它。
            </p>
            <ul>
              {data.blocked.map((b) => (
                <li key={b.publicRef}>
                  <label>
                    <input
                      type="checkbox"
                      checked={selection.includes(b.publicRef)}
                      onChange={(e) =>
                        setRetire(
                          e.target.checked
                            ? [...selection, b.publicRef]
                            : selection.filter((v) => v !== b.publicRef),
                        )
                      }
                    />
                    <b>{b.title ?? '未取得标题'}</b>
                    <span>{b.externalId}</span>
                  </label>
                </li>
              ))}
            </ul>
            <button className="c-btn-primary" disabled={!selection.length || commands.busy}>
              确认这些作品已失效
            </button>
          </form>
        )}
        {data.contentGapAction && (
          <Action
            target={data.target}
            action={data.contentGapAction}
            className="c-btn-secondary"
            params={params}
            commands={commands}
          />
        )}
      </section>
    </>
  );
}
