import type { TopicProgress as Progress } from './types';
import { number } from './topicModel';
import s from './Overview.module.css';
export function TopicProgress({ progress }: { progress?: Progress }) {
  const count = (value: number | undefined) => value === undefined ? '—' : number(value);
  const groups = [
    { name: '材料准备', values: [['就绪', progress?.material.ready], ['准备中', progress?.material.preparing], ['受阻', progress?.material.blocked], ['未知', progress?.material.unknown]] },
    { name: '内容理解', values: [['可用', progress?.understanding.available], ['未理解', progress?.understanding.missing], ['待更新', progress?.understanding.stale], ['未知', progress?.understanding.unknown]] },
    { name: '主题归属', values: [['已归主题', progress?.graph.assigned], ['待归主题', progress?.graph.unassigned]] },
  ] as const;
  return <section className={s.progress} aria-label="当前范围处理进度">{groups.map(group => <div key={group.name}>
    <div className={s.label}>{group.name}</div>
    <div className={s.detail}>{group.values.map(([label, value]) => <span key={label}>{label} <b>{count(value)}</b></span>)}</div>
  </div>)}</section>;
}
