import { ApiError } from '../../shared/api';
interface Domain { domainRef: string; name: string; status: string; targetCount: number; sampleCount: number | null }
interface Shell { domains: Domain[] | null; domainsReadable: boolean; selectedDomain: string | null; totalTargets: number | null; monitoringTargets: number | null; scheduler: 'running' | 'stale' | 'unknown' }
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value as Record<string, unknown>;
}
function text(value: unknown): string { if (typeof value !== 'string') throw new Error('采集外壳字段无法解析'); return value; }
function count(value: unknown): number | null { if (value === null) return null; if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error('采集外壳读数无法解析'); return value; }
export function parseCollectionShell(value: unknown): Shell {
  const v = object(value);
  if (typeof v.domainsReadable !== 'boolean' || !['running', 'stale', 'unknown'].includes(String(v.scheduler))) throw new Error('采集外壳状态无法解析');
  const domains = v.domains === null ? null : Array.isArray(v.domains) ? v.domains.map(value => {
    const d = object(value); const targetCount = count(d.targetCount); if (targetCount === null) throw new Error('领域目标数无法解析');
    return { domainRef: text(d.domainRef), name: text(d.name), status: text(d.status), targetCount, sampleCount: count(d.sampleCount) };
  }) : (() => { throw new Error('领域列表无法解析'); })();
  return { domains, domainsReadable: v.domainsReadable, selectedDomain: v.selectedDomain === null ? null : text(v.selectedDomain), totalTargets: count(v.totalTargets), monitoringTargets: count(v.monitoringTargets), scheduler: v.scheduler as Shell['scheduler'] };
}
