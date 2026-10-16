import { apiPath } from './evidencePaths';
import type { Reobservation as Channel, WorkItem } from './evidenceTypes';

/** 复观测入口的可用性：仅小红书、资格已确认、入口通过同源校验才给按钮；否则写原因，不提供不可执行的按钮。 */
export function reobservationAccess(item: Pick<WorkItem, 'platform'>, channel: Channel): { actionPath: string | null; reason: string } {
  const actionPath = apiPath(channel.url);
  if (item.platform === 'xhs' && channel.eligible && actionPath !== null) return { actionPath, reason: '' };
  return {
    actionPath: null,
    reason: !channel.supported
      ? '该作品的平台当前不支持复观测；页面没有创建可调用入口。'
      : channel.eligible
        ? '复观测入口未通过同源路径校验；页面不会猜测后端路由。'
        : '该作品当前尚未具备监控目标关联的有效深度归档授权；页面不会提供不可执行的操作。',
  };
}
