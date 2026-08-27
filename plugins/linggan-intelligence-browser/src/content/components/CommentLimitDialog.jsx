import React, { useState, useEffect, useRef } from 'react';
import { createRoot } from 'react-dom/client';
import { COMMENT_DEPTH_MODE } from '../../shared/constants.js';
import { mountDialog, unmountDialog, NEO_OVERLAY_STYLE, NEO_PANEL_STYLE } from './dialogShared.js';

function CommentLimitDialog({
  title = '采集评论设置',
  description = '填写最多采集多少条评论，留空或填 0 表示不限。可选只采一级+二级评论，或尽量展开全部回复。',
  confirmText = '开始采集',
  defaultDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL,
  onResolve,
}) {
  const [maxComments, setMaxComments] = useState('');
  const [depthMode, setDepthMode] = useState(defaultDepthMode);
  const inputRef = useRef(null);
  const focusTimer = useRef(null);

  useEffect(() => {
    focusTimer.current = setTimeout(() => inputRef.current?.focus(), 50);
    const handleKey = (e) => {
      if (e.key === 'Escape') onResolve(null);
    };
    document.addEventListener('keydown', handleKey);
    return () => {
      clearTimeout(focusTimer.current);
      document.removeEventListener('keydown', handleKey);
    };
  }, [onResolve]);

  const isAll = depthMode === COMMENT_DEPTH_MODE.ALL_REPLIES;

  return (
    <div style={NEO_OVERLAY_STYLE} onClick={(e) => { if (e.target === e.currentTarget) onResolve(null); }}>
      <div style={NEO_PANEL_STYLE} role="dialog" aria-modal="true">
        <div style={{ fontSize: '34px', fontWeight: 900, color: '#121212', marginBottom: '8px' }}>{title}</div>
        <div style={{ fontSize: '13px', color: '#444', marginBottom: '18px', fontFamily: "'Segoe UI',sans-serif", fontWeight: 700, lineHeight: 1.6 }}>
          {description}
        </div>

        <label style={{ fontSize: '14px', fontWeight: 900, color: '#121212', display: 'block', marginBottom: '6px' }}>
          最多采集评论数
        </label>
        <input
          ref={inputRef}
          type="number"
          min={0}
          placeholder="不限（默认采集全部）"
          value={maxComments}
          onChange={(e) => setMaxComments(e.target.value)}
          style={{
            width: '100%',
            boxSizing: 'border-box',
            border: '2px solid #121212',
            borderRadius: '10px',
            padding: '12px 14px',
            fontSize: '18px',
            color: '#121212',
            outline: 'none',
            marginBottom: '16px',
            boxShadow: '2px 2px 0 #121212',
            fontFamily: "'Arial Black','Segoe UI',sans-serif",
          }}
        />

        <button
          type="button"
          onClick={() => setDepthMode(isAll ? COMMENT_DEPTH_MODE.TWO_LEVEL : COMMENT_DEPTH_MODE.ALL_REPLIES)}
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'flex-start',
            gap: '10px',
            marginBottom: '8px',
            border: '2px solid #121212',
            borderRadius: '10px',
            background: '#fff',
            padding: '10px 12px',
            cursor: 'pointer',
            fontSize: '14px',
            fontWeight: 800,
            color: '#121212',
            boxShadow: '2px 2px 0 #121212',
          }}
        >
          <span style={{
            display: 'inline-flex',
            width: '20px',
            height: '20px',
            border: '2px solid #121212',
            borderRadius: '6px',
            alignItems: 'center',
            justifyContent: 'center',
            background: isAll ? '#7dd87a' : '#fff',
            fontSize: '12px',
          }}>
            {isAll ? '✓' : ''}
          </span>
          <span>{isAll ? '全部展开（含所有楼层回复）' : '仅一二级评论（速度快）'}</span>
        </button>
        <small style={{ display: 'block', color: '#555', lineHeight: 1.5, margin: '-2px 0 18px', fontSize: '12px', fontFamily: "'Segoe UI',sans-serif", fontWeight: 600 }}>
          {isAll ? '会尽量展开所有回复楼层，耗时较长，适合深度分析。' : '只采主评论和直接回复，速度快，适合快速了解主要声音。'}
        </small>

        <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
          <button
            type="button"
            onClick={() => onResolve(null)}
            style={{
              background: '#fff',
              color: '#121212',
              border: '2px solid #121212',
              borderRadius: '10px',
              padding: '10px 22px',
              fontSize: '14px',
              fontWeight: 800,
              cursor: 'pointer',
              boxShadow: '2px 2px 0 #121212',
            }}
          >
            取消
          </button>
          <button
            type="button"
            onClick={() => {
              const val = parseInt(maxComments, 10);
              onResolve({
                maxComments: isNaN(val) || val <= 0 ? 0 : val,
                commentDepthMode: depthMode,
              });
            }}
            style={{
              background: '#3bb8d8',
              color: '#121212',
              border: '2px solid #121212',
              borderRadius: '10px',
              padding: '10px 22px',
              fontSize: '14px',
              fontWeight: 800,
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

export function showCommentLimitDialog(options = {}) {
  return new Promise((resolve, reject) => {
    const handleResolve = (result) => {
      unmountDialog(container);
      if (result === null) {
        reject(new Error('用户已取消'));
      } else {
        resolve(result);
      }
    };

    const { container } = mountDialog('lgboom-limit-overlay', <CommentLimitDialog {...options} onResolve={handleResolve} />);
  });
}
