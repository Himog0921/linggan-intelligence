import { useQuery } from '@tanstack/react-query';
import { Link, useSearchParams } from 'react-router';
import { AppShell } from '../../app/AppShell';
import { ActionButton } from '../../shared/ActionButton';
import { AI_SETTINGS_KEY, fetchAiSettings } from './aiSettingsApi';
import { AiModelSection } from './AiModelSection';
import { AiTaskSection } from './AiTaskSection';
import { AiSkillSection } from './AiSkillSection';
import { AiHealthSection } from './AiHealthSection';
import { NoticeLine } from './AiSettingsFields';
import { useAiAction } from './useAiAction';
import { ApiError } from '../../shared/api';
import { humanMoment } from '../../shared/formatting';
import styles from './AiSettings.module.css';

const SECTIONS = { models: '模型', automation: '自动化与额度', skills: '做法', health: '运行状况' };
export function ModelSettingsPage() {
  const [query] = useSearchParams(); const selected = query.get('section') ?? 'models';
  const section = Object.hasOwn(SECTIONS, selected) ? selected : 'models';
  const settings = useQuery({ queryKey: AI_SETTINGS_KEY, queryFn: ({ signal }) => fetchAiSettings(signal), retry: false, refetchOnWindowFocus: false });
  const action = useAiAction();
  const data = settings.data;
  return <AppShell active={null} boundaryLabel={data?.synthetic ? '合成隔离环境' : '本机设置'} crumb={<>设置 / AI 设置</>} meta="" className={styles.shell}>
    <main className={styles.page}><header className={styles.header}><div><h1>AI 设置</h1><p>{data?.synthetic ? '当前使用合成隔离环境，回执不代表真实供应商或共享运行状态。' : '连接、工作配置、固定做法与运行回执。'}</p></div><ActionButton disabled={action.busy !== null || settings.isFetching} onClick={() => void settings.refetch()}>刷新设置</ActionButton></header>
      <NoticeLine notice={action.notice} />{settings.isError ? <div role="alert"><p>AI 设置读取失败，未采用无法核对的内容；保留上次成功读取的内容，当前状态待重新核对。{settings.error instanceof ApiError ? ` ${settings.error.message}` : ''}</p></div> : null}
      {data ? <p><small>本页读取时间：{humanMoment(new Date(settings.dataUpdatedAt).toISOString())}。运行状态以本次数据库回执为准。</small></p> : null}
      {settings.isPending ? <p role="status">正在读取 AI 设置…</p> : data ? <div className={styles.workspace}><nav className={styles.index} aria-label="AI 设置分区">{Object.entries(SECTIONS).map(([key, label]) => <Link key={key} to={`?section=${key}`} aria-current={section === key ? 'page' : undefined}>{label}</Link>)}</nav>
        <div className={styles.surface}>{section === 'models' ? <AiModelSection data={data} action={action} /> : section === 'automation' ? <AiTaskSection data={data} action={action} /> : section === 'skills' ? <AiSkillSection data={data} action={action} /> : <AiHealthSection data={data} action={action} />}</div>
      </div> : null}
    </main>
  </AppShell>;
}
