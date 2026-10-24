import type { Lifecycle } from './api';
import { compact, exact, metricLabel, percentile, type Bucket } from './chartMath';
function ReviewItem({
  label,
  emphasis,
  value,
  body,
  exactLabel,
}: {
  label: string;
  emphasis?: string;
  value: string | number;
  body: string;
  exactLabel?: string;
}) {
  return (
    <div className="life-review-item">
      <details>
        <summary><span>{label}{emphasis&&` · ${emphasis}`}</span><strong title={exactLabel} aria-label={exactLabel}>{value}</strong></summary>
        <p>{body}</p>
      </details>
    </div>
  );
}
export function Review({
  p,
  week,
  distribution,
  buckets,
}: {
  p: Lifecycle;
  week: boolean;
  distribution: boolean;
  buckets: Bucket[];
}) {
  const values = p.points.map((v) => v.value),
    metric = metricLabel(p.metric),
    newCount = p.points.filter((v) => v.new).length;
  return (
    <aside className="life-review" aria-label="复核摘要">
      <div className="life-review-head">
        <b>复核读数</b>
      </div>
      {distribution ? (
        <>
          <ReviewItem
            label="典型区间"
            exactLabel={`精确区间 ${exact(percentile(values, 0.25) ?? 0)}–${exact(percentile(values, 0.75) ?? 0)}`}
            emphasis="25%–75%"
            value={`${compact(percentile(values, 0.25) ?? 0)}–${compact(percentile(values, 0.75) ?? 0)}`}
            body={`按当前纳入作品的${metric}第 25 至第 75 百分位计算，不表示行业常态或质量判断。`}
          />
          <ReviewItem
            label="高讨论率"
            emphasis="> 20%"
            value={p.points.filter((v) => v.discussionRate !== null && v.discussionRate > 0.2).length}
            body="只标记评论／点赞可判且超过 20% 的作品；它是复核信号，不改变纵轴指标。"
          />
          <ReviewItem
            label="讨论率未可判"
            value={p.points.filter((v) => v.discussionRate === null).length}
            body="评论或点赞未知，或点赞为 0 时不计算比率，不写成低讨论率。"
          />
        </>
      ) : (
        <>
          <ReviewItem
            label={`当前窗口中位${metric}`}
            exactLabel={`精确中位数 ${exact(percentile(values, 0.5) ?? 0)}`}
            value={compact(percentile(values, 0.5) ?? 0)}
            body="只按当前可分析作品计算；缺少指标的作品不会以零参与中位数。"
          />
          <ReviewItem
            label="发布密度"
            value={`${(p.eligible / Math.max(1, buckets.length)).toFixed(1)} / ${week ? '活跃周' : '活跃月'}`}
            body={`按当前可分析作品数除以 ${buckets.length} 个${week ? '活跃周' : '活跃月'}计算；没有发布的时间桶不参与分母。`}
          />
          <ReviewItem
            label="最近巡查新增"
            value={`${newCount} 篇`}
            body="这里仅标记已进入当前可分析集的新增作品，未把读取未知写成零。"
          />
        </>
      )}
      <details className="life-review-boundary">
        <summary>复核边界</summary>
        <p>
          {distribution
            ? '先点击具体作品查证其来源与语境；点的高低不等于内容价值。'
            : '观察结果不等于结论。先查证单篇作品与其来源，再做解释。'}
        </p>
      </details>
    </aside>
  );
}
