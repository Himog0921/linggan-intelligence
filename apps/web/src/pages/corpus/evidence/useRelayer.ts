import { useEffect, useRef, useState } from 'react';
import { relayerDossier } from './dossierApi';
import { relayerFailure, relayerSuccess, type Feedback } from './evidenceFeedback';

interface Options {
  work: string;
  domain: string;
  /** 当前读到的底稿修订；写入带着它，过期就是 409，不会盲目覆盖。 */
  revision: string;
  /** 成功之后：底稿、列表、详情一并重读（封面 OCR 标题、原声摘录可能随分层变化）。 */
  onApplied: () => void;
}

export interface Relayer {
  sending: boolean;
  feedback: Feedback | null;
  run: () => void;
}

/**
 * 重新分层（写）。只由点击触发，不由 Effect 触发（StrictMode 安全）；进行中重复点击只显示进行中；从不自动重试。
 * 结局分三种：已完成（有答复且成功）、未完成（有明确拒绝）、未确认（没有答复或 2xx 看不懂——写入可能已经发生，
 * 所以不拿旧的修订盲目重发，而是先刷新底稿再让用户决定）。
 */
export function useRelayer({ work, domain, revision, onApplied }: Options): Relayer {
  const [sending, setSending] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const inFlight = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  function run() {
    if (inFlight.current) return;
    inFlight.current = true;
    setSending(true);
    setFeedback(null);
    relayerDossier(work, domain, revision).then(
      receipt => {
        if (!mounted.current) return;
        setFeedback(relayerSuccess(receipt));
        onApplied();
      },
      (error: unknown) => {
        if (mounted.current) setFeedback(relayerFailure(error));
      },
    ).finally(() => {
      inFlight.current = false;
      if (mounted.current) setSending(false);
    });
  }

  return { sending, feedback, run };
}
