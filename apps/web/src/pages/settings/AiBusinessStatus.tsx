import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../shared/ActionButton';
import { AI_SETTINGS_KEY, command, fetchAiSettings } from './aiSettingsApi';
import { controlStatus, TASK_STATE } from './aiSettingsModel';
import { NoticeLine } from './AiSettingsFields';
import { useAiAction } from './useAiAction';
import styles from './AiSettings.module.css';

const LANES = {
  comment_study: { label: '评论研究 AI', description: '已停用，接入时间另定；历史结果仍可读。' },
  legacy_creator_discovery: { label: '创作者 AI', description: '旧分析已停用；作者简介判断在第二步接入。' },
  legacy_topic_map: { label: '主题研究 AI', description: '旧主题研究已退役；材料与历史结果仍可读。' },
};
export function AiBusinessStatus({ taskKey }: { taskKey: keyof typeof LANES }) {
  const lane = LANES[taskKey];
  const query = useQuery({ queryKey: AI_SETTINGS_KEY, queryFn: ({ signal }) => fetchAiSettings(signal), retry: false, refetchOnWindowFocus: false });
  const action = useAiAction();
  const task = query.data?.tasks.find(t => t.taskKey === taskKey);
  const control = query.data?.controls.find(c => c.taskKey === taskKey);
  function pause() {
    if (!control) return;
    const payload = { scope: { kind: 'task', task: taskKey }, expectedRevision: control.revision, enabled: false, reason: `从${lane.label}业务页暂停` };
    void action.run(`pause:${taskKey}`, payload, commandRef => command('control', { ...payload, commandRef }), '统一工作开关已关闭，未创建新任务。');
  }
  return <section className={styles.businessStatus} aria-label={`${lane.label}状态`}><p><b>{lane.label}</b>：{lane.description}</p>
    <p>{query.isError ? '开关读取失败，当前值待核对。' : query.isPending ? '正在读取统一工作开关…' : task && control ? `${TASK_STATE[task.lifecycle] ?? '接入状态未识别'} · ${controlStatus(control)}` : '统一工作记录暂不可用。'}</p>
    <div className={styles.actions}><ActionButton onClick={pause} disabled={!control?.enabled || action.busy !== null}>{control && !control.enabled ? '工作开关已关闭' : '暂停此项 AI'}</ActionButton><a href={`/settings/models?section=automation&task=${taskKey}`}>去 AI 设置</a></div>{action.notice ? <NoticeLine notice={action.notice} /> : null}
  </section>;
}
