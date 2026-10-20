import { num, studyTime } from './format';
import type { StudyPolicy } from './methodDtos';
import { c } from './styles';

export function PolicySnapshot({ policy }: { policy: StudyPolicy }) {
  const manifest = policy.methodManifest;
  if (!manifest || !Object.keys(manifest.stages).length) return <p className={c('study-restricted')}>这个历史方法的完整内容未记录。</p>;
  const fixed = Object.fromEntries(Object.entries(manifest).filter(([key]) => key !== 'stages'));
  return <article className={c('study-method-detail')}><h3>{policy.methodName || '未命名方法'}</h3><p>版本 {policy.policyRef} · {studyTime(policy.createdAt)}{policy.isActive ? ' · 当前默认' : ''}</p><p>默认评论上限 {num(policy.defaults?.commentBudget)} 条；语境上限 {num(policy.defaults?.contextCharacterBudget)} 字符。版本哈希 {policy.methodHash || '未记录'}。</p>
    {([['semantic', '评论语义提取'], ['resolution', '问题归并'], ['pair', '独立证据配对']] as const).map(([key, title]) => <section key={key}><h3>{title}</h3>{manifest.stages[key] ? <><pre>{manifest.stages[key].systemInstruction || '说明未记录'}</pre><details><summary>查看严格输出格式定义</summary><pre>{manifest.stages[key].outputSchema == null ? '未记录' : JSON.stringify(manifest.stages[key].outputSchema, null, 2)}</pre></details></> : <p>本阶段说明未记录。</p>}</section>)}
    <details><summary>固定证据规则、清洗器与模型配置</summary><pre>{JSON.stringify({ ...fixed, model: policy.model || null }, null, 2)}</pre></details>
  </article>;
}
