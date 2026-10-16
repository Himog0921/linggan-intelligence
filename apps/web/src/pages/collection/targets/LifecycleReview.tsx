import type { CSSProperties, ReactNode } from 'react';
import type { Lifecycle } from './api';
import { compact, exact, metricLabel, percentile, type Bucket } from './chartMath';
function ReviewItem({
  label,
  emphasis,
  value,
  body,
  children,
  exactLabel,
}: {
  label: string;
  emphasis?: string;
  value: string | number;
  body: string;
  children?: ReactNode;
  exactLabel?: string;
}) {
  return (
    <div className="life-review-item">
      <div>
        <span>{label}</span>
        {emphasis && <em>{emphasis}</em>}
      </div>
      <strong title={exactLabel} aria-label={exactLabel}>{value}</strong>
      <p>{body}</p>
      {children}
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
    newCount = p.points.filter((v) => v.new).length,
    maxPosts = Math.max(1, ...buckets.map((b) => b.points.length));
  return (
    <aside className="life-review" aria-label="复核摘要">
      <div className="life-review-head">
        <b>复核摘要</b>
        <span>{distribution ? '把“分布”转成逐篇可查的范围。' : '把“变化”转成可核验线索。'}</span>
      </div>
      {distribution ? (
        <>
          <ReviewItem
            label="当前纳入作品"
            value={p.eligible}
            body="只包括当前时间窗与当前指标均为已知的作品；Known zero 仍保留在图中。"
          />
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
          <ReviewItem
            label="当前指标未知"
            value={p.exclusions.metricUnknown}
            body="这类作品没有进入当前分布；未知不以 0 或典型区间的边界替代。"
          />
        </>
      ) : (
        <>
          <ReviewItem
            label={`当前窗口中位${metric}`}
            exactLabel={`精确中位数 ${exact(percentile(values, 0.5) ?? 0)}`}
            value={compact(percentile(values, 0.5) ?? 0)}
            body="只按当前可分析作品计算；缺少指标的作品不会以零参与中位数。"
          >
            <div className="life-mini-strip">
              {buckets.slice(-7).map((b) => (
                <i
                  key={b.label}
                  className={b.points.some((v) => v.new) ? 'life-mini-hot' : undefined}
                  style={{ '--life-strip-size': (2 + (b.points.length / maxPosts) * 5).toFixed(3) } as CSSProperties}
                />
              ))}
            </div>
          </ReviewItem>
          <ReviewItem
            label="发布密度"
            emphasis={`${(p.eligible / Math.max(1, buckets.length)).toFixed(1)} / ${week ? '活跃周' : '活跃月'}`}
            value={p.eligible}
            body={`当前窗口内 ${p.eligible} 篇可分析作品，分布在 ${buckets.length} 个${week ? '按周' : '按月'}时间桶。`}
          />
          <ReviewItem
            label="最近巡查新增"
            emphasis={`${newCount} 篇`}
            value={newCount}
            body="这里仅标记已进入当前可分析集的新增作品，未把读取未知写成零。"
          />
        </>
      )}
      <div className="life-review-boundary">
        <b>复核边界</b>
        <p>
          {distribution
            ? '先点击具体作品查证其来源与语境；点的高低不等于内容价值。'
            : '观察结果不等于结论。先查证单篇作品与其来源，再做解释。'}
        </p>
      </div>
    </aside>
  );
}
