import { useCallback, useEffect, useLayoutEffect, useRef, type RefObject } from 'react';

/**
 * Remembers the shared inspector body's scroll position per work and tab.
 *
 * The work and records tabs scroll in the inspector's shared `.body`. Switching to the dossier tab turns
 * that body into an overflow-hidden host whose content scrolls in the dossier's own surface, so the browser
 * clamps the shared scrollTop to 0. This hook restores each work/tab's own position when its panel becomes
 * active again; the dossier keeps using useDossierReadingPosition and is intentionally skipped here.
 *
 * The selected work's detail can still be loading when the position is restored, so the body may be too
 * short to hold the target and the browser clamps it to 0. The target therefore stays pending until the
 * panel content actually grows enough to hold it: a ResizeObserver on the body and its content retries the
 * restore on every height change. A wheel, touch, pointer or key interaction cancels the pending restore so
 * it never pulls the reader back after they have started reading something else.
 */
export function useInspectorTabScroll(bodyRef: RefObject<HTMLDivElement | null>, workRef: string | null, tab: string, ready: boolean) {
  const positions = useRef(new Map<string, number>());
  const key = workRef ? `${workRef}:${tab}` : null;
  const activeKey = useRef<string | null>(key);
  const readyNow = useRef(ready);
  const tabNow = useRef(tab);
  const pending = useRef<number | null>(null);
  const frame = useRef(0);

  const applyPending = useCallback(() => {
    const target = pending.current;
    const body = bodyRef.current;
    if (target === null || !body || tabNow.current === 'dossier') return;
    body.scrollTop = target;
    if (Math.abs(body.scrollTop - target) <= 1) pending.current = null;
  }, [bodyRef]);

  const stop = useCallback(() => {
    pending.current = null;
    if (frame.current) cancelAnimationFrame(frame.current);
    frame.current = 0;
  }, []);

  const schedule = useCallback((target: number) => {
    pending.current = target;
    if (frame.current) cancelAnimationFrame(frame.current);
    applyPending();
    if (pending.current !== null) {
      frame.current = requestAnimationFrame(function retryAfterLayout() {
        frame.current = 0;
        applyPending();
      });
    }
  }, [applyPending]);

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

  // Re-apply the pending position when the panel content changes height, e.g. when delayed detail data
  // arrives after the tab switch. Observing the children catches the content growing inside the body.
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => applyPending());
    const observeContent = () => {
      if (!observer) return;
      observer.disconnect();
      observer.observe(body);
      for (const child of Array.from(body.children)) observer.observe(child);
    };
    observeContent();
    const mutations = typeof MutationObserver === 'undefined' ? null : new MutationObserver(observeContent);
    mutations?.observe(body, { childList: true });
    return () => {
      mutations?.disconnect();
      observer?.disconnect();
    };
  }, [bodyRef, applyPending]);

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
