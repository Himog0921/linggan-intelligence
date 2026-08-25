import React from 'react';
import { PLATFORM } from '../utils.js';

export default function ActionButtons({
  platform,
  capabilities,
  onCollectNote,
  onCollectSecondary,
  onCommentImages,
  busyPrimary = false,
  busySecondary = false,
  busyCommentImages = false,
}) {
  const isDouyin = platform === PLATFORM.DOUYIN;
  const hasCurrentContentActions = Boolean(capabilities.canCollectPrimary || capabilities.canCollectSecondary);
  const primaryText = isDouyin
    ? (capabilities.canCollectPrimary ? '采集当前视频' : '当前页不支持')
    : (capabilities.canCollectPrimary ? '采集当前笔记' : '先打开笔记页');
  const secondaryText = capabilities.canCollectSecondary
    ? (capabilities.secondaryAction === 'author' ? '采集当前博主' : '采集当前评论')
    : (isDouyin ? '当前页不支持' : '先打开笔记详情页或博主页');

  if (!hasCurrentContentActions) {
    return (
      <div className="action-empty-state" id="actionEmptyState">
        <strong>当前页还不是可执行采集面板</strong>
        <p>
          {isDouyin
            ? '切到作品详情或作者主页后，再发起单条采集。'
            : '先打开笔记详情页或博主页，再发起单条采集。'}
        </p>
        <div className="action-empty-tags" aria-hidden="true">
          <span>{isDouyin ? '视频详情' : '笔记详情'}</span>
          <span>评论采集</span>
          <span>博主采集</span>
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="btn-row">
        <button
          id="btnCollectNote"
          className={`popup-btn primary${busyPrimary ? ' is-busy' : ''}`}
          disabled={!capabilities.canCollectPrimary || busyPrimary}
          aria-busy={busyPrimary ? 'true' : 'false'}
          onClick={onCollectNote}
        >
          {busyPrimary ? '正在发起...' : primaryText}
        </button>
        <button
          id="btnCollectComment"
          className={`popup-btn secondary${busySecondary ? ' is-busy' : ''}`}
          disabled={!capabilities.canCollectSecondary || busySecondary}
          aria-busy={busySecondary ? 'true' : 'false'}
          onClick={onCollectSecondary}
        >
          {busySecondary ? '正在发起...' : secondaryText}
        </button>
      </div>
      {isDouyin && capabilities.canDownloadCommentImages && (
        <div className="btn-row">
          <button
            id="btnCommentImages"
            className={`popup-btn secondary${busyCommentImages ? ' is-busy' : ''}`}
            disabled={!capabilities.canDownloadCommentImages || busyCommentImages}
            aria-busy={busyCommentImages ? 'true' : 'false'}
            onClick={onCommentImages}
          >
            {busyCommentImages ? '正在扫描...' : '评论图片区'}
          </button>
        </div>
      )}
    </>
  );
}
