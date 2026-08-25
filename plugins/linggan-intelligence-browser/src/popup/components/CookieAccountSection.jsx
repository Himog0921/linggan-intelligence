import React from 'react';
import { getAccountStatusMeta } from '../../shared/feedback.js';

export default function CookieAccountSection({
  currentPlatform = 'unknown',
  cookieStatus,
  accounts,
  onGetCookies,
  onOpenAddAccount,
  onRemoveAccount,
  gettingCookies = false,
  openingAddAccount = false,
  removingAccountId = '',
}) {
  const currentPlatformKey = currentPlatform === 'douyin'
    ? 'douyin'
    : currentPlatform === 'xhs'
      ? 'xhs'
      : null;
  const currentPlatformLabel = currentPlatformKey === 'douyin' ? '抖音' : currentPlatformKey === 'xhs' ? '小红书' : '当前平台';
  const currentCookieData = currentPlatformKey ? cookieStatus?.[currentPlatformKey] : null;
  const accountCount = Array.isArray(accounts) ? accounts.length : 0;

  const formatCookieTime = (ts) => {
    if (!ts) return '';
    const d = new Date(ts);
    return `${d.getMonth() + 1}/${d.getDate()} ${d.getHours()}:${String(d.getMinutes()).padStart(2, '0')}`;
  };

  return (
    <div className="cookie-bottom-section">
      <div className="data-subsection cookie-status-card">
        <div className="section-heading compact">
          <div>
            <h2>Cookie 状态</h2>
            <p>只显示当前平台，抓取时间会影响当前页采集与同步。</p>
          </div>
        </div>
        <div className="cookie-status-row single-platform">
          {currentPlatformKey ? (
            <div className="cookie-platform-status">
              <span className="cookie-platform-label">{currentPlatformLabel}</span>
              <span
                id={`cookie${currentPlatformKey === 'xhs' ? 'Xhs' : 'Douyin'}Badge`}
                className={`cookie-platform-badge ${currentCookieData?.count > 0 ? 'captured' : 'not-captured'}`}
              >
                {currentCookieData?.count > 0 ? `${currentCookieData.count} 条 ${formatCookieTime(currentCookieData.capturedAt)}` : '未获取'}
              </span>
            </div>
          ) : (
            <div className="cookie-platform-status">
              <span className="cookie-platform-label">未识别页面</span>
              <span className="cookie-platform-badge not-captured">请先打开小红书或抖音页面</span>
            </div>
          )}
          <button
            id="btnGetCookies"
            className={`popup-btn primary cookie-fetch-btn${gettingCookies ? ' is-busy' : ''}`}
            onClick={onGetCookies}
            disabled={!currentPlatformKey || gettingCookies}
            aria-busy={gettingCookies ? 'true' : 'false'}
          >
            {currentPlatformKey ? (gettingCookies ? '获取中...' : '获取 Cookie') : '切到支持页'}
          </button>
        </div>
      </div>

      <div className="data-subsection">
        <div className="section-heading compact">
          <div>
            <h2>采集账号</h2>
            <p>已保存账号会参与监控与执行，配额与状态会在这里持续更新。</p>
          </div>
          <span className="section-count-badge">{accountCount} 个账号</span>
        </div>

        <div id="accountList">
          {accountCount === 0 ? (
            <div className="empty-inline-note empty-account-state">
              <strong>还没有采集账号</strong>
              <p>先抓取当前平台 Cookie，或手动粘贴 Cookie 保存一个执行账号。</p>
            </div>
          ) : (
            accounts.map((a) => {
              const statusMeta = getAccountStatusMeta(a.status, a.cooldownUntil);
              return (
                <div key={a.accountId} className="account-item">
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div className="account-item-name">{a.name || '未命名'}</div>
                    <div className="account-item-meta">
                      <span>{a.dailyQuotaUsed || 0}/{a.dailyQuotaLimit || 100}</span>
                      <span className={`account-status-pill tone-${statusMeta.tone}`}>{statusMeta.label}</span>
                      <span>{statusMeta.detail}</span>
                    </div>
                  </div>
                  <div className="account-item-actions">
                    <button
                      className="delete"
                      onClick={() => onRemoveAccount(a.accountId)}
                      disabled={removingAccountId === a.accountId}
                    >
                      {removingAccountId === a.accountId ? '删除中...' : '删除'}
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>

        <button
          id="btnAddAccount"
          className={`popup-btn outline${openingAddAccount ? ' is-busy' : ''}`}
          onClick={onOpenAddAccount}
          disabled={openingAddAccount}
        >
          {openingAddAccount ? '正在打开...' : '+ 手动添加账号'}
        </button>
      </div>
    </div>
  );
}
