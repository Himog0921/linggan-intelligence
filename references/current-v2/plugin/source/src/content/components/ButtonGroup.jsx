import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { icon } from '../../shared/icons.js';
import { BRAND_ASSETS, getBrandAssetUrl } from '../../shared/brandAssets.js';
import { getCurrentTheme } from '../../themes/themeManager.js';
import { DEFAULT_TOKENS } from '../../shared/taskUi.js';
import { AC_COLORS } from '../../themes/ac-ui/tokens.js';

const INJECT_LOGO_SRC = getBrandAssetUrl(BRAND_ASSETS.injectLogo);
const INJECT_BANNER_SRC = getBrandAssetUrl(BRAND_ASSETS.banner);
const FLOATING_POSITION_PREFIX = 'lgbbb.content.float';
const FLOATING_VIEWPORT_GAP = 12;

function getFloatingPositionStorageKey(floatingKey = '') {
  return floatingKey ? `${FLOATING_POSITION_PREFIX}.${floatingKey}` : '';
}

async function readFloatingPosition(floatingKey = '') {
  const storageKey = getFloatingPositionStorageKey(floatingKey);
  if (!storageKey) return null;

  try {
    const result = await chrome.storage?.local?.get(storageKey);
    if (result?.[storageKey]) return result[storageKey];
  } catch {
    // ignore extension storage read failure
  }

  try {
    const raw = window.localStorage.getItem(storageKey);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

async function writeFloatingPosition(floatingKey = '', value = null) {
  const storageKey = getFloatingPositionStorageKey(floatingKey);
  if (!storageKey || !value) return;

  try {
    await chrome.storage?.local?.set({ [storageKey]: value });
    return;
  } catch {
    // ignore extension storage write failure
  }

  try {
    window.localStorage.setItem(storageKey, JSON.stringify(value));
  } catch {
    // ignore UX-only storage failure
  }
}

function clampFloatingPosition(left = 0, top = 0, width = 0, height = 0) {
  const maxLeft = Math.max(FLOATING_VIEWPORT_GAP, window.innerWidth - width - FLOATING_VIEWPORT_GAP);
  const maxTop = Math.max(FLOATING_VIEWPORT_GAP, window.innerHeight - height - FLOATING_VIEWPORT_GAP);

  return {
    left: Math.min(Math.max(FLOATING_VIEWPORT_GAP, left), maxLeft),
    top: Math.min(Math.max(FLOATING_VIEWPORT_GAP, top), maxTop),
  };
}

function applyFloatingPosition(host, position = null) {
  if (!host || !position) return;

  const rect = host.getBoundingClientRect();
  const width = rect.width || host.offsetWidth || 0;
  const height = rect.height || host.offsetHeight || 0;
  const next = clampFloatingPosition(Number(position.left || rect.left), Number(position.top || rect.top), width, height);

  host.style.left = `${next.left}px`;
  host.style.top = `${next.top}px`;
  host.style.right = 'auto';
  host.style.bottom = 'auto';
}

function ButtonGroup({
  buttons = [],
  platform = 'xhs',
  compact = false,
  brandVariant = 'logo',
  floatingKey = '',
  containerStyle = {},
  brandStyle = {},
  buttonStyle = {},
}) {
  const [logoFailed, setLogoFailed] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [pendingAction, setPendingAction] = useState('');
  const groupRef = useRef(null);
  const dragStateRef = useRef(null);
  const pendingTimerRef = useRef(null);
  const theme = getCurrentTheme();
  const isAc = theme === 'ac-ui';
  const isDy = platform === 'douyin';
  const btnPrefix = isDy ? 'lgboom-dy-btn' : 'lgboom-btn';
  const isBannerBrand = brandVariant === 'banner';
  const isFloating = Boolean(floatingKey);
  const logoHeight = isBannerBrand ? (compact ? 28 : 34) : (compact ? 44 : 54);
  const bannerShellStyle = isBannerBrand
    ? {
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: compact ? '4px 6px' : '5px 6px',
        border: `3px solid ${DEFAULT_TOKENS.line}`,
        borderRadius: compact ? '14px' : '16px',
        background: '#fff8d6',
        lineHeight: 0,
      }
    : null;

  const groupBase = isAc
    ? {
        display: 'flex',
        gap: compact ? '6px' : '8px',
        padding: compact ? '10px 12px' : '12px 16px',
        background: '#f2f2f2',
        borderRadius: '5px',
        border: '1px solid #ddd',
        alignItems: 'center',
        boxShadow: '0 2px 8px rgba(0,0,0,0.06)',
      }
    : {
        display: 'flex',
        gap: compact ? '6px' : '8px',
        padding: compact ? '10px 12px' : '12px 16px',
        background: DEFAULT_TOKENS.surface,
        borderRadius: '12px',
        border: `3px solid ${DEFAULT_TOKENS.line}`,
        alignItems: 'center',
        boxShadow: compact ? '3px 3px 0 #121212' : '5px 5px 0 #121212',
      };

  const brandBase = isAc
    ? {
        fontWeight: 700,
        fontSize: compact ? 13 : 14,
        color: '#4f4f4f',
        fontFamily: "'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif",
        marginRight: compact ? 6 : isDy ? 0 : 12,
      }
    : {
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'flex-start',
        flex: '0 0 auto',
        padding: '1px 0',
        marginRight: compact ? 8 : isDy ? 0 : 10,
      };

  useEffect(() => () => {
    if (pendingTimerRef.current) clearTimeout(pendingTimerRef.current);
  }, []);

  useEffect(() => {
    if (!isFloating) return undefined;

    let cancelled = false;
    const host = groupRef.current?.parentElement;
    if (!host) return undefined;

    void readFloatingPosition(floatingKey).then((position) => {
      if (cancelled || !position) return;
      applyFloatingPosition(host, position);
    });

    const handleResize = () => {
      const currentHost = groupRef.current?.parentElement;
      if (!currentHost) return;
      applyFloatingPosition(currentHost, {
        left: currentHost.getBoundingClientRect().left,
        top: currentHost.getBoundingClientRect().top,
      });
    };

    window.addEventListener('resize', handleResize);
    return () => {
      cancelled = true;
      window.removeEventListener('resize', handleResize);
    };
  }, [floatingKey, isFloating]);

  useEffect(() => {
    if (!isFloating) return undefined;

    const handlePointerMove = (event) => {
      const state = dragStateRef.current;
      if (!state?.host) return;

      const width = state.host.offsetWidth || state.host.getBoundingClientRect().width || 0;
      const height = state.host.offsetHeight || state.host.getBoundingClientRect().height || 0;
      const next = clampFloatingPosition(event.clientX - state.offsetX, event.clientY - state.offsetY, width, height);

      state.host.style.left = `${next.left}px`;
      state.host.style.top = `${next.top}px`;
      state.host.style.right = 'auto';
      state.host.style.bottom = 'auto';
    };

    const finishDrag = () => {
      const state = dragStateRef.current;
      if (!state?.host) return;

      dragStateRef.current = null;
      setIsDragging(false);
      void writeFloatingPosition(floatingKey, {
        left: Math.round(state.host.getBoundingClientRect().left),
        top: Math.round(state.host.getBoundingClientRect().top),
      });
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', finishDrag);
    window.addEventListener('pointercancel', finishDrag);
    return () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', finishDrag);
      window.removeEventListener('pointercancel', finishDrag);
    };
  }, [floatingKey, isFloating]);

  const acknowledgeAction = (action) => {
    if (!action) return;
    if (pendingTimerRef.current) clearTimeout(pendingTimerRef.current);
    setPendingAction(action);
    pendingTimerRef.current = setTimeout(() => {
      setPendingAction('');
      pendingTimerRef.current = null;
    }, 900);
  };

  const startFloatingDrag = (event) => {
    if (!isFloating || event.button !== 0) return;

    const host = groupRef.current?.parentElement;
    if (!host) return;

    const rect = host.getBoundingClientRect();
    host.style.left = `${rect.left}px`;
    host.style.top = `${rect.top}px`;
    host.style.right = 'auto';
    host.style.bottom = 'auto';

    dragStateRef.current = {
      host,
      offsetX: event.clientX - rect.left,
      offsetY: event.clientY - rect.top,
    };

    setIsDragging(true);
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  };

  return (
    <div
      ref={groupRef}
      className={isDy ? 'lgboom-dy-btn-group' : 'lgboom-btn-group'}
      style={{ ...groupBase, ...containerStyle }}
    >
      <div
        onPointerDown={startFloatingDrag}
        style={{
          ...brandBase,
          ...(isFloating
            ? {
                cursor: isDragging ? 'grabbing' : 'grab',
                touchAction: 'none',
                userSelect: 'none',
              }
            : null),
          ...brandStyle,
        }}
      >
        {isAc ? (
          isDy ? (
            <>
              <span style={{ display: 'block', marginBottom: '2px', fontSize: '11px', fontWeight: 700 }}>
                灵感爆爆爆
              </span>
              <span style={{ fontSize: '9px', color: '#888', fontFamily: 'sans-serif', fontWeight: 400 }}>
                抖音
              </span>
            </>
          ) : (
            '灵感爆爆爆'
          )
        ) : (
          logoFailed ? (
            <span
              style={{
                display: 'inline-block',
                alignItems: 'center',
                justifyContent: 'center',
                color: '#121212',
                fontSize: compact ? '10px' : '11px',
                fontWeight: 900,
                lineHeight: 1,
                textAlign: 'center',
                fontFamily: "'Arial Black','Segoe UI',sans-serif",
                userSelect: 'none',
              }}
            >
              LG BOOM
            </span>
          ) : isBannerBrand ? (
            <span
              style={{
                ...bannerShellStyle,
              }}
            >
              <img
                src={INJECT_BANNER_SRC}
                alt="灵感爆爆爆"
                draggable="false"
                onError={() => setLogoFailed(true)}
                style={{
                  display: 'block',
                  width: 'auto',
                  height: `${logoHeight}px`,
                  userSelect: 'none',
                  pointerEvents: 'none',
                }}
              />
            </span>
          ) : (
            <span
              style={{
                display: 'block',
                borderRadius: compact ? '10px' : '12px',
                overflow: 'hidden',
                boxShadow: '2px 2px 0 #121212',
                lineHeight: 0,
              }}
            >
              <img
                src={INJECT_LOGO_SRC}
                alt="灵感爆爆爆"
                draggable="false"
                onError={() => setLogoFailed(true)}
                style={{
                  display: 'block',
                  width: 'auto',
                  height: `${logoHeight}px`,
                  borderRadius: compact ? '10px' : '12px',
                  userSelect: 'none',
                  pointerEvents: 'none',
                }}
              />
            </span>
          )
        )}
      </div>
      {buttons.map(({ text, action, style = 'secondary', data, icon: iconName, hidden }) => {
        if (hidden) return null;
        const btnClass = `${btnPrefix} ${btnPrefix}-${style}`;
        const isPending = pendingAction === action;

        if (isAc) {
          const cm = AC_COLORS[style] || AC_COLORS.secondary;
          return (
            <button
              key={action}
              className={btnClass}
              data-action={action}
              data-params={data ? JSON.stringify(data) : undefined}
              style={{
                border: `1px solid ${cm.border}`,
                borderRadius: '5px',
                padding: isDy ? '8px 14px' : '8px 16px',
                fontSize: isDy ? '12px' : '13px',
                fontWeight: 700,
                cursor: isPending ? 'progress' : 'pointer',
                transition: 'background-position 300ms ease,transform 0.15s ease,box-shadow 0.15s ease',
                boxShadow: isPending ? '0 1px 2px rgba(0,0,0,0.06)' : '0 2px 4px rgba(0,0,0,0.06)',
                color: '#4f4f4f',
                backgroundImage: `linear-gradient(to right, ${cm.grad}, white)`,
                backgroundSize: '200% 100%',
                backgroundPosition: isPending ? 'right 75% top 0' : 'right 0 top 0',
                fontFamily: "'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif",
                whiteSpace: 'nowrap',
                opacity: isPending ? 0.86 : 1,
                ...buttonStyle,
              }}
              disabled={isPending}
              onClick={() => acknowledgeAction(action)}
              onMouseEnter={(e) => {
                if (isPending) return;
                e.currentTarget.style.backgroundPosition = 'right 40% top 0';
                e.currentTarget.style.transform = 'translateY(-1px)';
                e.currentTarget.style.boxShadow = '0 4px 8px rgba(0,0,0,0.08)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundPosition = 'right 0 top 0';
                e.currentTarget.style.transform = 'translateY(0)';
                e.currentTarget.style.boxShadow = '0 2px 4px rgba(0,0,0,0.06)';
              }}
              onMouseDown={(e) => {
                if (isPending) return;
                e.currentTarget.style.backgroundPosition = 'right 100% top 0';
                e.currentTarget.style.transform = 'translateY(0)';
              }}
              onMouseUp={(e) => {
                if (isPending) return;
                e.currentTarget.style.backgroundPosition = 'right 40% top 0';
                e.currentTarget.style.transform = 'translateY(-1px)';
              }}
            >
              {iconName && (
                <span
                  dangerouslySetInnerHTML={{ __html: icon(iconName, { size: 14 }) }}
                  style={{ marginRight: '4px', verticalAlign: 'middle' }}
                />
              )}
              <span style={{ verticalAlign: 'middle' }}>{text}</span>
            </button>
          );
        }

        const bg = style === 'primary' ? '#3bb8d8' : style === 'danger' ? '#e03e3e' : '#fff';
        return (
          <button
            key={action}
            className={btnClass}
            data-action={action}
            data-params={data ? JSON.stringify(data) : undefined}
            style={{
              border: `2px solid ${DEFAULT_TOKENS.line}`,
              borderRadius: '9px',
              padding: isDy ? '8px 14px' : '8px 16px',
              fontSize: isDy ? '12px' : '13px',
              fontWeight: 800,
              cursor: isPending ? 'progress' : 'pointer',
              transition: 'transform 0.12s ease,box-shadow 0.12s ease,background 0.12s ease',
              boxShadow: isPending ? `1px 1px 0 ${DEFAULT_TOKENS.line}` : `2px 2px 0 ${DEFAULT_TOKENS.line}`,
              background: bg,
              color: DEFAULT_TOKENS.ink,
              fontFamily: "'Arial Black','Segoe UI',sans-serif",
              whiteSpace: 'nowrap',
              opacity: isPending ? 0.88 : 1,
              ...buttonStyle,
            }}
            disabled={isPending}
            onClick={() => acknowledgeAction(action)}
            onMouseEnter={(e) => {
              if (isPending) return;
              e.currentTarget.style.transform = 'translate(-1px,-1px)';
              e.currentTarget.style.boxShadow = '3px 3px 0 #121212';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.transform = 'translate(0,0)';
              e.currentTarget.style.boxShadow = '2px 2px 0 #121212';
            }}
            onMouseDown={(e) => {
              if (isPending) return;
              e.currentTarget.style.transform = 'translate(1px,1px)';
              e.currentTarget.style.boxShadow = '1px 1px 0 #121212';
            }}
            onMouseUp={(e) => {
              if (isPending) return;
              e.currentTarget.style.transform = 'translate(-1px,-1px)';
              e.currentTarget.style.boxShadow = '3px 3px 0 #121212';
            }}
          >
            {iconName && (
              <span
                dangerouslySetInnerHTML={{ __html: icon(iconName, { size: 14 }) }}
                style={{ marginRight: '4px', verticalAlign: 'middle' }}
              />
            )}
            <span style={{ verticalAlign: 'middle' }}>{text}</span>
          </button>
        );
      })}
    </div>
  );
}

const buttonGroupRoots = new Map();

export function renderButtonGroup(container, props) {
  let root = buttonGroupRoots.get(container);
  if (!root) {
    root = createRoot(container);
    buttonGroupRoots.set(container, root);
  }
  root.render(<ButtonGroup {...props} />);
}

export function unmountButtonGroup(container) {
  const root = buttonGroupRoots.get(container);
  if (root) {
    root.unmount();
    buttonGroupRoots.delete(container);
  }
}
