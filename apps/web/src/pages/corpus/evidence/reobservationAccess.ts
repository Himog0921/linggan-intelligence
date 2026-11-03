import { apiPath } from './evidencePaths';
import type { Reobservation as Channel, WorkItem } from './evidenceTypes';

/**
 * 复观测入口的可用性：仅小红书、资格已确认、入口通过同源校验才给按钮；否则写原因，不提供不可执行的按钮。
 * 不合格原因只按接口明确给出的理由映射：没有有效授权是已知事实；缺省／未识别的理由保守写「资格待确认」或
 * 「暂不可用」，不推断授权已过期或观察目标已停用。原始原因码只留在技术事实里。
 */
export function reobservationAccess(item: Pick<WorkItem, 'platform'>, channel: Channel): { actionPath: string | null; reason: string } {
  if (item.platform !== 'xhs' || !channel.supported) {
    return { actionPath: null, reason: '暂不支持这个平台。' };
  }
  const actionPath = apiPath(channel.url);
  if (channel.eligible && actionPath !== null) return { actionPath, reason: '' };
  if (channel.eligible) return { actionPath: null, reason: '入口地址异常，页面没有提供按钮。' };
  if (channel.reason === 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED') {
    return { actionPath: null, reason: '没有有效的观察授权。' };
  }
  if (channel.reason === 'REOBSERVATION_ELIGIBILITY_UNKNOWN' || channel.reason === '') {
    return { actionPath: null, reason: '资格待确认。' };
  }
  return { actionPath: null, reason: '暂不可用。' };
}
