import React from 'react';

export default function StatsSection({ stats }) {
  return (
    <div className="stats-section">
      <div className="stat-item">
        <span className="stat-num" id="noteCount">{stats.notes}</span>
        <span className="stat-label">笔记/视频</span>
      </div>
      <div className="stat-item">
        <span className="stat-num" id="commentCount">{stats.comments}</span>
        <span className="stat-label">评论</span>
      </div>
      <div className="stat-item">
        <span className="stat-num" id="authorCount">{stats.authors}</span>
        <span className="stat-label">博主</span>
      </div>
    </div>
  );
}
