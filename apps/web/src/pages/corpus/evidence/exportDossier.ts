import { exportFileName } from './dossierPresent';
import type { DossierPayload } from './dossierTypes';

/**
 * 单篇导出：用户显式触发，经浏览器下载。Markdown 是服务器给的标准底稿；JSON 是服务器给的底稿原样序列化
 * （旧页如此：含 `metadata.authorExternalId`，不增加也不收紧暴露面，DR-9）。不新增批量导出。
 */
export function dossierExport(payload: DossierPayload, kind: 'md' | 'json'): { name: string; type: string; data: string } {
  const work = (payload.dossier.raw as { workRef?: unknown }).workRef;
  return {
    name: exportFileName(typeof work === 'string' ? work : 'dossier', kind),
    type: kind === 'md' ? 'text/markdown;charset=utf-8' : 'application/json;charset=utf-8',
    data: kind === 'md' ? payload.markdown : JSON.stringify(payload.dossier.raw, null, 2),
  };
}

export function downloadDossier(payload: DossierPayload, kind: 'md' | 'json'): void {
  const file = dossierExport(payload, kind);
  const url = URL.createObjectURL(new Blob([file.data], { type: file.type }));
  const link = document.createElement('a');
  link.href = url;
  link.download = file.name;
  link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
