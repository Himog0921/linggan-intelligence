/** 表格里作者名按钮上的标记（CreatorTable 里的 data-author-key）；返回焦点、恢复位置都靠它找回同一位作者。 */
export const AUTHOR_KEY_ATTRIBUTE = 'data-author-key';

/** 不拼选择器（作者键里有冒号），逐个比对属性值。 */
export function findAuthorButton(creatorKey: string, root: ParentNode = document): HTMLElement | null {
  for (const element of root.querySelectorAll<HTMLElement>(`[${AUTHOR_KEY_ATTRIBUTE}]`)) {
    if (element.getAttribute(AUTHOR_KEY_ATTRIBUTE) === creatorKey) return element;
  }
  return null;
}
