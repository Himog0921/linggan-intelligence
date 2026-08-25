import React from 'react';
import { COMMENT_DEPTH_MODE } from '../../shared/constants.js';

export default function ProgressSection({ visible, current, total, status, stage, depthMode }) {
  if (!visible) return null;
  const pct = total > 0 ? Math.round((current / total) * 100) : 0;

  return (
    <div id="progressSection" className="progress-section">
      <div className="progress-card">
        <div className="section-heading task-heading">
          <div>
            <h2 id="progressTitle">{stage.description || '任务进行中'}</h2>
          </div>
          <span id="progressStageBadge" className={`task-stage-badge ${stage.className || ''}`.trim()}>
            {stage.label || '进行中'}
          </span>
        </div>
        <div className="progress-bar">
          <div id="progressFill" className="progress-fill" style={{ width: `${pct}%` }} />
        </div>
        <div className="progress-info">
          <span id="progressCount">{current}/{total}</span>
          <span id="progressStatus">{status}</span>
        </div>
        {depthMode && (
          <div id="progressDepthHint" className="progress-depth-hint">
            {depthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? '评论深度：全部展开' : '评论深度：仅一二级'}
          </div>
        )}
      </div>
    </div>
  );
}
