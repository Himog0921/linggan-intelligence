import { apiPath } from './evidencePaths';
import type { Reobservation as Channel, WorkItem } from './evidenceTypes';

/** 复观测入口的可用性：仅小红书、资格已确认、入口通过同源校验才给按钮；否则写原因，不提供不可执行的按钮。 */
export function reobservationAccess(item: Pick<WorkItem, 'platform'>, channel: Channel): { actionPath: string | null; reason: string } {
  const actionPath = apiPath(channel.url);
  if (item.platform === 'xhs' && channel.eligible && actionPath !== null) return { actionPath, reason: '' };
  return {
    actionPath: null,
    reason: !channel.supported
      ? '这个平台的作品暂时不能复观测。'
      : channel.eligible
        ? '复观测入口地址不对，页面没有提供按钮。'
        : '这篇笔记现在不能复观测：它不在任何仍有效的观察授权里（比如来源的观察目标已停用，或授权已过期）。',
  };
}
