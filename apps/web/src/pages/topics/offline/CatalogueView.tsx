import type { Preview } from './api';
import styles from './OfflineReview.module.css';

export function CatalogueView({ data, select }: { data: Preview; select: (ref: string) => void }) {
  const names = new Map([...data.baselineCatalogue.directions, ...data.candidateCatalogue.directions].map(d => [d.directionRef, d.name]));
  const labels = { keep: '保留', merge: '合并', delete: '删除', new: '新增' };
  return <section className={styles.catalogue}>
    <h2>合成方向清单 · {data.catalogue.directions.length} 个方向</h2><p>只核对结构与合同，不是已发布清单。所有例子均为合成笔记。</p>
    {data.catalogue.groups.map(group => <section key={group.groupRef}><h3>{group.name}</h3><table><caption className="v7-sr-only">{group.name}方向与样例</caption><thead><tr><th>方向 / 定义</th><th>例子编号</th></tr></thead><tbody>{data.catalogue.directions.filter(d => d.groupRef === group.groupRef).map(direction => <tr key={direction.directionRef}><td><strong>{direction.name}</strong><p>{direction.definition}</p></td><td>{direction.exampleWorkRefs.map(ref => <button type="button" key={ref} onClick={() => select(ref)}>{data.notes.find(n => n.input.identity.workRef === ref)?.title.split('：')[0]}</button>)}</td></tr>)}</tbody></table></section>)}
    <details><summary>旧结构对照：保留、合并、删除、新增</summary><p>固定合成旧版与候选版的完整结构对照；切换示例版本不发布方向。</p><table><thead><tr><th>变化</th><th>旧方向</th><th>候选方向</th></tr></thead><tbody>{data.comparison.map((change, i) => <tr key={i}><td>{labels[change.kind]}</td><td>{change.oldRefs.map(ref => names.get(ref)).join('、') || '不适用'}</td><td>{change.newRefs.map(ref => names.get(ref)).join('、') || '不适用'}</td></tr>)}</tbody></table></details>
  </section>;
}

export function StatisticsView({ data }: { data: Preview }) {
  const s = data.summary;
  return <details className={styles.statistics}><summary>查看本次合成样本汇总 · {s.total} 篇</summary>
    <p>只按当前版本的主方向计数，也相关不累计。仅合成样本，不代表市场趋势。</p>
    <p>未知 {s.unknown} 篇 · 不相关 {s.notRelated} 篇 · 暂无合适方向 {s.noSuitableDirection} 篇 · 点赞未知 {s.unknownLikes} 篇 · 发布周未知 {s.unknownWeek} 篇</p>
    <div className={styles.tableScroll}><table><thead><tr><th>主方向</th><th>篇数 / 可能</th><th>过1000 / 过5000赞</th><th>未知点赞 / 周</th><th>发布周 / 篇数</th><th>谁在讲</th><th>关键词前5 / 篇数</th></tr></thead><tbody>{s.directions.map(row => <tr key={row.directionRef}><td>{data.catalogue.directions.find(d => d.directionRef === row.directionRef)?.name}</td><td>{row.count} / {row.possible}</td><td>{row.over1000} / {row.over5000}</td><td>{row.unknownLikes} / {row.unknownWeek}</td><td>{row.weeks.map(([week, count]) => `${week}：${count}`).join('；') || '没有已知发布周'}</td><td>{row.perspectives.map(([name, count]) => `${name}：${count}`).join('；') || '本方向0篇'}</td><td>{row.keywords.map(word => `${word.text}：${word.count}`).join('；') || '无关键词'}</td></tr>)}</tbody></table></div>
    <p>这是人工固定的样本集合，没有真实新增关键词、博主或采集时间；不据此计算库内或市场增长。</p>
  </details>;
}
