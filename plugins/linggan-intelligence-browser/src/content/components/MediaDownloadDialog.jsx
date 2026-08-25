import React, { useEffect, useMemo, useState } from 'react';
import { mountDialog, unmountDialog } from './dialogShared.js';

const OVERLAY_STYLE = {
  position: 'fixed',
  inset: 0,
  background: 'rgba(0,0,0,0.45)',
  zIndex: '2147483647',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
};

const PANEL_STYLE = {
  background: '#fff',
  borderRadius: '16px',
  padding: '32px 36px',
  boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
  maxWidth: '380px',
  width: '90%',
  fontFamily: "-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif",
};

function hasValue(value) {
  if (Array.isArray(value)) return value.some(hasValue);
  if (value && typeof value === 'object') {
    return [
      value.urlDefault,
      value.url_default,
      value.url,
      value.src,
      value.href,
      value.masterUrl,
      value.master_url,
      value.candidates,
      value.streams,
    ].some(hasValue);
  }
  return Boolean(String(value || '').trim());
}

function getMediaOptions(note, mediaCount, noteType) {
  if (!note || typeof note !== 'object') {
    return [{
      value: 'all',
      label: noteType === 'video' ? '下载视频' : '下载所有图片',
      count: Number(mediaCount || 0),
    }].filter((item) => item.count > 0);
  }

  const imageCount = Array.isArray(note.imageCandidates) && note.imageCandidates.length > 0
    ? note.imageCandidates.filter(hasValue).length
    : (Array.isArray(note.images) ? note.images.filter(hasValue).length : 0);
  const liveCount = Array.isArray(note.livePhotoStreams) ? note.livePhotoStreams.filter(hasValue).length : 0;
  const hasVideo = hasValue([note.videoStreams, note.videoDownloadUrl, note.videoPlayUrl, note.video]);
  const isVideoNote = String(note.type || '').trim() === 'video' || hasVideo;
  const hasCover = hasValue([
    note.cover,
    note.coverImg,
    note.coverUrl,
    note.thumbnail,
    Array.isArray(note.images) ? note.images[0] : '',
    Array.isArray(note.imageCandidates) ? note.imageCandidates[0] : '',
  ]);

  const options = [];
  if (hasCover) options.push({ value: 'cover', label: '下载封面', count: 1 });
  if (imageCount > 0 && (!isVideoNote || imageCount > 1)) {
    options.push({ value: 'images', label: '下载所有图片', count: imageCount });
  }
  if (liveCount > 0) options.push({ value: 'live', label: '下载 Live', count: liveCount });
  if (hasVideo) options.push({ value: 'video', label: '下载视频', count: 1 });
  return options;
}

function countSelectedMedia(options, selected) {
  const hasImages = selected.has('images');
  return options
    .filter((item) => !(item.value === 'cover' && hasImages))
    .filter((item) => selected.has(item.value))
    .reduce((total, item) => total + Number(item.count || 0), 0);
}

function MediaDownloadDialog({ note, mediaCount, noteType, onResolve }) {
  const options = useMemo(() => getMediaOptions(note, mediaCount, noteType), [note, mediaCount, noteType]);
  const [selected, setSelected] = useState(() => new Set(options.map((item) => item.value)));
  const selectedCount = countSelectedMedia(options, selected);
  const totalCount = countSelectedMedia(options, new Set(options.map((item) => item.value)));

  useEffect(() => {
    const handleKey = (e) => {
      if (e.key === 'Escape') onResolve(false);
    };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
  }, [onResolve]);

  const toggle = (value) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(value)) next.delete(value);
      else next.add(value);
      return next;
    });
  };

  const submit = () => {
    const mediaTypes = options.filter((item) => selected.has(item.value)).map((item) => item.value);
    if (mediaTypes.length === 0) return;
    onResolve({ mediaTypes, count: selectedCount });
  };

  return (
    <div style={OVERLAY_STYLE} onClick={(e) => { if (e.target === e.currentTarget) onResolve(false); }}>
      <div style={PANEL_STYLE} role="dialog" aria-modal="true">
        <div style={{ fontSize: '20px', fontWeight: 700, color: '#333', marginBottom: '8px' }}>下载媒体文件</div>
        <div style={{ fontSize: '14px', color: '#666', marginBottom: '20px' }}>
          该笔记包含 <strong>{totalCount}</strong> 个可下载媒体文件
        </div>
        <div style={{ display: 'grid', gap: '10px', marginBottom: '22px' }}>
          {options.map((item) => {
            const checked = selected.has(item.value);
            return (
              <label
                key={item.value}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '10px',
                  padding: '10px 12px',
                  border: `1px solid ${checked ? '#ff4757' : '#e5e5e5'}`,
                  borderRadius: '10px',
                  cursor: 'pointer',
                  background: checked ? '#fff5f6' : '#fff',
                }}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => toggle(item.value)}
                  style={{ width: '16px', height: '16px', accentColor: '#ff4757' }}
                />
                <span style={{ flex: 1, fontSize: '14px', color: '#333', fontWeight: 600 }}>{item.label}</span>
                <span style={{ fontSize: '13px', color: '#777' }}>{item.count} 个</span>
              </label>
            );
          })}
        </div>
        <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
          <button
            type="button"
            onClick={() => onResolve(false)}
            style={{
              background: '#f0f0f0',
              color: '#555',
              border: 'none',
              borderRadius: '8px',
              padding: '10px 22px',
              fontSize: '14px',
              fontWeight: 600,
              cursor: 'pointer',
            }}
          >
            不用了
          </button>
          <button
            type="button"
            onClick={submit}
            disabled={selectedCount === 0}
            style={{
              background: selectedCount === 0 ? '#d8d8d8' : '#ff4757',
              color: '#fff',
              border: 'none',
              borderRadius: '8px',
              padding: '10px 22px',
              fontSize: '14px',
              fontWeight: 600,
              cursor: selectedCount === 0 ? 'not-allowed' : 'pointer',
            }}
          >
            下载选中
          </button>
        </div>
      </div>
    </div>
  );
}

export function showMediaDownloadDialog(noteOrCount, noteType) {
  return new Promise((resolve) => {
    const note = noteOrCount && typeof noteOrCount === 'object' ? noteOrCount : null;
    const mediaCount = note ? 0 : Number(noteOrCount || 0);
    const handleResolve = (result) => {
      unmountDialog(container);
      resolve(result);
    };

    const { container } = mountDialog('lgboom-limit-overlay', <MediaDownloadDialog note={note} mediaCount={mediaCount} noteType={noteType} onResolve={handleResolve} />);
  });
}
