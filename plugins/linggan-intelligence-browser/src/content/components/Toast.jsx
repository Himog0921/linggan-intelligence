import React, { useEffect } from 'react';
import { createRoot } from 'react-dom/client';
import { getCurrentTheme } from '../../themes/themeManager.js';
import { icon } from '../../shared/icons.js';
import { getFeedbackMeta } from '../../shared/feedback.js';

function buildToastStyle(type = 'info') {
  const theme = getCurrentTheme();
  const isAc = theme === 'ac-ui';
  const toneMap = isAc
    ? { info: '#e8f4fd', success: '#e8f5e8', warning: '#fff8e0', error: '#fde8e8' }
    : { info: '#d8f4ff', success: '#d4f5d3', warning: '#fff0a8', error: '#ffd4d4' };
  const ink = isAc ? '#4f4f4f' : '#121212';
  const line = isAc ? '#ddd' : '#121212';
  const radius = isAc ? '5px' : '10px';
  const shadow = isAc ? '0 2px 8px rgba(0,0,0,0.08)' : `4px 4px 0 ${line}`;
  const border = isAc ? `1px solid ${line}` : `2px solid ${line}`;
  const fontFamily = isAc
    ? "'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif"
    : "'Arial Black','Segoe UI',sans-serif";

  return {
    position: 'fixed',
    top: '20px',
    left: '50%',
    transform: 'translateX(-50%)',
    width: '320px',
    minHeight: '52px',
    boxSizing: 'border-box',
    padding: '10px 16px',
    display: 'flex',
    alignItems: 'flex-start',
    justifyContent: 'flex-start',
    gap: '10px',
    background: toneMap[type] || toneMap.info,
    color: ink,
    border,
    borderRadius: radius,
    boxShadow: shadow,
    fontSize: '13px',
    fontWeight: isAc ? '700' : '900',
    lineHeight: '1.35',
    zIndex: '2147483646',
    transition: 'opacity 0.3s',
    fontFamily,
    whiteSpace: 'normal',
  };
}

function Toast({ message, type = 'info', onDismiss }) {
  const meta = getFeedbackMeta(type);
  useEffect(() => {
    const timer = setTimeout(() => onDismiss?.(), 3000);
    return () => clearTimeout(timer);
  }, [onDismiss]);

  return (
    <div
      style={buildToastStyle(type)}
      role={type === 'error' ? 'alert' : 'status'}
      aria-live={type === 'error' ? 'assertive' : 'polite'}
      aria-atomic="true"
    >
      <span
        style={{ display: 'inline-flex', flex: '0 0 auto', marginTop: '1px' }}
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: icon(meta.icon, { size: 16 }) }}
      />
      <span style={{ display: 'block', minWidth: 0, flex: 1 }}>
        <strong style={{ display: 'block', fontSize: '11px', marginBottom: '1px' }}>{meta.title}</strong>
        <span>{message}</span>
      </span>
    </div>
  );
}

const toastRoots = new Map();

function unmountExistingToasts() {
  toastRoots.forEach((root, container) => {
    try { root.unmount(); } catch { /* ignore */ }
    container.remove();
  });
  toastRoots.clear();
}

export function showToast(message, type = 'info') {
  unmountExistingToasts();

  const container = document.createElement('div');
  container.className = 'lgboom-toast';
  document.body.appendChild(container);

  const root = createRoot(container);
  toastRoots.set(container, root);

  const dismiss = () => {
    if (!container.parentNode) return;
    container.style.opacity = '0';
    setTimeout(() => {
      toastRoots.delete(container);
      try { root.unmount(); } catch { /* ignore */ }
      container.remove();
    }, 300);
  };

  root.render(<Toast message={message} type={type} onDismiss={dismiss} />);
}

export function showDouyinToast(message, type = 'info') {
  unmountExistingToasts();

  const container = document.createElement('div');
  container.className = 'lgboom-dy-toast';
  document.body.appendChild(container);

  const root = createRoot(container);
  toastRoots.set(container, root);

  const dismiss = () => {
    if (!container.parentNode) return;
    container.style.opacity = '0';
    setTimeout(() => {
      toastRoots.delete(container);
      try { root.unmount(); } catch { /* ignore */ }
      container.remove();
    }, 300);
  };

  root.render(<Toast message={message} type={type} onDismiss={dismiss} />);
}
