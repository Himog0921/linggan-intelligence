import { useCallback, useEffect, useLayoutEffect, useRef, type RefObject } from 'react';

const MAX_RETRY_FRAMES = 120;

/**
 * Remembers the shared inspector body's scroll position per work and tab.
 *
 * The work and records tabs scroll in the inspector's shared `.body`. Switching to the dossier tab turns
 * that body into an overflow-hidden host whose content scrolls in the dossier's own surface, so the browser
 * clamps the shared scrollTop to 0. This hook restores each work/tab's own position when its panel becomes
 * active again; the dossier keeps using useDossierReadingPosition and is intentionally skipped here.
 *
 * A restore can run before the newly selected work's detail has been laid out, when the body is still short
 * enough to clamp the target to 0. In that case it retries for a few frames until the content can hold the
 * position. A wheel, touch, pointer or key interaction cancels the pending restore so it never fights the reader.
 */
export function useInspectorTabScroll(bodyRef: RefObject<HTMLDivElement | null>, workRef: string | null, tab: string, ready: boolean) {
  const positions = useRef(new Map<string, number>());
  const key = workRef ? `${workRef}:${tab}` : null;
  const activeKey = useRef<string | null>(key);
  const readyNow = useRef(ready);
  const tabNow = useRef(tab);
  const pending = useRef<number | null>(null);
  const frame = useRef(0);
  const attempts = useRef(0);

  const stop = useCallback(() => {
    pending.current = null;
    if (frame.current) cancelAnimationFrame(frame.current);
    frame.current = 0;
  }, []);

  const apply = useCallback(function applyPending() {
    frame.current = 0;
    const target = pending.current;
    const body = bodyRef.current;
    if (target === null || !body || tabNow.current === 'dossier') return;
    body.scrollTop = target;
    if (Math.abs(body.scrollTop - target) <= 1) {
      pending.current = null;
      return;
    }
    if (attempts.current++ < MAX_RETRY_FRAMES) frame.current = requestAnimationFrame(applyPending);
    else pending.current = null;
  }, [bodyRef]);

  const schedule = useCallback((target: number) => {
    pending.current = target;
    attempts.current = 0;
    if (frame.current) cancelAnimationFrame(frame.current);
    apply();
  }, [apply]);

  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const remember = () => {
      const current = activeKey.current;
      if (current && readyNow.current && pending.current === null) positions.current.set(current, body.scrollTop);
    };
    const cancel = () => stop();
    body.addEventListener('scroll', remember, { passive: true });
    body.addEventListener('wheel', cancel, { passive: true });
    body.addEventListener('touchstart', cancel, { passive: true });
    body.addEventListener('pointerdown', cancel);
    body.addEventListener('keydown', cancel);
    return () => {
      body.removeEventListener('scroll', remember);
      body.removeEventListener('wheel', cancel);
      body.removeEventListener('touchstart', cancel);
      body.removeEventListener('pointerdown', cancel);
      body.removeEventListener('keydown', cancel);
      stop();
    };
  }, [bodyRef, stop]);

  useLayoutEffect(() => {
    activeKey.current = key;
    readyNow.current = ready;
    tabNow.current = tab;
    if (tab === 'dossier') {
      stop();
      return;
    }
    schedule(key ? positions.current.get(key) ?? 0 : 0);
  }, [key, ready, schedule, stop, tab]);
}
