import { requestJson } from '../shared/api';
import { decode, list, num, optNum, optStr, str } from '../shared/decode';

/**
 * 语料外壳的只读合同：`GET /api/local/corpus/shell?domain=…`（`apps/api/src/local_web/corpus_shell_api.rs`）。
 * 领域选择器要显示的字段，和一级导航「采集」项的状态词，都以服务器为准。
 */
export interface ShellDomain {
  domainRef: string;
  name: string;
  /** `active`（运行中）或 `paused`（已暂停）。 */
  status: string;
  targetCount: number;
  /** 领域内去重作品数；读不到是 null，不是 0。 */
  sampleCount: number | null;
}

export interface CorpusShell {
  domains: ShellDomain[];
  /** 服务器按地址里的 domain 解析出的领域；无效或未选时为 null，不会替用户挑一个。 */
  selectedDomain: string | null;
  /** 观察中 / 无观察目标 / 状态未知：读不到时保持「状态未知」，不宣称已接通或为空。 */
  collectionState: string;
}

export function parseCorpusShell(value: unknown): CorpusShell {
  return decode(value, root => ({
    domains: list(root, 'domains', row => ({
      domainRef: str(row, 'domainRef'),
      name: str(row, 'name'),
      status: str(row, 'status'),
      targetCount: num(row, 'targetCount'),
      sampleCount: optNum(row, 'sampleCount'),
    })),
    selectedDomain: optStr(root, 'selectedDomain'),
    collectionState: str(root, 'collectionState'),
  }));
}

export async function fetchCorpusShell(domain: string | null, signal?: AbortSignal): Promise<CorpusShell> {
  const query = domain ? `?domain=${encodeURIComponent(domain)}` : '';
  return parseCorpusShell(await requestJson(`/api/local/corpus/shell${query}`, signal ? { signal } : {}));
}

/** 与 `render_domain_picker` 的 meta 完全一致：暂停显示「已暂停」，否则「N 个目标 · M 篇作品」，作品数未知写「未知」。 */
export function domainMeta(domain: ShellDomain): string {
  if (domain.status === 'paused') return '已暂停';
  const works = domain.sampleCount === null ? '未知' : String(domain.sampleCount);
  return `${domain.targetCount} 个目标 · ${works} 篇作品`;
}
