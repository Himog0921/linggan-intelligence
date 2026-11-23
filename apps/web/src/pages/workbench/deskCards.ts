import { GROUPS, type CanvasBlock, type CanvasGroup, type WorkDocument } from './workbenchModel';

/** Desk cards of one work, shared by the material desk and the writing sidebar so both show the same cards. */
export const kindLabel: Record<string, string> = {
  note: '原笔记', comment: '评论', ocr: '图片文字', transcript: '口播', frame_text: '画面文字', ops_publication: '发布记录',
};
export const kindDot: Record<string, string> = {
  note: 'warn', comment: 'info', ocr: 'ok', transcript: 'ok', frame_text: 'ok', ops_publication: 'info',
};
export const groupLabel: Record<string, string> = Object.fromEntries(GROUPS.map(item => [item.id, item.label]));
/** Uses that do not go into the text (or are not decided yet) are drawn grey, as in the sample. */
export const softRole = (group: CanvasGroup) => group === 'inbox' || group === 'reference_only' || group === 'counterexample';

export interface Card {
  id: string;
  note: boolean;
  kind: string;
  dot: string;
  text: string;
  expression: string;
  group: CanvasGroup;
  role: string;
  memo: string;
  excerptId: string | null;
  /** False when the cited source is restricted or no longer readable. */
  readable: boolean;
  meta: string;
  x: number;
  y: number;
}

export function cardsOf(document: WorkDocument | undefined): Card[] {
  if (!document) return [];
  return document.canvas.map((block: CanvasBlock) => {
    const excerpt = block.excerptId ? document.excerpts.find(item => item.id === block.excerptId) : undefined;
    const role = groupLabel[block.group] ?? '待分';
    if (excerpt) return {
      id: block.id,
      note: false,
      kind: kindLabel[excerpt.citation.sourceKind] ?? '材料',
      dot: kindDot[excerpt.citation.sourceKind] ?? 'info',
      text: excerpt.selectedText ?? '来源已受限或不可用',
      expression: '',
      group: block.group,
      role,
      memo: block.note.trim(),
      excerptId: excerpt.id,
      readable: excerpt.selectedText !== null,
      meta: `${kindLabel[excerpt.citation.sourceKind] ?? '来源'} · 采集于 ${new Date(excerpt.citation.asOf).toLocaleDateString('zh-CN')}`,
      x: 0,
      y: 0,
    };
    return {
      id: block.id,
      note: true,
      kind: '便签',
      dot: 'warn',
      text: block.expression.trim() || block.note.trim() || '（空便签）',
      expression: block.expression,
      group: block.group,
      role,
      memo: block.note.trim(),
      excerptId: null,
      readable: true,
      meta: '便签',
      x: 0,
      y: 0,
    };
  });
}
