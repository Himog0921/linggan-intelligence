import React, { useState, useEffect, useRef } from 'react';
import { mountDialog, unmountDialog, NEO_OVERLAY_STYLE } from './dialogShared.js';

const PANEL_STYLE = {
  width: 'min(464px, 100%)',
  background: '#fff8da',
  border: '3px solid #121212',
  boxShadow: '6px 6px 0 #121212',
  borderRadius: '22px',
  padding: '18px 20px 18px',
  fontFamily: "'SF Pro Text','Segoe UI','PingFang SC','Microsoft YaHei',sans-serif",
};

const HEADLINE_FONT = "'Avenir Next Demi Bold','Avenir Next','SF Pro Display','PingFang SC','Hiragino Sans GB','Microsoft YaHei',sans-serif";
const BODY_FONT = "'SF Pro Text','Segoe UI','PingFang SC','Microsoft YaHei',sans-serif";

function getDialogIconText(title = '') {
  if (/评论图片区/.test(title)) return '图';
  if (/评论/.test(title)) return '评';
  if (/批量/.test(title)) return '批';
  return '采';
}

function sanitizeFieldValue(field, rawValue) {
  if (field.type === 'checkbox') return Boolean(rawValue);
  if (field.type === 'number') return String(rawValue ?? '').replace(/[^\d]/g, '');
  return String(rawValue ?? '');
}

function buildQuickOptionLabel(field, value) {
  if (field.name === 'maxCommentsPerVideo' && value === 0) return '全部';
  return String(value);
}

function ActionDialog({
  title = '操作',
  description = '',
  confirmText = '开始',
  cancelText = '取消',
  fields = [],
  onResolve,
}) {
  const [values, setValues] = useState(() => {
    const initial = {};
    fields.forEach((field) => {
      initial[field.name] = field.type === 'checkbox'
        ? Boolean(field.defaultValue)
        : sanitizeFieldValue(field, field.defaultValue ?? '');
    });
    return initial;
  });

  const firstRef = useRef(null);
  const focusTimer = useRef(null);
  const iconText = getDialogIconText(title);

  useEffect(() => {
    focusTimer.current = setTimeout(() => firstRef.current?.focus(), 50);
    const handleKey = (e) => {
      if (e.key === 'Escape') onResolve(null);
    };
    document.addEventListener('keydown', handleKey);
    return () => {
      clearTimeout(focusTimer.current);
      document.removeEventListener('keydown', handleKey);
    };
  }, [onResolve]);

  return (
    <div
      style={NEO_OVERLAY_STYLE}
      onClick={(e) => { if (e.target === e.currentTarget) onResolve(null); }}
      aria-hidden="false"
    >
      <div style={PANEL_STYLE} role="dialog" aria-modal="true" aria-labelledby="lgboom-action-dialog-title">
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: '12px', marginBottom: '14px' }}>
          <div style={{
            width: '42px',
            height: '42px',
            border: '2px solid #121212',
            borderRadius: '14px',
            background: 'linear-gradient(180deg, #fffef8 0%, #ffe88a 100%)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: '22px',
            fontWeight: 900,
            color: '#121212',
            boxShadow: '2px 2px 0 #121212',
            fontFamily: HEADLINE_FONT,
            flex: '0 0 auto',
          }}>
            {iconText}
          </div>

          <div style={{ minWidth: 0, flex: 1 }}>
            <h2
              id="lgboom-action-dialog-title"
              style={{
                fontSize: '24px',
                fontWeight: 900,
                color: '#121212',
                lineHeight: 1.16,
                margin: '0 0 6px',
                letterSpacing: '0.12px',
                fontFamily: HEADLINE_FONT,
              }}
            >
              {title}
            </h2>

            {description && (
              <p style={{
                fontFamily: BODY_FONT,
                fontSize: '13px',
                fontWeight: 700,
                lineHeight: 1.62,
                margin: 0,
                color: '#55514a',
              }}>
                {description}
              </p>
            )}
          </div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          {fields.map((field, idx) => {
            const fieldValue = values[field.name];

            if (field.type === 'checkbox') {
              const checked = Boolean(fieldValue);
              return (
                <label
                  key={field.name}
                  style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: '12px',
                    padding: '14px 14px',
                    border: '2px solid #121212',
                    borderRadius: '16px',
                    background: checked ? '#f2fcff' : '#fff',
                    boxShadow: checked ? '3px 3px 0 #121212' : '2px 2px 0 #121212',
                    fontFamily: BODY_FONT,
                    cursor: 'pointer',
                    transition: 'background 120ms ease, box-shadow 120ms ease, transform 120ms ease',
                  }}
                >
                  <input
                    ref={idx === 0 ? firstRef : null}
                    type="checkbox"
                    checked={checked}
                    onChange={(e) => setValues((prev) => ({ ...prev, [field.name]: e.target.checked }))}
                    style={{
                      position: 'absolute',
                      opacity: 0,
                      pointerEvents: 'none',
                      width: 1,
                      height: 1,
                    }}
                  />
                  <span style={{
                    display: 'inline-flex',
                    width: '24px',
                    height: '24px',
                    border: '2px solid #121212',
                    borderRadius: '7px',
                    alignItems: 'center',
                    justifyContent: 'center',
                    background: checked ? '#3bb8d8' : '#fff',
                    color: '#121212',
                    fontSize: '14px',
                    fontWeight: 900,
                    lineHeight: 1,
                    flex: '0 0 auto',
                    marginTop: '1px',
                  }}>
                    {checked ? '✓' : ''}
                  </span>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '5px', minWidth: 0 }}>
                    <span style={{
                      display: 'block',
                      fontSize: '14px',
                      fontWeight: 900,
                      color: '#121212',
                      lineHeight: 1.35,
                      fontFamily: HEADLINE_FONT,
                    }}>
                      {field.label || field.name}
                    </span>
                    {field.helpText && (
                      <small style={{
                        display: 'block',
                        color: '#585858',
                        lineHeight: 1.6,
                        fontSize: '12px',
                        fontFamily: BODY_FONT,
                        fontWeight: 700,
                      }}>
                        {field.helpText}
                      </small>
                    )}
                  </div>
                </label>
              );
            }

            const currentString = String(fieldValue ?? '');

            return (
              <div
                key={field.name}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '8px',
                  padding: '13px 14px 12px',
                  border: '2px solid #121212',
                  borderRadius: '16px',
                  background: '#fff',
                  boxShadow: '2px 2px 0 #121212',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                  <span style={{
                    fontSize: '14px',
                    fontWeight: 900,
                    color: '#121212',
                    fontFamily: HEADLINE_FONT,
                    lineHeight: 1.3,
                  }}>
                    {field.label || field.name}
                  </span>
                </div>

                <input
                  ref={idx === 0 ? firstRef : null}
                  type={field.type === 'number' ? 'text' : (field.type || 'text')}
                  inputMode={field.type === 'number' ? 'numeric' : undefined}
                  pattern={field.type === 'number' ? '[0-9]*' : undefined}
                  value={currentString}
                  onChange={(e) => {
                    const nextValue = sanitizeFieldValue(field, e.target.value);
                    setValues((prev) => ({ ...prev, [field.name]: nextValue }));
                  }}
                  placeholder={field.placeholder || ''}
                  min={field.min}
                  max={field.max}
                  style={{
                    height: '52px',
                    border: '2px solid #121212',
                    borderRadius: '15px',
                    padding: '0 16px',
                    fontSize: '17px',
                    fontFamily: HEADLINE_FONT,
                    fontVariantNumeric: 'tabular-nums',
                    letterSpacing: '0.12px',
                    background: '#fffef8',
                    color: '#121212',
                    boxShadow: 'inset 0 0 0 1px rgba(255, 255, 255, 0.65)',
                    outline: 'none',
                  }}
                  onFocus={(e) => { e.target.style.boxShadow = '0 0 0 3px rgba(59, 184, 216, 0.22), inset 0 0 0 1px rgba(255, 255, 255, 0.65)'; }}
                  onBlur={(e) => { e.target.style.boxShadow = 'inset 0 0 0 1px rgba(255, 255, 255, 0.65)'; }}
                />

                {Array.isArray(field.quickOptions) && field.quickOptions.length > 0 && (
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
                    {field.quickOptions.map((value) => {
                      const optionValue = value === 0 ? '0' : String(value);
                      const isActive = currentString === optionValue;
                      return (
                        <button
                          key={value}
                          type="button"
                          onClick={() => setValues((prev) => ({ ...prev, [field.name]: optionValue }))}
                          style={{
                            minWidth: '54px',
                            height: '36px',
                            padding: '0 13px',
                            border: '2px solid #121212',
                            borderRadius: '999px',
                            background: isActive ? '#121212' : '#fffdf0',
                            color: isActive ? '#fff8da' : '#121212',
                            boxShadow: isActive ? '2px 2px 0 #121212' : '1px 1px 0 #121212',
                            fontSize: '12px',
                            fontWeight: 900,
                            fontFamily: BODY_FONT,
                            cursor: 'pointer',
                            transition: 'background 120ms ease, color 120ms ease, box-shadow 120ms ease',
                          }}
                        >
                          {buildQuickOptionLabel(field, value)}
                        </button>
                      );
                    })}
                  </div>
                )}

                {field.helpText && (
                  <small style={{
                    color: '#5b5b5b',
                    lineHeight: 1.55,
                    fontSize: '12px',
                    fontFamily: BODY_FONT,
                    fontWeight: 700,
                  }}>
                    {field.helpText}
                  </small>
                )}
              </div>
            );
          })}
        </div>

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', marginTop: '14px' }}>
          <button
            type="button"
            onClick={() => onResolve(null)}
            style={{
              minWidth: '116px',
              height: '50px',
              background: '#fff',
              color: '#121212',
              border: '2px solid #121212',
              borderRadius: '16px',
              padding: '0 20px',
              fontSize: '15px',
              fontWeight: 900,
              fontFamily: HEADLINE_FONT,
              cursor: 'pointer',
              boxShadow: '2px 2px 0 #121212',
            }}
          >
            {cancelText}
          </button>
          <button
            type="button"
            onClick={() => onResolve(values)}
            style={{
              minWidth: '170px',
              height: '50px',
              background: '#3bb8d8',
              color: '#121212',
              border: '2px solid #121212',
              borderRadius: '16px',
              padding: '0 22px',
              fontSize: '15px',
              fontWeight: 900,
              fontFamily: HEADLINE_FONT,
              cursor: 'pointer',
              boxShadow: '2px 2px 0 #121212',
            }}
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}

export function showActionDialog(options = {}) {
  return new Promise((resolve) => {
    const handleResolve = (result) => {
      unmountDialog(container);
      resolve(result);
    };

    const { container } = mountDialog('lgboom-dy-dialog-overlay', <ActionDialog {...options} onResolve={handleResolve} />);
  });
}
