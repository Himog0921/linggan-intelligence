import { Sidebar, type SidebarNavigationItem } from '../../shared/Sidebar';

export type SettingsViewKey = 'capabilities' | 'models' | 'runs' | 'advanced';

const labels: Record<SettingsViewKey, string> = {
  capabilities: 'AI 能力',
  models: '模型连接',
  runs: '运行与用量',
  advanced: '高级配置',
};

const codes: Record<SettingsViewKey, string> = {
  capabilities: 'CAP',
  models: 'MOD',
  runs: 'RUN',
  advanced: 'ADV',
};

export function SettingsSideNav({
  activeId,
  showAi = true,
  onViewSelect,
}: {
  activeId: string;
  showAi?: boolean;
  onViewSelect?: (view: SettingsViewKey) => void;
}) {
  const children: SidebarNavigationItem[] = (Object.keys(labels) as SettingsViewKey[]).map(view => ({
    id: `ai-${view}`,
    label: labels[view],
    code: codes[view],
    href: `/settings/models?view=${view}`,
    onSelect: onViewSelect ? () => onViewSelect(view) : undefined,
  }));
  const items: SidebarNavigationItem[] = [
    ...(showAi ? [{
      id: 'ai-settings',
      label: 'AI 设置',
      index: '01',
      icon: 'diagnostics',
      href: '/settings/models',
      children,
    }] : []),
    {
      id: 'account-security',
      label: '账户安全',
      index: '02',
      icon: 'security',
      href: '/settings/account',
    },
  ];

  return (
    <Sidebar
      aria-label="设置导航"
      eyebrow="SYSTEM / NAV"
      title="设置工作区"
      items={items}
      activeId={activeId}
      activeParentId={activeId.startsWith('ai-') ? 'ai-settings' : undefined}
    />
  );
}
