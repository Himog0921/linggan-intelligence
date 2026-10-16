import type { ReactNode } from 'react';
import type { RulePanel } from './api';

function receiptMeaning(outcome: string, reason: string) {
  if (outcome === 'replay') return '相同命令已处理过；这里显示的是耐久重放结果。';
  if (outcome === 'stale_revision') return '页面基于旧版本提交，没有覆盖当前规则。请核对当前值后重试。';
  if (outcome === 'identity_conflict') return '同一命令标识携带了不同内容，没有覆盖原命令。';
  const applied: Record<string, string> = {
    rule_saved: '规则已保存。没有据此声称采集已开始或完成。',
    monitor_paused: '未来自动调度已暂停；人工观察仍可申请。',
    monitor_resumed: '自动调度已恢复；尚未据此声称已经派出任务。',
    monitor_stopped: '目标已停止新增工作，既有历史仍保留。',
    manual_observe_created: '人工观察工单已入队，尚未被插件认领；这不是执行或采集回执。',
    manual_observe_reused: '已有工作覆盖这次人工观察，没有重复创建。',
  };
  if (outcome === 'applied' && applied[reason]) return applied[reason];
  const rejected: Record<string, string> = {
    baseline_not_ready: '关键词建档尚未完成：先完成搜索面和待补详情，再保存或恢复巡查规则。',
    invalid_interval: '观察间隔只支持 6 小时、12 小时、24 小时、2 天或 7 天。',
    invalid_schedule: '当前版本只接受固定、全天的单一观察间隔。',
    target_not_requestable: '这个目标当前不允许新增观察工作。',
  };
  return rejected[reason] ?? '命令没有改变当前规则，也没有创建采集事实。';
}
export function RuleReceipt({ receipt }: { receipt: RulePanel['receipt'] }) {
  if (!receipt) return null;
  const [tone, title] = ({
    applied: ['ok', '命令已应用'], replay: ['info', '已返回原命令结果'],
    stale_revision: ['warn', '规则版本已经变化'], identity_conflict: ['bad', '命令标识发生冲突'],
  } as Record<string, [string, string]>)[receipt.outcome] ?? ['bad', '命令未应用'];
  return <section className={`c-rule-feedback c-rule-feedback-${tone}`} data-outcome={receipt.outcome}>
    <div><b>{title}</b><span>{receipt.reason}</span></div>
    <p>{receiptMeaning(receipt.outcome, receipt.reason)}</p>
    <dl>
      <div><dt>回执</dt><dd>{receipt.receiptRef}</dd></div>
      <div><dt>记录时间</dt><dd>{receipt.recordedAt}</dd></div>
      <div><dt>结果</dt><dd>{receipt.appliedRuleRevisionRef ? `规则 ${receipt.appliedRuleRevisionRef.slice(0, 8)}` : '未产生规则版本'}</dd></div>
    </dl>
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
