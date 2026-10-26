import type { ReactNode } from 'react';
import type { RulePanel } from './api';

function receiptMeaning(outcome: string, reason: string) {
  if (outcome === 'stale_revision'||reason==='stale_revision') return '页面基于旧版本提交，没有覆盖当前规则。请核对当前值后重试。';
  if (outcome === 'identity_conflict'||reason==='identity_conflict') return '同一命令标识携带了不同内容，没有覆盖原命令。';
  const rejected: Record<string, string> = {
    baseline_not_ready: '关键词建档尚未完成：先完成搜索面和待补详情，再保存或恢复巡查规则。',
    invalid_interval: '观察间隔只支持 6 小时、12 小时、24 小时、2 天或 7 天。',
    invalid_schedule: '当前版本只接受固定、全天的单一观察间隔。',
    target_not_requestable: '这个目标当前不允许新增观察工作。',
  };
  return rejected[reason] ?? '命令没有改变当前规则，也没有创建采集事实。';
}
export function RuleReceipt({ receipt }: { receipt: RulePanel['receipt'] }) {
  if (!receipt||receipt.outcome==='applied'||(receipt.outcome==='replay'&&['rule_saved','monitor_paused','monitor_resumed','monitor_stopped','manual_observe_created','manual_observe_reused'].includes(receipt.reason))) return null;
  const outcome=receipt.outcome==='replay'?receipt.reason:receipt.outcome;
  const [tone, title] = ({
    stale_revision: ['warn', '规则版本已经变化'], identity_conflict: ['bad', '命令标识发生冲突'],
  } as Record<string, [string, string]>)[outcome] ?? ['bad', '命令未应用'];
  return <section className={`c-rule-feedback c-rule-feedback-${tone}`} data-outcome={receipt.outcome}>
    <div><b>{title}</b></div>
    <p>{receiptMeaning(receipt.outcome, receipt.reason)}</p>
  </section>;
}
export function RuleErrors({ code }: { code: string | null }) {
  if (!code) return null;
  const summary: Record<string, string> = {
    stale_revision: '规则版本已变化；下面保留了你提交的值，没有覆盖当前版本。',
    monitor_rule_command_stale: '规则版本已变化；下面保留了你提交的值，没有覆盖当前版本。',
    identity_conflict: '命令标识冲突；原命令仍保留，下面的值尚未应用。',
    baseline_not_ready: '关键词建档尚未完成：先完成搜索面和待补详情，再保存或恢复巡查规则。',
    read_model_not_connected: '本机规则读写当前不可用；没有写入规则或采集工作。',
  };
  return <div className="c-rule-errors" role="alert" data-error-code={code}>
    <b>这次没有保存</b><p>{summary[code] ?? '请修正标出的字段后再提交；当前规则没有改变。'}</p>
    <ul>{code === 'invalid_mode' && <li><a href="#mode">请选择一个受支持的运行方式。</a></li>}</ul>
  </div>;
}

function ruleReadFailure(code: string | null) {
  if (code === 'target_not_found') return { title: '没有找到这个观察目标', body: '这个本机标识没有对应目标；它不证明平台上的对象不存在，也不证明目标曾被删除。', code: 'TARGET_NOT_FOUND' };
  if (code === 'rule_schema_unavailable') return { title: '监控规则当前不可读', body: '规则表尚未可用。这次没有保存任何规则，也没有创建采集工作。', code: 'RULE_SCHEMA_UNAVAILABLE' };
  return { title: '监控规则读取失败', body: '目标列表仍可读取，但这个目标的规则状态当前未知。刷新会重新读取，不会自动重试或创建工作。', code: 'RULE_READ_UNAVAILABLE' };
}

export function RuleUnavailable({ code, close }: { code: string | null; close: ReactNode }) {
  const failure = ruleReadFailure(code);
  return <header className="c-rule-head"><div><p className="c-rule-context">{failure.code}</p><h2 id="c-rule-title">{failure.title}</h2><p>{failure.body}</p></div>{close}</header>;
}
