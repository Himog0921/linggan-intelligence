import { ApiError } from '../../../shared/api';
import { errorCode, readErrorText } from './evidenceFeedback';

/** 结果区的状态块：读取中、空、失败、地址无效。读取失败如实说，不把失败显示成空列表。 */
export interface Status {
  kind: 'loading' | 'empty' | 'error';
  title: string;
  detail: string;
  /** 合同里的错误码，作旁注。 */
  code: string | null;
  /** 中文说明（按错误码映射）；没有就是 null。 */
  reason: string | null;
  action: 'retry-shell' | 'retry-list' | 'clear-scope' | 'manage-domains' | null;
}

export interface StatusInput {
  shell: { pending: boolean; error: unknown; domainCount: number | null };
  domainSelected: boolean;
  scopeProblem: 'collectionRunRef' | 'creatorFilter' | null;
  list: { pending: boolean; error: unknown; count: number };
}

function shellFailure(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.failure.kind === 'http') return readErrorText(error) ?? '读取暂不可用，请重试。';
    if (error.failure.kind === 'network') return '读取没有得到答复，请检查本机服务后重试。';
    return '服务返回的内容无法识别，页面没有采用它。';
  }
  return '读取暂不可用，请重试。';
}

/** 结果区该显示什么；有作品可显示、不需要状态块时返回 null。 */
export function describeStatus({ shell, domainSelected, scopeProblem, list }: StatusInput): Status | null {
  if (shell.pending) return { kind: 'loading', title: '正在读取领域…', detail: '读取到领域之后才会读取材料。', code: null, reason: null, action: null };
  if (shell.error) return { kind: 'error', title: '读取失败', detail: '领域范围当前没有读到，页面没有把未知结果显示成空列表。', code: errorCode(shell.error), reason: shellFailure(shell.error), action: 'retry-shell' };
  if (!domainSelected) {
    return shell.domainCount === 0
      ? { kind: 'empty', title: '请选择研究领域', detail: '还没有可选择的研究领域。', code: null, reason: null, action: 'manage-domains' }
      : { kind: 'empty', title: '请选择研究领域', detail: '选择一个领域后，页面会读取该领域的统一作品材料，并保留每件作品的主研究／参照角色。', code: null, reason: null, action: null };
  }
  if (scopeProblem) {
    return {
      kind: 'error',
      title: '地址里的筛选条件无效',
      detail: scopeProblem === 'collectionRunRef' ? '地址里的采集运行编号不是有效编号，页面没有拿它去读取。' : '地址里的创作者筛选条件不是有效的筛选内容，页面没有拿它去读取。',
      code: null,
      reason: null,
      action: 'clear-scope',
    };
  }
  if (list.pending) return { kind: 'loading', title: '正在读取本机材料投影', detail: '只读取 Linggan 已接纳的作品级材料；不会触发平台搜索或采集。', code: null, reason: null, action: null };
  if (list.error && list.count === 0) {
    return { kind: 'error', title: '本机材料读取暂时不可用', detail: '当前没有读取任何作品材料；页面不会回退到旧卡片、远程数据库或平台 CDN。', code: errorCode(list.error), reason: readErrorText(list.error), action: 'retry-list' };
  }
  if (list.count === 0) return { kind: 'empty', title: '当前查询没有匹配的作品材料', detail: '读取已经成功；这个结果只描述当前本地查询，不证明平台或现实中没有相关内容。', code: null, reason: null, action: null };
  return null;
}
