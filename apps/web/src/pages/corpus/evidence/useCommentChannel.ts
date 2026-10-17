import { useEffect, useRef, useState } from 'react';
import { fetchCommentPage } from './evidenceApi';
import type { Comment } from './evidenceTypes';

export interface CommentChannel {
  phase: 'idle' | 'loading' | 'loaded' | 'error';
  items: readonly Comment[];
  cursor: string | null;
  total: number | null;
  returned: number | null;
  truncated: boolean;
  /** 读取失败时的原始错误，由展示层翻译。 */
  error: unknown;
  /** `append` 为真时用上一次回执里的游标续读；否则从头读。只由用户点击触发，不自动读取。 */
  load: (append: boolean) => void;
}

interface ChannelState {
  phase: CommentChannel['phase'];
  items: readonly Comment[];
  cursor: string | null;
  total: number | null;
  returned: number | null;
  truncated: boolean;
  error: unknown;
}

const IDLE: ChannelState = { phase: 'idle', items: [], cursor: null, total: null, returned: null, truncated: false, error: null };

/**
 * 评论原文只经本机授权通道、用户点击后按页读取，且只在内存里：换作品（组件按作品重建）或卸载即清空并取消在途请求，
 * 不进地址、不进浏览器存储（I11）。
 */
export function useCommentChannel(channelUrl: string | null): CommentChannel {
  const [state, setState] = useState<ChannelState>(IDLE);
  const controller = useRef<AbortController | null>(null);

  useEffect(() => {
    return () => controller.current?.abort();
  }, []);

  function load(append: boolean) {
    if (channelUrl === null) return;
    controller.current?.abort();
    const abort = new AbortController();
    controller.current = abort;
    const cursor = append ? state.cursor : null;
    setState(previous => ({ ...previous, phase: 'loading', error: null }));
    fetchCommentPage(channelUrl, cursor, abort.signal).then(
      page => {
        if (abort.signal.aborted) return;
        setState(previous => ({
          phase: 'loaded',
          items: append ? [...previous.items, ...page.items] : page.items,
          cursor: page.nextCursor,
          total: page.total,
          returned: page.returned,
          truncated: page.truncated,
          error: null,
        }));
      },
      (error: unknown) => {
        if (abort.signal.aborted) return;
        setState(previous => ({ ...previous, phase: 'error', error }));
      },
    );
  }

  return { ...state, load };
}
