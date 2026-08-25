import React from 'react';

export default function StatsSection({ stats }) {
  const unavailable = stats.statsState === 'not_connected' ? '未连接' : '未知';
  const display = (value) => (Number.isFinite(value) ? value : unavailable);
  return (
    <div className="stats-section">
      <div className="stat-item">
        <span className="stat-num" id="noteCount">{display(stats.notes)}</span>
        <span className="stat-label">笔记/视频</span>
      </div>
      <div className="stat-item">
        <span className="stat-num" id="commentCount">{display(stats.comments)}</span>
        <span className="stat-label">评论</span>
      </div>
      <div className="stat-item">
        <span className="stat-num" id="authorCount">{display(stats.authors)}</span>
        <span className="stat-label">博主</span>
      </div>
    </div>
  );
}
