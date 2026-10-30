/**
 * 选中作者的名单导出。
 *
 * 格式：UTF-8 带 BOM（Excel 直接打开不乱码）、CRLF 换行、每个字段都加双引号（逗号、换行和引号都不破坏列）。
 * 只导出选中的作者；数值未知写「未知」，不写 0。
 */
import { authorName, observedText } from './creatorsPresent';
import { presetDates } from './creatorsQuery';
import type { CreatorItem } from './creatorsTypes';

export const CREATOR_CSV_COLUMNS = ['作者', '发现词', '内容方向', '相关作品', '领域最高赞', '首次发现', '观察状态'] as const;

function cell(value: string | number): string {
  return `"${String(value).replace(/"/g, '""')}"`;
}

/** 生成 CSV 文本。`filtered` 为真时「相关作品」写当前条件下命中的篇数，与表格列一致。 */
export function creatorsCsv(items: readonly CreatorItem[], filtered: boolean): string {
  const rows = [CREATOR_CSV_COLUMNS.map(cell).join(',')];
  for (const item of items) {
    rows.push([
      authorName(item),
      item.discoveryKeywords.join('、') || '未记录发现词',
      item.topicHints.filter(Boolean).join(' · ') || '内容方向尚未形成',
      filtered ? item.matchedRelatedWorkCount : item.relatedWorkCount,
      item.candidateHighLikeCount ?? '未知',
      item.firstSeenDisplay ?? item.firstSeenDate ?? '未知',
      observedText(item.observation),
    ].map(cell).join(','));
  }
  return `\uFEFF${rows.join('\r\n')}\r\n`;
}

/** 触发浏览器下载，返回文件名里的日期（北京时间），方便页面说明导出了哪一天的名单。 */
export function downloadCreatorsCsv(items: readonly CreatorItem[], filtered: boolean, now = new Date()): string {
  const stamp = presetDates(1, now).from;
  const blob = new Blob([creatorsCsv(items, filtered)], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `创作者名单-${stamp}.csv`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1_000);
  return stamp;
}
