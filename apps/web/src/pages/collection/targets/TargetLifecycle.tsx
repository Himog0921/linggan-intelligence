import { Chart } from './LifecycleChart';
import { WorkResearchActions } from './inspection/TargetResearchContext';
import styles from './inspection/TargetResearch.module.css';
import { Review } from './LifecycleReview';
import { Link,useNavigate } from 'react-router';
import { InspectorSelect } from './inspection/InspectorSelect';
import type { Drawer } from './api';
import { listHref } from './useWorkspace';

// These are display coordinates and summaries of the already-qualified Rust point set.
// No input work is admitted, omitted, or re-qualified by the chart.
import { bucketPoints, metricLabel } from './chartMath';

function State({ title, body }: { title: string; body: string }) {
  return (
    <section className="c-dw-section life-panel"><div className="life-heading"><h2>作品表现</h2></div><div className="life-state"><b>{title}</b><p>{body}</p></div></section>
  );
}
export function LifecyclePanel({
  lifecycle,
  params,
}: {
  lifecycle: Drawer['lifecycle'];
  params: URLSearchParams;
}) {
  const navigate=useNavigate();
  if (lifecycle.state === 'invalid')
    return (
      <State
        title="作品分布的查询条件无效"
        body="地址里的时间范围或指标不受支持。页面没有改用默认条件，也没有把失败读取成空数据。"
      />
    );
  const p = lifecycle.projection;
  if (!p)
    return (
      <State
        title="作品分布当前读不到"
        body="当前读取状态未知，不表示创作者没有作品，也不表示互动数据为零。"
      />
    );
  if (p.status === 'NOT_APPLICABLE')
    return <State title="不适用于当前目标" body="只有已确认身份的创作者目标才能生成作品生命周期。" />;
  const distribution = params.get('life_chart') === 'distribution',
    week = params.get('life_grain') === 'week',
    buckets = bucketPoints(p.points, week),
    selected = p.points.find((v) => v.publicRef === params.get('life_work')),
    selectedBucket = !distribution ? buckets.find(b=>b.label===params.get('life_bucket')) : undefined;
  const viewParams = new URLSearchParams(params);
  viewParams.set('life_chart', distribution ? 'distribution' : 'trend');
  viewParams.set('life_grain', week ? 'week' : 'month');
  if (!selected) viewParams.delete('life_work');
  const exclusions: [string, number][] = [
    ['作者未确认', p.exclusions.authorNotVerified],
    ['作者不匹配', p.exclusions.authorMismatch],
    ['发布时间不合格', p.exclusions.publishedAtNotQualified],
    ['不在当前时间窗', p.exclusions.outsideWindow],
    ['指标未知', p.exclusions.metricUnknown],
  ];
  return (
    <section className="c-dw-section life-panel" id="creator-lifecycle">
      <div className="life-performance-controls">
        <div>
          <div className={styles.toolbar} aria-label="生命周期口径">
            <InspectorSelect label="时间范围" value={p.window} options={[{value:'recent_90_days',label:'近 90 天'},{value:'all',label:'全部周期'}]} onChange={value=>navigate(listHref(viewParams,{life_work:null,life_bucket:null,life_window:value,life_metric:p.metric}))}/>
            <InspectorSelect label="图表指标" value={p.metric} options={[{value:'likes',label:'点赞'},{value:'comments',label:'评论'},{value:'collects',label:'收藏'},{value:'shares',label:'转发'}]} onChange={value=>navigate(listHref(viewParams,{life_work:null,life_window:p.window,life_metric:value}))}/>
          </div>
        </div>
        <p>
          数据截至 <time>{p.asOf}</time>
        </p>
      </div>
      <div className={styles.coverage} aria-label="作品覆盖摘要">
        <span>{p.linked===null?'作品目录下限':'作品目录'} <b>{p.linked===null?`≥${p.lowerBound}`:p.linked}</b> 篇</span>
        <span>{p.linked!==null&&p.linked>0&&p.confirmed===p.linked?'作者均已确认':<>作者已确认 <b>{p.confirmed}</b> 篇</>}</span>
        <span>{p.linked!==null&&p.linked>0&&p.eligible===p.linked?'当前均可分析':<>当前可分析 <b>{p.eligible}</b> 篇</>}</span>
      </div>
      <div className={`life-performance-grid${p.points.length ? '' : ' life-performance-grid-empty'}`}>
        <article className="life-trend-panel">
          {p.points.length ? (
            <Chart p={p} params={viewParams} distribution={distribution} week={week} buckets={buckets} />
          ) : (
            <State
              title="观察不足，暂时无法成图"
              body="作品必须同时具备可确认的作者归属、真实发布时间和当前互动数据。未知值不会按零计算。"
            />
          )}
        </article>
        {p.points.length > 0 && (distribution && selected ? (
          <aside className="life-review life-selected-review" aria-label="选中作品与对照操作">
            <div className="life-review-head"><b>选中作品</b><Link to={listHref(viewParams,{life_work:null},'creator-lifecycle')}>返回复核摘要</Link></div>
            <h3>{selected.title??'标题未取得'}</h3>
            <p className={styles.meta}>{selected.date} · {metricLabel(p.metric)} {selected.value}</p>
            <p className={styles.meta}>当前累计值。</p>
            <WorkResearchActions work={selected.publicRef}/>
          </aside>
        ) : <Review p={p} week={week} distribution={distribution} buckets={buckets} />)}
      </div>
      {selectedBucket && <section className="life-evidence" aria-label="所选时间桶的作品">
        <div className="life-evidence-head"><h2>{selectedBucket.label} · 本桶可分析作品</h2>
          <Link to={listHref(viewParams,{life_bucket:null},'creator-lifecycle')}>清除时间桶</Link></div>
        <div className="life-evidence-table-wrap"><table className="life-evidence-table">
          <thead><tr><th>作品</th><th>发布时间</th><th>{metricLabel(p.metric)}</th><th>对照与证据</th></tr></thead>
          <tbody>{selectedBucket.points.map(work=><tr key={work.publicRef}>
            <td><Link to={listHref(viewParams,{life_work:work.publicRef},'creator-lifecycle')}>{work.title??'标题未取得'}</Link></td>
            <td>{work.date}</td><td>{work.value}</td><td><WorkResearchActions work={work.publicRef}/></td>
          </tr>)}</tbody>
        </table></div>
      </section>}
      {!selectedBucket&&<section className="life-evidence">
        <div className="life-evidence-head">
          <h2>近期作品证据</h2>
          <p>当前窗口最近 4 篇。</p>
        </div>
        {p.points.length ? (
          <div className="life-evidence-table-wrap">
            <table className="life-evidence-table">
              <thead>
                <tr>
                  <th>近期作品证据</th>
                  <th>发布时间</th>
                  <th>{metricLabel(p.metric)}</th>
                  <th>归属</th>
                </tr>
              </thead>
              <tbody>
                {p.points
                  .slice(-4)
                  .reverse()
                  .map((v) => (
                    <tr key={v.publicRef}>
                      <td>
                        <Link
                          className="life-evidence-title"
                          to={listHref(viewParams, { life_work: v.publicRef }, 'creator-lifecycle')}
                        >
                          {v.title ?? '标题未知'}
                        </Link>
                      </td>
                      <td>{v.date}</td>
                      <td className="life-evidence-number">{v.value}</td>
                      <td>
                        {v.association === 'AUTHOR_CONFIRMED' ? '作者已确认' : '目录已列入，作者待确认'}
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="life-evidence-empty">当前窗口没有可展示的单篇作品；未知数据不会替换成占位读数。</p>
        )}
      </section>}
      {(exclusions.some(([, count]) => count > 0) || p.receipt.truncated) && (
        <ul className="life-exclusions" aria-label="未进入当前曲线的原因">
          {exclusions
            .filter(([, count]) => count > 0)
            .map(([label, count]) => (
              <li key={label}>
                {label} {count}
              </li>
            ))}
          {p.receipt.truncated && <li>当前作品范围仍有后续内容</li>}
        </ul>
      )}
      {selected && !distribution && (
        <section className="life-selected" aria-label="选中作品摘要">
          <h3>{selected.title ?? '标题未知'}</h3>
          <dl>
            <div>
              <dt>发布时间</dt>
              <dd>{selected.date}</dd>
            </div>
            <div>
              <dt>{metricLabel(p.metric)}</dt>
              <dd>{selected.value}</dd>
            </div>
          </dl>
          <WorkResearchActions work={selected.publicRef}/>
          <Link className="life-corpus-link" to={`/corpus/evidence?work=${selected.publicRef}`}>
            到语料页查看这篇作品
          </Link>
        </section>
      )}
      <details className={styles.conditions}><summary>覆盖与图表口径</summary><p>可分析指当前时间窗和指标资格，不代表内容分析完成。</p></details>
    </section>
  );
}
