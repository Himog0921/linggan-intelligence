import type { OpenedIdeaDrafts } from './workbenchModel';

/** What the server did, in the words of the desk: who got a new draft, who already had one, what was left out. */
export function openedText(opened: OpenedIdeaDrafts, nameOf: (ref: string) => string): string {
  const made = opened.drafts.filter(draft => draft.created).map(draft => nameOf(draft.accountRef));
  const kept = opened.drafts.filter(draft => !draft.created).map(draft => nameOf(draft.accountRef));
  const parts = [made.length ? `已开稿 ${made.length} 篇：${made.join('、')}` : '选中的账号都开过稿了，没有再开'];
  if (made.length && kept.length) parts.push(`${kept.join('、')}原来就有稿，没有重复开`);
  if (opened.replayed) parts.push('这些稿是上一次同一个请求已经开好的');
  if (opened.skippedExcerpts) parts.push(`${opened.skippedExcerpts} 段来源现在读不到，没有放进新稿`);
  return parts.join('；');
}
