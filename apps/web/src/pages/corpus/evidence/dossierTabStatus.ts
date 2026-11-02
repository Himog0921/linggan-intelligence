import { readingGaps, remainingReadingIssues, resolveReadingSections } from './dossierDocumentModel';
import type { Dossier } from './dossierTypes';
export interface DossierTabStatus { label: string; tone: 'muted' | 'warning' | 'danger'; description: string }
/** Describe the existing query snapshot only; never request data to decorate a tab. */
export function dossierTabStatus(dossier: Dossier | undefined, error: unknown, fetching: boolean): DossierTabStatus | null {
  const sections = dossier ? resolveReadingSections(dossier) : [];
  const gaps = dossier ? readingGaps(dossier) : [];
  const issues = dossier ? remainingReadingIssues(dossier, sections) : [];
  const review = issues.length > 0 || sections.some(section => section.groups.some(group => group.issues.length > 0));
  const facts = [gaps.length ? '有待补内容' : '', review ? '有待核对事项' : ''].filter(Boolean).join('；');
  if (error) return dossier
    ? { label: '刷新失败', tone: 'warning', description: `刷新未完成，保留上次可读版本${facts ? `；${facts}` : ''}。具体原因见正文。` }
    : { label: '不可读', tone: 'danger', description: '当前研究底稿不可读，原因与重试入口见正文。' };
  if (!dossier) return { label: fetching ? '读取中' : '未读取', tone: 'muted', description: fetching ? '正在读取已有底稿，尚无可用结果。' : '研究底稿尚未读取，状态未知。' };
  if (gaps.length || review) return { label: gaps.length && review ? '待补·核对' : gaps.length ? '待补' : '待核对', tone: 'warning', description: `${facts}，具体原因见正文。` };
  if (!sections.length) return { label: '无正文', tone: 'muted', description: '当前范围暂无可读文字。' };
  return null;
}
