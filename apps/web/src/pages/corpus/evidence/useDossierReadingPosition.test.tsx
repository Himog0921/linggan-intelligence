import { useLayoutEffect, useRef } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { captureReadingPosition, restoreReadingPosition, useDossierReadingPosition } from './useDossierReadingPosition';
import { DossierFocusDialog } from './DossierFocusDialog';
import { parseDossierPayload } from './dossierDecode';
import { resolveReadingSections } from './dossierDocumentModel';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';
function rect(y: number, height: number) { return new DOMRect(0, y, 800, height); }
function geometry(root: HTMLElement, taller = false) {
  Object.defineProperty(root, 'clientHeight', { configurable: true, value: 500 });
  Object.defineProperty(root, 'clientWidth', { configurable: true, value: 800 });
  root.getBoundingClientRect = () => rect(200, 500);
  const anchors = root.querySelectorAll<HTMLElement>('[data-reading-anchor]');
  anchors.forEach((node, index) => { node.getBoundingClientRect = () => rect(200 + (index ? taller ? 800 : 500 : 100) - root.scrollTop, index ? taller ? 1200 : 600 : 300); });
}
function gapGeometry(root: HTMLElement, taller = false) {
  geometry(root);
  root.querySelectorAll<HTMLElement>('[data-reading-anchor]').forEach((node, index) => {
    node.getBoundingClientRect = () => rect(200 + (index ? taller ? 500 : 300 : 100) - root.scrollTop, index ? taller ? 400 : 267 : 200);
  });
}
beforeEach(() => { vi.useFakeTimers(); localStorage.clear(); });
afterEach(() => { vi.useRealTimers(); });
function advance() { act(() => { vi.advanceTimersByTime(300); }); }
function Harness({ memoryKey, revision, taller = false }: { memoryKey: string; revision: string; taller?: boolean }) {
  const root = useRef<HTMLDivElement>(null), toolbar = useRef<HTMLDivElement>(null);
  useDossierReadingPosition(root, toolbar, memoryKey, true, revision);
  useLayoutEffect(() => { if (root.current) geometry(root.current, taller); if (toolbar.current) toolbar.current.getBoundingClientRect = () => rect(200, 55); }, [taller]);
  return <div ref={root} data-testid="reading-scroll"><div ref={toolbar}/><p data-reading-anchor="first">第一段私人正文</p><p data-reading-anchor="second">第二段私人正文</p></div>;
}
it('容器重排后恢复同段内相对位置，锚点撤回时退回已保存的滚动值', () => {
  const root = document.createElement('div'); root.innerHTML = '<p data-reading-anchor="a"></p><p data-reading-anchor="b"></p>';
  geometry(root); root.scrollTop = 854;
  const saved = captureReadingPosition(root, 69);
  expect(saved.anchor).toBe('b'); expect(saved.within).toBeCloseTo(.705);
  geometry(root, true); restoreReadingPosition(root, 69, saved);
  expect(root.scrollTop).toBeCloseTo(1577);
  root.querySelector('[data-reading-anchor="b"]')?.remove(); restoreReadingPosition(root, 69, saved);
  expect(root.scrollTop).toBe(854);
});
it('阅读线位于段尾外 17px 留白时精确恢复，重排保留段尾与留白偏移', () => {
  const root = document.createElement('div'); root.innerHTML = '<p data-reading-anchor="a"></p><p data-reading-anchor="b"></p>';
  gapGeometry(root); root.scrollTop = 570;
  const saved = captureReadingPosition(root, 14);
  expect(saved).toMatchObject({ anchor: 'b', within: 1, offset: 17, top: 570 });
  root.scrollTop = 0; restoreReadingPosition(root, 14, saved); expect(root.scrollTop).toBe(570);
  gapGeometry(root, true); restoreReadingPosition(root, 14, saved); expect(root.scrollTop).toBe(903);
  const end = root.querySelector('[data-reading-anchor="b"]')!.getBoundingClientRect().bottom;
  expect(root.getBoundingClientRect().top + 14 - end).toBe(17);
});
it('相同作品新 revision 不重挂正文或回到顶端，作品之间记忆隔离且存储没有正文', () => {
  const view = render(<Harness memoryKey="position-work-a" revision="r1"/>); advance();
  const root = screen.getByTestId('reading-scroll'); root.scrollTop = 854; fireEvent.scroll(root); advance();
  const data = JSON.parse(localStorage.getItem('position-work-a')!) as Record<string, unknown>;
  expect(Object.keys(data).sort()).toEqual(['anchor','offset','top','within']);
  expect(localStorage.getItem('position-work-a')).not.toContain('私人正文');
  view.rerender(<Harness memoryKey="position-work-a" revision="r2" taller/>); advance();
  expect(screen.getByTestId('reading-scroll')).toBe(root); expect(root.scrollTop).toBeCloseTo(1577);
  view.rerender(<Harness memoryKey="position-work-b" revision="b1"/>); advance(); expect(root.scrollTop).toBe(0);
  view.rerender(<Harness memoryKey="position-work-a" revision="r2" taller/>); advance(); expect(root.scrollTop).toBeCloseTo(1577);
});
it('专注模式卸载后的零矩形不覆盖最后有效位置，重开仍读同段', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', blocks: [{ ref: 'a', kind: 'paragraph', text: '第一段' }, { ref: 'b', kind: 'paragraph', text: '第二段' }] })] })).dossier;
  const groups = resolveReadingSections(dossier)[0]!.groups;
  const props = { title: '合成作品', groups, memoryKey: 'focus-cleanup-test', onClose: vi.fn() };
  const first = render(<DossierFocusDialog {...props}/>);
  const root = screen.getByLabelText('完整口播正文'); gapGeometry(root); advance();
  root.scrollTop = 570; fireEvent.scroll(root);
  first.unmount();
  render(<DossierFocusDialog {...props}/>);
  const reopened = screen.getByLabelText('完整口播正文'); gapGeometry(reopened); advance();
  expect(reopened.scrollTop).toBe(570);
});
