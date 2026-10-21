import { useQuery } from '@tanstack/react-query';
import { Link, useSearchParams } from 'react-router';
import { AppShell } from '../../app/AppShell';
import { ActionButton } from '../../shared/ActionButton';
import { AI_SETTINGS_KEY, fetchAiSettings } from './aiSettingsApi';
import { AiModelSection } from './AiModelSection';
import { AiTaskSection } from './AiTaskSection';
import { AiSkillSection } from './AiSkillSection';
import { AiHealthSection } from './AiHealthSection';
import { AiAdvanced, AiCapabilities, AiConnections, AiRuns } from './AiSettingsExperience';
import { NoticeLine } from './AiSettingsFields';
import { useAiAction } from './useAiAction';
import { ApiError } from '../../shared/api';
import { humanMoment } from '../../shared/formatting';
import legacyStyles from './AiSettings.module.css';
import styles from './AiSettingsExperience.module.css';

const LEGACY = { models: '模型', automation: '自动化与额度', skills: '做法', health: '运行状况' };
const VIEWS = { capabilities: 'AI 能力', models: '模型连接', runs: '运行与用量', advanced: '高级配置' };
export function ModelSettingsPage() {
  const [query] = useSearchParams();
  // Existing deep links and automated clients keep their historical entry points.
  // New user navigation only exposes the four capability-first views.
  const oldSection = query.get('section');
  const legacy = oldSection && Object.hasOwn(LEGACY, oldSection) ? oldSection as keyof typeof LEGACY : null;
  const requested = query.get('view');
  const view = requested && Object.hasOwn(VIEWS, requested) ? requested as keyof typeof VIEWS : 'capabilities';
  const settings = useQuery({ queryKey: AI_SETTINGS_KEY, queryFn: ({ signal }) => fetchAiSettings(signal), retry: false, refetchOnWindowFocus: false });
  const action = useAiAction();
  const data = settings.data;
  const crumb = <span className={styles.crumb}><span>设置</span><span className="v7-slash">/</span><b>AI 设置</b></span>;
  const contextNav = <nav aria-label="AI 设置视图" className={styles.nav}>{Object.entries(VIEWS).map(([key, label], index) =>
    <Link to={'?view=' + key} key={key} aria-current={!legacy && view === key ? 'page' : undefined}><span>{String(index + 1).padStart(2, '0')}</span>{label}</Link>)}</nav>;
  const meta = data ? <small>数据读取：{humanMoment(new Date(settings.dataUpdatedAt).toISOString())}</small> : null;
  return <AppShell active={null} boundaryLabel={data?.synthetic ? '合成隔离环境' : '本机设置'} crumb={crumb} contextNav={contextNav} meta={meta}>
    <main className={[legacyStyles.page, styles.root].join(' ')}><div className={styles.inner}>
      <header className={styles.heading}><div className={styles.headingText}><small>SYSTEM / AI OPERATIONS</small><h1>AI 设置</h1><p>{data?.synthetic ? '合成隔离环境：数据与回执不代表真实供应商状态。' : '管理研究能力、模型连接与运行结果，让每一项 AI 工作有据可查。'}</p></div>
        <div className={styles.headingMeta}><ActionButton disabled={action.busy !== null || settings.isFetching} onClick={() => void settings.refetch()}>刷新设置</ActionButton></div>
      </header>
      <NoticeLine notice={action.notice}/>
      {settings.isError ? <div role="alert" className={styles.error}>AI 设置读取失败，无法核对当前状态。{settings.error instanceof ApiError ? ' ' + settings.error.message : ''} {data ? '下方为上一次成功读取的内容。' : ''}</div> : null}
      {settings.isPending ? <p role="status" className={styles.loading}>正在读取 AI 设置…</p> : data ? legacy ?
        <div className={legacyStyles.workspace}><nav className={legacyStyles.index} aria-label="旧版 AI 设置分区">{Object.entries(LEGACY).map(([key, label]) => <Link key={key} to={'?section=' + key} aria-current={legacy === key ? 'page' : undefined}>{label}</Link>)}</nav>
          <div className={legacyStyles.surface}>{legacy === 'models' ? <AiModelSection data={data} action={action}/> : legacy === 'automation' ? <AiTaskSection data={data} action={action}/> : legacy === 'skills' ? <AiSkillSection data={data} action={action}/> : <AiHealthSection data={data} action={action}/>}</div>
        </div> : view === 'capabilities' ? <AiCapabilities data={data}/> : view === 'models' ? <AiConnections data={data} action={action}/> : view === 'runs' ? <AiRuns data={data}/> : <AiAdvanced data={data} action={action}/> : null}
    </div></main>
  </AppShell>;
}
