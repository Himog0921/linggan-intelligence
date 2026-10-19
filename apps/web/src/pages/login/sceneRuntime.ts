import sceneUrl from './assets/station.svg?url';
import dataUrl from './assets/scene-data.json?url';
import { stateAt } from './sceneModel';

interface SceneData {
  axes: [number, number][];
  moving: { id: string; motion: string }[];
  effects: { droneOrigin: number[]; channelPoints: number[][]; channelLengths: number[]; channelTotal: number; channelHosts: string[] };
  rotors: { id: string; drone: string; phase: number; size: number }[];
  partIds: string[]; paintStates: number[][]; paintSequence: number[];
}
/** Imperative SVG geometry is decorative; React owns the page and this scoped lifecycle. */
export async function mountScene(host: HTMLDivElement, signal: AbortSignal): Promise<() => void> {
  const fallback = host.querySelector<HTMLDivElement>('.station-fallback')!;
  const mobile = matchMedia('(max-width:800px)'), reduced = matchMedia('(prefers-reduced-motion:reduce)');
  const responses = await Promise.all([fetch(sceneUrl, { signal }), fetch(dataUrl, { signal })]);
  if (responses.some(response => !response.ok)) throw new Error('Scene assets unavailable');
  const [markup, json] = await Promise.all([responses[0]!.text(), responses[1]!.json() as Promise<SceneData>]);
  if (signal.aborted) return () => undefined;
  const template = new DOMParser().parseFromString(markup, 'image/svg+xml').documentElement;
  if (template.localName !== 'svg' || template.querySelector('script, foreignObject, parsererror')) throw new Error('Invalid scene SVG');
  let isMobile = mobile.matches, isReduced = reduced.matches;
  let svg: SVGElement | null = null, frame = 0, elapsed = 0, last = 0, visible = true, disposed = false;
  const [axisX, axisY, axisZ] = json.axes as [[number, number], [number, number], [number, number]];
  const effects = json.effects;
  const translate = (node: SVGElement, x = 0, y = 0, z = 0) => node.setAttribute('transform', `translate(${axisX[0] * x + axisY[0] * y + axisZ[0] * z} ${axisX[1] * x + axisY[1] * y + axisZ[1] * z})`);
  const attr = (node: SVGElement, name: string, value: number) => node.setAttribute(name, String(value));
  const get = (selector: string) => { const node = svg!.querySelector<SVGElement>(selector); if (!node) throw new Error(`Missing scene node ${selector}`); return node; };
  function removeSvg() { svg?.remove(); svg = null; }
  let paintOrder: number[] | undefined;
  let render: (seconds: number) => void = () => undefined;
  function install() {
    svg = document.importNode(template, true) as unknown as SVGElement; host.append(svg);
    const byId = new Map([...svg.querySelectorAll<SVGElement>('[data-part]')].map(node => [node.dataset.part!, node]));
    const parts = get('#station-parts'), moving = json.moving.map(item => ({ ...item, node: byId.get(item.id)! }));
    const scans = [...svg.querySelectorAll<SVGElement>('.read-scan')], core = [...svg.querySelectorAll<SVGElement>('.core-glass')];
    const read = get('.scanner-status'), coreScan = get('#core-scan'), archive = get('.archive-selection');
    const evidence = get('.evidence-selection'), evidenceMark = get('.evidence-mark'), highlight = get('.signal-highlight');
    const point = get('.signal-point'), target = get('.signal-target'), ready = get('.console-ready');
    const ray = get('#drone-ray'), rayOrigin = get('#drone-ray-origin'), observeTarget = get('#observation-target');
    const travels = effects.channelHosts.map((_, i) => get(`#signal-travel-${i}`));
    const rotors = json.rotors.map(rotor => ({ ...rotor, node: get(`[id="${rotor.id}"]`), blades: get(`[id="${rotor.id}-blades"]`) }));
    const input = byId.get('input.card')!;
    render = seconds => {
      const state = stateAt(seconds);
      for (const item of moving) {
        if (item.motion === 'input') { translate(item.node, 0, state.inputY - 291); attr(item.node, 'opacity', state.inputVisible); }
        else if (item.motion === 'slice') translate(item.node, 0, 0, state.slice);
        else if (item.motion === 'result') translate(item.node, 0, state.result);
        else { const q = item.motion === 'drone-small' ? .66 : 1; translate(item.node, state.droneX * q, state.droneY * q, state.droneZ * q); }
      }
      const selected = json.paintStates[json.paintSequence[Math.min(json.paintSequence.length - 1, Math.floor(state.t * 8))]!]!;
      if (selected !== paintOrder) { for (const index of selected) parts.appendChild(byId.get(json.partIds[index]!)!); paintOrder = selected; }
      attr(input, 'opacity', state.inputVisible);
      for (const scan of scans) { attr(scan, 'opacity', state.scan); translate(scan, 0, state.scanY - 145 - (state.inputY - 291)); }
      read.style.opacity = String(.25 + .75 * state.read); for (const node of core) node.style.opacity = String(.22 + .1 * state.core);
      attr(coreScan, 'opacity', state.coreScanVisible); translate(coreScan, 0, 0, state.coreScan);
      archive.style.opacity = String(state.archive); evidence.style.opacity = evidenceMark.style.opacity = String(state.evidence);
      highlight.style.opacity = point.style.opacity = String(state.signal); target.style.opacity = String(.7 * state.signal);
      ready.style.opacity = String(.16 + .84 * state.ready); ray.style.opacity = String(.18 * state.observe); observeTarget.style.opacity = String(.6 * state.observe);
      const rayX = effects.droneOrigin[0]! + .66 * (axisX[0] * state.droneX + axisY[0] * state.droneY + axisZ[0] * state.droneZ);
      const rayY = effects.droneOrigin[1]! + .66 * (axisX[1] * state.droneX + axisY[1] * state.droneY + axisZ[1] * state.droneZ);
      attr(ray, 'x1', rayX); attr(ray, 'y1', rayY); attr(rayOrigin, 'cx', rayX); attr(rayOrigin, 'cy', rayY); rayOrigin.style.opacity = String(.6 * state.observe);
      const distance = state.pulseAlong * effects.channelTotal, lengths = effects.channelLengths;
      let segment = 0; while (segment < lengths.length - 1 && distance > lengths[segment]!) segment++;
      const start = segment ? lengths[segment - 1]! : 0, q = (distance - start) / (lengths[segment]! - start), a = effects.channelPoints[segment]!, b = effects.channelPoints[segment + 1]!;
      travels.forEach((node, i) => { attr(node, 'opacity', i === segment ? state.pulse : 0); if (i === segment) { attr(node, 'cx', a[0]! + (b[0]! - a[0]!) * q); attr(node, 'cy', a[1]! + (b[1]! - a[1]!) * q); } });
      for (const rotor of rotors) {
        const angle = state.rotor + rotor.phase, c = Math.cos(angle), s = Math.sin(angle), q = rotor.size;
        rotor.blades.setAttribute('transform', `matrix(${(axisX[0] * c + axisY[0] * s) * q} ${(axisX[1] * c + axisY[1] * s) * q} ${(-axisX[0] * s + axisY[0] * c) * q} ${(-axisX[1] * s + axisY[1] * c) * q} 0 0)`);
        const f = rotor.drone === 'drone-small' ? .66 : 1; translate(rotor.node, state.droneX * f, state.droneY * f, state.droneZ * f);
      }
      host.dataset.time = state.t.toFixed(3);
    };
    render(isReduced ? 8.4 : elapsed); fallback.hidden = true;
  }
  function stop() { cancelAnimationFrame(frame); frame = 0; last = 0; }
  function tick(now: number) { frame = 0; if (disposed || !svg || isMobile || isReduced || document.hidden || !visible) { last = 0; return; } if (last) elapsed += (now - last) / 1000; last = now; render(elapsed); frame = requestAnimationFrame(tick); }
  function sync() {
    stop(); if (disposed) return;
    if (isMobile) { svg?.remove(); svg = null; paintOrder = undefined; fallback.hidden = false; host.dataset.status = 'mobile'; return; }
    if (!svg) { try { install(); } catch { removeSvg(); fallback.hidden = false; host.dataset.status = 'fallback'; return; } }
    if (isReduced) { render(8.4); host.dataset.status = 'still'; return; }
    host.dataset.status = document.hidden || !visible ? 'paused' : 'running'; if (!document.hidden && visible) frame = requestAnimationFrame(tick);
  }
  function mobileChange(event: MediaQueryListEvent) { isMobile = event.matches; sync(); }
  function reducedChange(event: MediaQueryListEvent) { isReduced = event.matches; sync(); }
  const observer = new IntersectionObserver(entries => { visible = entries[0]?.isIntersecting ?? false; sync(); }, { threshold: 0 });
  function pagehide(event: PageTransitionEvent) { if (event.persisted) { stop(); host.dataset.status = 'paused'; } else destroy(); }
  function pageshow(event: PageTransitionEvent) { if (event.persisted) sync(); }
  function destroy() { if (disposed) return; disposed = true; stop(); observer.disconnect(); mobile.removeEventListener('change', mobileChange); reduced.removeEventListener('change', reducedChange); document.removeEventListener('visibilitychange', sync); window.removeEventListener('pagehide', pagehide); window.removeEventListener('pageshow', pageshow); svg?.remove(); svg = null; fallback.hidden = false; }
  mobile.addEventListener('change', mobileChange); reduced.addEventListener('change', reducedChange); document.addEventListener('visibilitychange', sync); window.addEventListener('pagehide', pagehide); window.addEventListener('pageshow', pageshow); observer.observe(host); frame = requestAnimationFrame(sync);
  return destroy;
}
