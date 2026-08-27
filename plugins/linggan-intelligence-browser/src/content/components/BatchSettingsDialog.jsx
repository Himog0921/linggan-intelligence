import React, { useState, useEffect } from 'react';
import { COMMENT_DEPTH_MODE } from '../../shared/constants.js';
import { mountDialog, unmountDialog, NEO_OVERLAY_STYLE, NEO_PANEL_STYLE } from './dialogShared.js';
import {
  XHS_SEARCH_FILTERS,
  normalizeXhsSearchFilters,
} from '../../platforms/xhs/searchFilters.js';

const COUNT_OPTIONS = [5, 10, 20, 50];

function BatchSettingsDialog({
  title = '批量采集设置',
  enableTopLikes = true,
  enableSearchFilters = false,
  defaultSearchFilters = XHS_SEARCH_FILTERS.defaults,
  enableCommentDepth = false,
  enableCommentLimit = false,
  defaultDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL,
  onResolve,
}) {
  const showSearchFilters = Boolean(enableSearchFilters);
  const showTopLikes = enableTopLikes && !showSearchFilters;
  const subtitle = showSearchFilters
    ? '选择采集数量和小红书搜索筛选'
    : enableCommentDepth
    ? '选择采集数量、评论深度和可选排序方式'
    : (enableTopLikes ? '选择要采集的笔记数量和排序方式' : '选择要采集的篇数');

  const [selectedCount, setSelectedCount] = useState(10);
  const [topByLikes, setTopByLikes] = useState(false);
  const [searchFilters, setSearchFilters] = useState(() => normalizeXhsSearchFilters(defaultSearchFilters));
  const [commentDepthMode, setCommentDepthMode] = useState(
    defaultDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? COMMENT_DEPTH_MODE.ALL_REPLIES : COMMENT_DEPTH_MODE.TWO_LEVEL,
  );
  const [commentLimit, setCommentLimit] = useState('');

  const isAll = commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES;
  const updateSearchFilter = (groupKey, value) => {
    setSearchFilters((prev) => normalizeXhsSearchFilters({ ...prev, [groupKey]: value }));
  };

  useEffect(() => {
    const handleKey = (e) => {
      if (e.key === 'Escape') onResolve(null);
    };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
  }, [onResolve]);

  return (
    <div style={NEO_OVERLAY_STYLE} onClick={(e) => { if (e.target === e.currentTarget) onResolve(null); }}>
      <div style={{ ...NEO_PANEL_STYLE, maxHeight: 'calc(100vh - 48px)', overflowY: 'auto' }} role="dialog" aria-modal="true">
        <div style={{ fontSize: '34px', fontWeight: 900, color: '#121212', marginBottom: '8px' }}>{title}</div>
        <div style={{ fontSize: '13px', color: '#444', marginBottom: '20px', fontFamily: "'Segoe UI',sans-serif", fontWeight: 700, lineHeight: 1.6 }}>
          {subtitle}
        </div>

        <label style={{ fontSize: '14px', fontWeight: 900, color: '#121212', display: 'block', marginBottom: '8px' }}>
          采集数量
        </label>
        <div style={{ display: 'flex', gap: '8px', marginBottom: '16px', flexWrap: 'wrap' }}>
          {COUNT_OPTIONS.map((count) => (
            <button
              key={count}
              type="button"
              onClick={() => setSelectedCount(count)}
              style={{
                padding: '8px 20px',
                borderRadius: '10px',
                border: '2px solid #121212',
                background: selectedCount === count ? '#3bb8d8' : '#fff',
                fontSize: '14px',
                fontWeight: 900,
                cursor: 'pointer',
                color: '#121212',
                transition: 'all 0.2s',
                boxShadow: '2px 2px 0 #121212',
              }}
            >
              {count} 篇
            </button>
          ))}
        </div>

        {showSearchFilters && (
          <div style={{ marginBottom: '16px' }}>
            {Object.entries(XHS_SEARCH_FILTERS.groups).map(([groupKey, group]) => (
              <div key={groupKey} style={{ marginBottom: '12px' }}>
                <label style={{ fontSize: '14px', fontWeight: 900, color: '#121212', display: 'block', marginBottom: '8px' }}>
                  {group.title}
                </label>
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {group.options.map((option) => {
                    const active = searchFilters[groupKey] === option.value;
                    return (
                      <button
                        key={option.value}
                        type="button"
                        onClick={() => updateSearchFilter(groupKey, option.value)}
                        style={{
                          padding: '8px 14px',
                          borderRadius: '10px',
                          border: '2px solid #121212',
                          background: active ? '#f04a7d' : '#fff',
                          fontSize: '13px',
                          fontWeight: 900,
                          cursor: 'pointer',
                          color: '#121212',
                          transition: 'all 0.2s',
                          boxShadow: '2px 2px 0 #121212',
                        }}
                      >
                        {option.label}
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        )}

        {showTopLikes && (
          <button
            type="button"
            onClick={() => setTopByLikes((v) => !v)}
            style={{
              width: '100%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'flex-start',
              gap: '10px',
              marginBottom: '16px',
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
              background: topByLikes ? '#7dd87a' : '#fff',
              fontSize: '12px',
            }}>
              {topByLikes ? '✓' : ''}
            </span>
            <span>只采集数据最高的前 N 篇（按点赞排序）</span>
          </button>
        )}

        {enableCommentDepth && (
          <>
            <button
              type="button"
              onClick={() => setCommentDepthMode(isAll ? COMMENT_DEPTH_MODE.TWO_LEVEL : COMMENT_DEPTH_MODE.ALL_REPLIES)}
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
              <span>{isAll ? '尽量展开全部回复评论' : '只采一级 + 二级评论'}</span>
            </button>
            <small style={{ display: 'block', color: '#555', lineHeight: 1.5, margin: '-2px 0 16px', fontSize: '12px', fontFamily: "'Segoe UI',sans-serif", fontWeight: 600 }}>
              开启后会尽量展开全部回复，速度更慢，但更接近全量采集。
            </small>
          </>
        )}

        {enableCommentLimit && (
          <>
            <label style={{ fontSize: '14px', fontWeight: 900, color: '#121212', display: 'block', marginBottom: '6px' }}>
              每篇评论上限
            </label>
            <input
              type="number"
              min={0}
              placeholder="留空或 0 = 尽量全部"
              value={commentLimit}
              onChange={(e) => setCommentLimit(e.target.value)}
              style={{
                width: '100%',
                boxSizing: 'border-box',
                border: '2px solid #121212',
                borderRadius: '10px',
                padding: '12px 14px',
                fontSize: '16px',
                color: '#121212',
                outline: 'none',
                marginBottom: '18px',
                boxShadow: '2px 2px 0 #121212',
                fontFamily: "'Arial Black','Segoe UI',sans-serif",
              }}
            />
          </>
        )}

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
              const limit = enableCommentLimit
                ? Math.max(0, parseInt(String(commentLimit).trim(), 10) || 0)
                : 0;
              onResolve({
                count: selectedCount,
                topByLikes: showTopLikes ? topByLikes : false,
                searchFilters: showSearchFilters ? searchFilters : normalizeXhsSearchFilters(),
                commentLimit: limit,
                commentDepthMode,
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
            开始采集
          </button>
        </div>
      </div>
    </div>
  );
}

export function showBatchSettingsDialog(options = {}) {
  return new Promise((resolve, reject) => {
    const handleResolve = (result) => {
      unmountDialog(container);
      if (result === null) {
        reject(new Error('用户已取消'));
      } else {
        resolve(result);
      }
    };

    const { container } = mountDialog('lgboom-limit-overlay', <BatchSettingsDialog {...options} onResolve={handleResolve} />);
  });
}
