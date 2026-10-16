import { useCallback, useEffect, useRef } from 'react';

/**
 * 写操作的发起上下文（#384 发现：旧回执不得关闭或改写之后为另一个作者打开的抽屉/对话框）。
 *
 * 每个抽屉或对话框实例用自己的上下文发起请求；请求回来时先问一句「发起我的那个上下文还在吗」。
 * 卸载（关闭、换了作者）或 `subject` 变了之后，旧上下文发起的回执一律只做数据刷新，不碰任何界面状态。
 * 服务器上的动作不会因此回滚——这里只决定界面要不要接这个回执。
 */
export function useWriteGuard(subject: string): () => () => boolean {
  const state = useRef({ subject, mounted: false });
  useEffect(() => {
    const issued = { subject, mounted: true };
    state.current = issued;
    return () => {
      issued.mounted = false;
    };
  }, [subject]);
  return useCallback(() => {
    const issued = state.current;
    return () => issued.mounted && state.current === issued;
  }, []);
}
