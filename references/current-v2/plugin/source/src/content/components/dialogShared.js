import { createRoot } from 'react-dom/client';

const dialogRoots = new Map();

export function mountDialog(containerClass, component) {
  document.querySelector(`.${containerClass}`)?.remove();

  const container = document.createElement('div');
  container.className = containerClass;
  document.body.appendChild(container);

  const root = createRoot(container);
  dialogRoots.set(container, root);

  root.render(component);

  return { container, root };
}

export function unmountDialog(container) {
  const root = dialogRoots.get(container);
  if (root) {
    try { root.unmount(); } catch { /* ignore */ }
    dialogRoots.delete(container);
  }
  container.remove();
}

export const NEO_OVERLAY_STYLE = {
  position: 'fixed',
  inset: 0,
  background: 'rgba(0,0,0,0.45)',
  zIndex: '2147483647',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
};

export const NEO_PANEL_STYLE = {
  background: '#fff5c4',
  borderRadius: '16px',
  padding: '30px 34px',
  boxShadow: '6px 6px 0 #121212',
  maxWidth: '460px',
  width: '90%',
  border: '3px solid #121212',
  fontFamily: "'Arial Black','Segoe UI',sans-serif",
};
