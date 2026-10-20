// Synthetic only. Shared by settings tests; never imported by the application.
import { readFileSync } from 'node:fs';
const resource = JSON.parse(readFileSync('../../crates/contracts/src/ai_limits.json', 'utf8'));
const protocol = JSON.parse(readFileSync('../../crates/contracts/src/ai_protocol.json', 'utf8'));
export function settingsFixture() {
  const configuration = { contract: 'ai-task-configuration.v1', taskConfigRef: 'config-1', taskKey: 'model_probe', executionContract: 'ai-model-probe.v1', skillVersionRef: null,
    materialColumns: [], budget: { mode: 'unconfigured' }, origin: 'seed', createdAt: '2026-10-20T00:00:00Z', model: null };
  return { contract: 'ai-settings.v1', synthetic: true,
    connections: [{ connectionRef: 'connection-1', enabled: true, revision: 3, versionRef: 'version-1', name: '隔离合成连接', api: 'openai-completions', baseUrl: 'http://127.0.0.1:9/v1', localEndpoint: true, endpointRef: 'endpoint-1' }],
    models: [{ identityRef: 'identity-1', connectionRef: 'connection-1', modelId: 'synthetic-model', endpointRef: 'endpoint-1', modelRef: null, selectionModelRef: 'old-model-ref', connectionVersionRef: 'version-1', probe: null, capability: null, price: null }],
    controls: [{ scope: 'global', taskKey: null, enabled: false, revision: 0, actor: 'human', reason: '初始关闭', commandRef: null, updatedAt: '2026-10-20T00:00:00Z', label: null, lifecycle: null, automaticBrake: null },
      { scope: 'executor', taskKey: null, enabled: false, revision: 0, actor: 'human', reason: '初始关闭', commandRef: null, updatedAt: '2026-10-20T00:00:00Z', label: null, lifecycle: null, automaticBrake: null },
      { scope: 'task:model_probe', taskKey: 'model_probe', enabled: false, revision: 0, actor: 'human', reason: '初始关闭', commandRef: null, updatedAt: '2026-10-20T00:00:00Z', label: '模型测试', lifecycle: 'active', automaticBrake: false }],
    egress: [], skills: [], tasks: [{ taskKey: 'model_probe', label: '模型测试', lifecycle: 'active', requiresSkill: false, skillAllowed: false, handlerContract: 'ai-model-probe.v1', revision: 0, configuration,
      bindingRevision: 0, activeSkillVersionRef: null, approvalSource: null, approvalReason: null }], configurations: [configuration], trials: [],
    execution: { service: { state: 'not_started', observedState: 'not_started' }, dailyUsage: [], failures: [], lateUsage: { receiptCount: '0', knownInputTokens: null, knownOutputTokens: null, quotaReconciled: false } }, batches: [], resources: resource, protocol };
}

/** The three business surfaces consume the same closed settings contract. */
export function businessSettingsFixture() {
  const root = settingsFixture();
  const lanes = [
    { taskKey: 'comment_study', label: '评论研究', lifecycle: 'frozen' },
    { taskKey: 'legacy_creator_discovery', label: '旧创作者分析', lifecycle: 'retired' },
    { taskKey: 'legacy_topic_map', label: '旧主题研究', lifecycle: 'retired' },
  ];
  for (const lane of lanes) {
    const configuration = { ...root.configurations[0]!, taskKey: lane.taskKey, taskConfigRef: `config-${lane.taskKey}` };
    root.tasks.push({ ...root.tasks[0]!, ...lane, configuration });
    root.configurations.push(configuration);
    root.controls.push({ ...root.controls[2]!, scope: `task:${lane.taskKey}`, ...lane, automaticBrake: false });
  }
  return root;
}
