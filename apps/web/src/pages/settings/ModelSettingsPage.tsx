import { useQuery } from '@tanstack/react-query';
import { useSearchParams } from 'react-router';
import { AppShell } from '../../app/AppShell';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { SettingsSideNav, type SettingsViewKey } from './SettingsSideNav';
import { AI_SETTINGS_KEY, fetchAiSettings } from './aiSettingsApi';
import { AiModelSection } from './AiModelSection';
import { AiTaskSection } from './AiTaskSection';
import { AiSkillSection } from './AiSkillSection';
import { AiHealthSection } from './AiHealthSection';
import { CreativeBudget } from './CreativeBudget';
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
  const [query, setQuery] = useSearchParams();
  // Existing deep links and automated clients keep their historical entry points.
  // New user navigation only exposes the four capability-first views.
  const oldSection = query.get('section');
  const legacy = oldSection && Object.hasOwn(LEGACY, oldSection) ? oldSection as keyof typeof LEGACY : null;
  const requested = query.get('view');
  const view = requested && Object.hasOwn(VIEWS, requested) ? requested as keyof typeof VIEWS : 'capabilities';
  const settings = useQuery({ queryKey: AI_SETTINGS_KEY, queryFn: ({ signal }) => fetchAiSettings(signal), retry: false, refetchOnWindowFocus: false });
  const action = useAiAction();
  const data = settings.data;
  const crumb = <ContextBreadcrumb items={legacy
    ? ['设置', 'AI 设置', LEGACY[legacy]]
    : ['设置', 'AI 设置', VIEWS[view]]} />;
  const meta = data ? <small>数据读取：{humanMoment(new Date(settings.dataUpdatedAt).toISOString())}</small> : null;
  const notice = <>
    <NoticeLine notice={action.notice}/>
    {settings.isError ? <div role="alert" className={styles.error}>AI 设置读取失败，无法核对当前状态。{settings.error instanceof ApiError ? ' ' + settings.error.message : ''} {data ? '下方为上一次成功读取的内容。' : ''}</div> : null}
    {settings.isPending ? <p role="status" className={styles.loading}>正在读取 AI 设置…</p> : null}
  </>;
  const legacySurface = data ? legacy === 'models' ? <AiModelSection data={data} action={action}/> : legacy === 'automation' ? <><CreativeBudget/><AiTaskSection data={data} action={action}/></> : legacy === 'skills' ? <AiSkillSection data={data} action={action}/> : <AiHealthSection data={data} action={action}/> : null;
  const viewSurface = data ? view === 'capabilities' ? <><CreativeBudget/><AiCapabilities data={data} action={action}/></> : view === 'models' ? <AiConnections data={data} action={action}/> : view === 'runs' ? <AiRuns data={data}/> : <AiAdvanced data={data} action={action}/> : null;

  const settingsContent = legacy ? (
    <main className={[legacyStyles.page, styles.root].join(' ')} data-ai-settings-page aria-labelledby="ai-settings-page-title">
      <h1 id="ai-settings-page-title" className="v7-sr-only">AI 设置</h1>
      <div className={styles.inner}>{notice}{data ? <div className={legacyStyles.surface}>{legacySurface}</div> : null}</div>
    </main>
  ) : (
    <main className={[legacyStyles.page, styles.root].join(' ')} data-ai-settings-page aria-labelledby="ai-settings-page-title">
      <h1 id="ai-settings-page-title" className="v7-sr-only">AI 设置</h1>
      <div className={styles.inner}>{notice}{viewSurface}</div>
    </main>
  );

  return <AppShell active={null} boundaryLabel={data?.synthetic ? '合成隔离环境' : '本机设置'} contextBar={{ breadcrumb: crumb, meta }}>
    <div className="v7-shell">
      <SettingsSideNav
        activeId={legacy ? 'ai-settings' : 'ai-' + view}
        showTeam={false}
        onViewSelect={(next: SettingsViewKey) => setQuery({ view: next })}
      />
      {settingsContent}
    </div>
  </AppShell>;
}
