import type { DossierMedia } from './dossierTypes';

/** 研究底稿自带的三张词表（片段栏目名、处理状态、识别行角色），以及固定文案。 */
const FIELD_NAMES: Readonly<Record<string, string>> = {
  title: '原标题',
  body: '正文',
  cover_headline: '图片大字',
  ocr: '图片正文',
  transcript: '视频转写',
  native_title: '平台标题',
  image_substantive_text: '图片正文',
  asr_text: '视频转写',
  frame_ocr_text: '视频画面文字',
};

const PROCESSING_STATES: Readonly<Record<string, string>> = {
  ACCEPTED: '已整理',
  PARTIAL: '部分待确认',
  NEEDS_REVIEW: '待确认',
  FAILED: '处理失败',
  ACQUIRED: '已取得',
  QUEUED: '排队中',
  PROCESSING: '处理中',
  KNOWN_EMPTY: '本次未识别到文字',
  UNKNOWN: '尚未知晓',
  NOT_ENABLED: '未启用',
  WITHDRAWN_OR_RESTRICTED: '材料受限',
  RETIRED: '旧识别已停用',
};

const LINE_ROLES: Readonly<Record<string, string>> = {
  primary_copy: '图片大字',
  substantive_text: '图片正文',
  uncertain: '待确认',
  platform_watermark: '平台水印',
  platform_ui: '平台界面文字',
  low_confidence: '识别不清',
  incidental_scene_text: '场景文字',
};

const COVERAGE_LABELS: Readonly<Record<string, string>> = { KNOWN: '已取得', KNOWN_EMPTY: '已确认没有', UNKNOWN: '未知', UNAVAILABLE: '暂不可用' };

export function fieldName(field: string): string {
  return FIELD_NAMES[field] ?? '材料文字';
}

export function processingState(state: string): string {
  return PROCESSING_STATES[state] ?? state;
}

export function lineRole(role: string): string {
  return LINE_ROLES[role] ?? '待确认';
}

export function coverageLabel(value: string): string {
  return COVERAGE_LABELS[value] ?? value;
}

/** 空栏各写自己的说明——它们不是同一种「没有」。 */
export function sectionEmptyNote(kind: string): string {
  if (kind === 'comments') return '评论在下方单独读取。';
  if (kind === 'background') return '待确认行保留在原图对照中；当前规则不猜测照片中文件的类型。';
  if (kind === 'tags') return '尚无独立标签记录；正文中的标签保留在原文中。';
  return '当前没有可用内容；是否原本存在尚未确认。';
}

export const TASK_OPTIONS = [
  ['creator', '创作者发现 · 单篇材料'],
  ['comments', '评论研究 · 作品上下文'],
  ['topics', '主题研究 · 分窗前材料'],
] as const;

export function taskNote(key: 'creator' | 'comments' | 'topics'): string {
  if (key === 'creator') return '与任务共用单篇材料选择器，最多 4,000 字符。正式请求还会加入领域与提示词，并按模型预算缩短。';
  if (key === 'comments') return '这是任务共用的作品上下文，最多 20,000 字符。研究对象仍为评论，评论与父回复由原有研究通道读取。';
  return '这里保留该作品的完整可用材料；正式主题研究继续按原有预算分窗，并可能包含其他作品和评论。';
}

/** 底稿里「原图与识别对照」要列的图：非仅上下文，且已分层或属于识图处理器。 */
export function comparableImages(media: readonly DossierMedia[]): DossierMedia[] {
  return media.filter(item => !item.contextOnly && (item.layering !== null || item.processorKind === 'image_ocr' || item.processorKind === 'video_frame_ocr'));
}

export function imageSummary(media: DossierMedia): string {
  const state = processingState(media.layering?.state ?? media.state);
  return `图片 ${media.slotOrdinal ?? '—'} · ${state} · ${media.layering?.layerVersion ?? '尚未分层'}`;
}

export function exportFileName(work: string, kind: 'md' | 'json'): string {
  return `研究底稿-${work}.${kind}`;
}

/** 把已读到的评论排成「主评论下挂回复」；父评论还没读到的回复单独标出，不拼接其内容。 */
export interface CommentNode<T extends { sourceRef: string; parentSourceRef: string | null; relation: 'ROOT' | 'REPLY' }> {
  item: T;
  replies: T[];
  orphan: boolean;
}

export function nestComments<T extends { sourceRef: string; parentSourceRef: string | null; relation: 'ROOT' | 'REPLY' }>(items: readonly T[]): CommentNode<T>[] {
  const bySource = new Map(items.map(item => [item.sourceRef, item] as const));
  const nodes: CommentNode<T>[] = [];
  const roots = new Map<string, CommentNode<T>>();
  for (const item of items) {
    if (item.relation === 'ROOT') {
      const node: CommentNode<T> = { item, replies: [], orphan: false };
      roots.set(item.sourceRef, node);
      nodes.push(node);
    }
  }
  for (const item of items) {
    if (item.relation !== 'REPLY') continue;
    const parent = item.parentSourceRef === null ? undefined : bySource.get(item.parentSourceRef);
    const root = parent?.relation === 'ROOT' ? roots.get(parent.sourceRef) : undefined;
    if (root) root.replies.push(item);
    else nodes.push({ item, replies: [], orphan: parent === undefined });
  }
  return nodes;
}
