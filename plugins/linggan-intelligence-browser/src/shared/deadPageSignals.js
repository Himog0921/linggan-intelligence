/**
 * 失效页面 title 关键词表（2026-07-08 归因修复，从 workbench/runtime/capabilityCheck.js
 * 提取为共享常量，避免任务派发阶段和详情采集失败阶段各自维护一份、逐渐漂移）。
 * 词表来源与语义见该文件历史注释：任务目标是某条具体作品，但当前页面 title
 * 显示为死页时命中，用于判定 CONTENT_NOT_FOUND。
 */
export const DEAD_PAGE_TITLE_PATTERN =
  /页面不见了|暂时无法浏览|无法浏览|已删除|已私密|页面不存在|访问的页面|作品不存在|视频不可见|已失效/;

export function looksLikeDeadPageTitle(title = '') {
  return DEAD_PAGE_TITLE_PATTERN.test(String(title || '').trim());
}
