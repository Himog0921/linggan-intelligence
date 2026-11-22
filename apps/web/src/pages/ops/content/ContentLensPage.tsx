import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import { reason } from '../api';
import { Status } from '../ui';
import { generate, getSnapshot, percent, submitRun, useLensStatus, type Drill } from './api';
import { Coverage, Effects, Quality, Structure } from './LensSections';
import { PublicationList, SpotList } from './LensDetails';
import css from './Lens.module.css';

export function ContentLensPage({ domain }: { domain:string }) {
  const client = useQueryClient();
  const status = useLensStatus(true,true);
  const active = status.data?.run?.inProgress;
  const snapshot = useQuery({queryKey:['ops','content-lens','snapshot',domain],retry:false,
    enabled:status.isSuccess && status.data.enabled && !active,
    queryFn:({signal}) => getSnapshot(domain,signal)});
  const [ids] = useState(() => new RequestIdentities());
  const [detail,setDetail] = useState<{ref:string; drill:Drill|'spot'} | null>(null);
  const action = useMutation({
    mutationFn: async (name:'label'|'snapshot') => {
      if (name === 'snapshot') { await generate(domain); return '快照已生成'; }
      try {
        const ref = await submitRun(ids.keyFor('label','all'));
        ids.settle('label');
        return ref ? '标注任务已提交' : '当前没有待标标题';
      } catch (error) { if (error instanceof ApiError && error.answered) ids.settle('label'); throw error; }
    },
    onSuccess: async () => { await client.invalidateQueries({ queryKey:['ops','content-lens'] }); },
  });
  if (status.isError) return <Status error retry={() => void status.refetch()}>看板状态读取失败（{reason(status.error)}）。</Status>;
  if (status.isPending) return <Status>正在读取看板状态…</Status>;
  if (!status.data.enabled) return <Status>内容回溯未启用。</Status>;
  const run = status.data.run;
  const phase = run?.audit ?? run?.primary;
  const current = snapshot.isSuccess ? snapshot.data : null;
  const open = (drill:Drill) => current && setDetail({ref:current.ref,drill});
  return <div className={css.board}>
    <div className={css.top}><h2>内容回溯</h2><div className={css.controls}>
      <ActionButton disabled={active || action.isPending} onClick={() => action.mutate('label')}>标注全部业务待标标题</ActionButton>
      <ActionButton disabled={active || action.isPending} onClick={() => action.mutate('snapshot')}>生成当前范围快照</ActionButton>
    </div></div>
    {action.isError && <p role="alert">操作未完成（{reason(action.error)}）。</p>}
    {action.isSuccess && <p role="status">{action.data}</p>}
    {active && run && phase ? <section className={css.section} aria-live="polite"><h2>{run.audit ? '独立复标进行中' : '标注进行中'}</h2>
      <progress aria-label="标注进度" max="1" value={run.progress} /> <strong>{percent(run.progress)}</strong>
      <p>已完成 {phase.succeeded} / {phase.total} 个分片</p></section>
    : snapshot.isError ? <Status error retry={() => void snapshot.refetch()}>快照读取失败（{reason(snapshot.error)}）。</Status>
    : snapshot.isPending ? <Status>正在读取快照…</Status> : !current ? <Status>{run ? '当前范围暂无快照。完成标注与复标后可生成快照。' : '暂无标注数据。请先完成一次标题标注与复标，再生成快照。'}</Status>
    : <>
      <Quality snapshot={current} open={open} spot={() => setDetail({ref:current.ref,drill:'spot'})}/>
      <Structure snapshot={current} open={open} /><Coverage snapshot={current} open={open} /><Effects snapshot={current} open={open} />
      <section className={css.section}><h2>假设与检验</h2><p>暂无已登记的假设。</p></section>
      {detail && detail.ref === current.ref && (detail.drill === 'spot'
        ? <SpotList snapshot={current} close={() => setDetail(null)} />
        : <PublicationList snapshotRef={detail.ref} drill={detail.drill} close={() => setDetail(null)} />)}
    </>}
  </div>;
}
