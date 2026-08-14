import React, { useState } from 'react';
import { MSG } from '../../shared/constants.js';
import { sendToBackground } from '../utils.js';

const PLATFORM_LABELS = {
  xhs: '小红书',
  douyin: '抖音',
};

const HEALTH_LABELS = {
  healthy: '可接单',
  needs_login: '需登录',
  restricted: '受限',
  cooling: '冷却中',
  unhealthy: '不可用',
  unknown: '待确认',
};

const LOGIN_LABELS = {
  logged_in: '已登录',
  logged_out: '未登录',
  login_expired: '登录过期',
  unknown: '登录待确认',
};

const PERMISSION_LABELS = {
  granted: '页面权限正常',
  denied: '缺少页面权限',
  missing: '缺少页面权限',
  unknown: '权限待确认',
};

const TASK_STATUS_LABELS = {
  dispatched: '已接单',
  running: '执行中',
  paused: '已暂停',
  retrying: '等待重试',
  failed: '失败',
  succeeded: '已完成',
};

function text(value = '') {
  return String(value || '').trim();
}

function platformLabel(platform = '') {
  return PLATFORM_LABELS[text(platform)] || text(platform) || '未知平台';
}

function shortId(value = '') {
  const normalized = text(value);
  if (!normalized) return '';
  if (normalized.length <= 14) return normalized;
  return `${normalized.slice(0, 6)}...${normalized.slice(-4)}`;
}

function uniqueParts(parts = []) {
  const seen = new Set();
  return parts.filter((part) => {
    const normalized = text(part);
    if (!normalized || seen.has(normalized)) return false;
    seen.add(normalized);
    return true;
  });
}

function formatAccountDiagnostics(account = {}) {
  const rawProfile = account.rawProfile || {};
  const label = text(account.displayName)
    || shortId(account.platformAccountId)
    || `${platformLabel(account.platform)}账号`;
  const details = uniqueParts([
    HEALTH_LABELS[text(account.healthStatus)] || text(account.healthStatus),
    LOGIN_LABELS[text(rawProfile.loginState)] || text(rawProfile.loginState),
    PERMISSION_LABELS[text(rawProfile.pagePermission)] || text(rawProfile.pagePermission),
    rawProfile.platformBlocked ? '平台限制' : '',
  ]);
  return {
    title: `${platformLabel(account.platform)} · ${label}`,
    detail: details.length ? details.join(' · ') : '状态待确认',
  };
}

function formatCurrentTask(task = null) {
  if (!task || typeof task !== 'object') return '空闲';
  const taskName = text(task.taskType) || shortId(task.externalTaskId) || shortId(task.taskId) || '当前任务';
  const status = TASK_STATUS_LABELS[text(task.workbenchStatus)] || text(task.workbenchStatus) || '执行中';
  const account = shortId(task.accountId);
  // 报告 §9.1：活跃任务的采集进度封顶显示 95%——100% 是"服务端已确认入库"，
  // 只要任务还在本地活跃，就说明还没拿到服务端终态确认。
  const rawProgress = Number(task.progress);
  const progressPart = Number.isFinite(rawProgress) && rawProgress > 0
    ? (rawProgress >= 95 ? '95%（等待服务端确认）' : `${Math.floor(rawProgress)}%`)
    : '';
  return uniqueParts([platformLabel(task.platform), taskName, account, status, progressPart]).join(' · ');
}

function formatLockSummary(locks = []) {
  const list = Array.isArray(locks) ? locks : [];
  if (!list.length) return '无';
  const preview = list.slice(0, 2).map((lock) => (
    uniqueParts([platformLabel(lock.platform), shortId(lock.accountId), shortId(lock.taskId)]).join(' · ')
  )).join('；');
  return list.length > 2 ? `${list.length} 个：${preview}...` : `${list.length} 个：${preview}`;
}

export default function FlywheelSection({
  flywheelUrl,
  flywheelStatus,
  authorizationCode,
  authorizationStatus = {},
  stationStatus,
  presetUrls = {},
  testing = false,
  authorizing = false,
  requestingAuthorization = false,
  claimingAuthorization = false,
  clearingAuthorization = false,
  syncing = false,
  onUrlChange,
  onUsePresetUrl,
  onAuthorizationCodeChange,
  onAuthorize,
  onRequestAuthorization,
  onClaimAuthorization,
  onClearAuthorization,
  onTest,
  onSync,
}) {
  const [exportingRecovery, setExportingRecovery] = useState(false);

  // 导出恢复包：只读操作，拿到 JSON 后本地保存，供 plugin_local_recovery 导入。
  const handleExportRecovery = async () => {
    if (exportingRecovery) return;
    setExportingRecovery(true);
    try {
      const response = await sendToBackground(MSG.EXPORT_OUTBOX_RECOVERY, {}, { timeoutMs: 60000 });
      if (!response?.success || !response.export) return;
      const blob = new Blob([JSON.stringify(response.export, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `linggan-recovery-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.json`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10000);
    } finally {
      setExportingRecovery(false);
    }
  };

  const statusText = {
    unconfigured: '未配置',
    configured: '已配置',
    testing: '测试中...',
    connected: '已连接',
    disconnected: '连接失败',
  }[flywheelStatus] || flywheelStatus;

  const authorization = authorizationStatus.authorization || {};
  const rawAuthorizationStatus = text(authorization.status || authorization.authorizationStatus);
  const isAuthorizationPending = rawAuthorizationStatus === 'pending';
  const isAuthorizationApproved = rawAuthorizationStatus === 'approved';
  const authorizationState = authorizationStatus.authorized
    ? 'connected'
    : (authorization.status === 'expired' || authorization.status === 'revoked'
      ? 'disconnected'
      : (isAuthorizationPending || isAuthorizationApproved ? 'testing' : 'unconfigured'));
  const authorizationLabel = authorizationStatus.authorized
    ? (authorization.memberName || authorization.teamName || authorization.authorizationId || '已授权')
    : (isAuthorizationPending ? '待审批' : isAuthorizationApproved ? '可领取' : '未授权');
  const authorizationHint = authorizationStatus.authorized
    ? `当前浏览器已授权给${authorization.memberName || '团队成员'}使用${authorization.teamName ? `，归属 ${authorization.teamName}` : ''}${authorization.expiresAt ? `，有效期至 ${authorization.expiresAt}` : ''}。授权成功后会自动创建工位。`
    : (authorizationStatus.authorizationMessage || '从内容工作台下载的插件会自动授权并准备工位；其他来源安装时，可以在这里发起授权申请。');
  const canRequestAuthorization = !authorizationStatus.authorized
    && !isAuthorizationPending
    && !isAuthorizationApproved
    && typeof onRequestAuthorization === 'function';
  const canClaimAuthorization = !authorizationStatus.authorized
    && (isAuthorizationPending || isAuthorizationApproved)
    && typeof onClaimAuthorization === 'function';

  const stationName = stationStatus.registered
    ? (stationStatus.identity?.displayName || stationStatus.identity?.stationId || '已绑定')
    : '未绑定';
  const stationRoleLabel = '工位';

  const accounts = Array.isArray(stationStatus.platformAccounts) ? stationStatus.platformAccounts : [];
  const healthyCount = accounts.filter((a) => a.healthStatus === 'healthy').length;
  const stationTaskHint = `已发现 ${healthyCount} 个可用账号。这个浏览器会按任务优先级接单，人工下单会优先于监控任务。`;
  const diagnostics = stationStatus.diagnostics || {};
  const activeLocks = Array.isArray(diagnostics.activeLocks) ? diagnostics.activeLocks : [];
  const unsentOutboxCount = Number(diagnostics.unsentOutboxCount || 0);
  const outboxStatusCounts = diagnostics.outboxStatusCounts || null;
  const deadLetterCount = Number(outboxStatusCounts?.deadLetter || 0);
  const oldestDeadLetterAt = Number(outboxStatusCounts?.oldestDeadLetterCreatedAt || 0);
  const accountDiagnostics = accounts.map(formatAccountDiagnostics);
  const diagnosticItems = [
    { label: '插件版本', value: stationStatus.pluginVersion || '未知' },
    { label: '工位编号', value: shortId(stationStatus.identity?.stationId) || '未绑定' },
    { label: '当前任务', value: formatCurrentTask(diagnostics.currentTask) },
    { label: '本机锁', value: formatLockSummary(activeLocks) },
    // 待发送 = 仍会自动重试的记录；死信 = 已永久停发、需人工恢复的记录。
    // 两者必须分开呈现（2026-07-13 事故：死信混入待发送导致 2144 无法解释）。
    { label: '待发送', value: `${unsentOutboxCount} 条` },
    {
      label: '死信（不再自动发送）',
      value: deadLetterCount > 0
        ? `${deadLetterCount} 条${oldestDeadLetterAt ? `，最早 ${new Date(oldestDeadLetterAt).toLocaleString()}` : ''}`
        : '0 条',
      alert: deadLetterCount > 0,
    },
  ];
  const shouldShowDiagnostics = Boolean(stationStatus.pluginVersion || stationStatus.registered || accountDiagnostics.length);
  const presets = [
    { key: 'production', label: '线上正式站', url: presetUrls.production || 'https://lingganboom.fun' },
    { key: 'local', label: '本地 3000', url: presetUrls.local || 'http://localhost:3000' },
  ];
  const normalizeUrl = (value = '') => String(value || '').trim().replace(/\/+$/, '');
  const currentUrl = normalizeUrl(flywheelUrl);

  return (
    <div className="flywheel-section">
      <div className="context-section flywheel-card">
        <div className="section-heading">
          <h2>内容工作台</h2>
          <div className="flywheel-heading-side">
            <div className="flywheel-preset-row">
              {presets.map((preset) => (
                <button
                  key={preset.key}
                  type="button"
                  className={`flywheel-preset-chip${currentUrl === normalizeUrl(preset.url) ? ' active' : ''}`}
                  onClick={() => {
                    if (typeof onUsePresetUrl === 'function') {
                      void onUsePresetUrl(preset.url);
                      return;
                    }
                    onUrlChange?.(preset.url);
                  }}
                >
                  {preset.label}
                </button>
              ))}
            </div>
            <span id="flywheelStatus" className={`flywheel-status ${flywheelStatus}`}>
              {statusText}
            </span>
          </div>
        </div>
        <div className="flywheel-url-row">
          <input
            id="flywheelUrl"
            type="text"
            className="flywheel-input"
            placeholder="https://lingganboom.fun"
            value={flywheelUrl}
            onChange={(e) => onUrlChange(e.target.value)}
          />
          <button
            id="btnFlywheelTest"
            className={`popup-btn outline small${testing ? ' is-busy' : ''}`}
            onClick={onTest}
            disabled={testing}
          >
            {testing ? '测试中...' : '测试连接'}
          </button>
        </div>
        <p className="station-hint">
          支持线上正式站和本地 `3000`，插件会记住你最近一次使用的地址。
        </p>
      </div>

      <div className="station-panel">
        <div className="station-title-row">
          <span style={{ fontSize: '12px', fontWeight: 900 }}>插件授权</span>
          <span id="pluginAuthorizationStatus" className={`flywheel-status ${authorizationState}`}>
            {authorizationLabel}
          </span>
        </div>
        <p id="pluginAuthorizationHint" className="station-hint">
          {authorizationHint}
        </p>
        <div className="flywheel-url-row">
          <input
            id="pluginAuthorizationCode"
            type="text"
            className="flywheel-input"
            placeholder="输入授权码"
            value={authorizationCode}
            onChange={(e) => onAuthorizationCodeChange(e.target.value)}
            disabled={authorizing || clearingAuthorization}
          />
          <button
            id="btnPluginAuthorize"
            className={`popup-btn primary small${authorizing ? ' is-busy' : ''}`}
            onClick={onAuthorize}
            disabled={authorizing || clearingAuthorization}
          >
            {authorizing ? '连接中...' : '连接插件'}
          </button>
          {authorizationStatus.authorized && typeof onClearAuthorization === 'function' ? (
            <button
              id="btnPluginAuthorizationClear"
              className={`popup-btn outline small${clearingAuthorization ? ' is-busy' : ''}`}
              onClick={onClearAuthorization}
              disabled={authorizing || clearingAuthorization}
            >
              {clearingAuthorization ? '清除中...' : '清除'}
            </button>
          ) : null}
        </div>
        {canRequestAuthorization || canClaimAuthorization ? (
          <div className="flywheel-url-row">
            {canRequestAuthorization ? (
              <button
                id="btnPluginAuthorizationRequest"
                className={`popup-btn outline small${requestingAuthorization ? ' is-busy' : ''}`}
                onClick={onRequestAuthorization}
                disabled={requestingAuthorization || authorizing || clearingAuthorization}
              >
                {requestingAuthorization ? '发送中...' : '发起授权申请'}
              </button>
            ) : null}
            {canClaimAuthorization ? (
              <button
                id="btnPluginAuthorizationClaim"
                className={`popup-btn primary small${claimingAuthorization ? ' is-busy' : ''}`}
                onClick={onClaimAuthorization}
                disabled={claimingAuthorization || authorizing || clearingAuthorization}
              >
                {claimingAuthorization ? '检查中...' : '检查审批结果'}
              </button>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="station-panel">
        <div className="station-title-row">
          <span style={{ fontSize: '12px', fontWeight: 900 }}>工位</span>
          <span id="stationStatus" className={`flywheel-status ${stationStatus.registered ? 'connected' : 'unconfigured'}`}>
            {stationStatus.registered ? `${stationName} · ${stationRoleLabel}` : stationName}
          </span>
        </div>
        <p id="stationHint" className="station-hint">
          {!authorizationStatus.authorized
            ? '先连接插件；授权成功后会自动创建工位。'
            : stationStatus.registered
            ? (healthyCount > 0
              ? stationTaskHint
              : `已绑定${stationRoleLabel}；请先在 Cookie & 账号里保存可用账号，才能领取任务。`)
            : '工位正在等待工作台返回身份；重新点击连接插件或检查审批结果即可自动补齐。'}
        </p>
        {shouldShowDiagnostics ? (
          <div className="station-diagnostics" aria-label="工位诊断">
            <div className="station-diagnostic-grid">
              {diagnosticItems.map((item) => (
                <div className="station-diagnostic-item" key={item.label}>
                  <span>{item.label}</span>
                  <strong title={item.value} style={item.alert ? { color: '#d03050' } : undefined}>{item.value}</strong>
                </div>
              ))}
            </div>
            {deadLetterCount > 0 ? (
              <div className="station-diagnostic-item">
                <span>这些数据不会再自动发送，可导出后走恢复通道找回</span>
                <button
                  type="button"
                  className={`popup-btn outline small${exportingRecovery ? ' is-busy' : ''}`}
                  disabled={exportingRecovery}
                  onClick={handleExportRecovery}
                >
                  {exportingRecovery ? '导出中...' : '导出恢复数据'}
                </button>
              </div>
            ) : null}
            {accountDiagnostics.length ? (
              <div className="station-runtime-list">
                {accountDiagnostics.map((item) => (
                  <div className="station-runtime-row" key={`${item.title}:${item.detail}`}>
                    <span>{item.title}</span>
                    <strong>{item.detail}</strong>
                  </div>
                ))}
              </div>
            ) : (
              <p className="station-hint">暂无可展示的账号状态；保存账号后这里会显示登录与权限状态。</p>
            )}
          </div>
        ) : null}
      </div>

      <button
        id="btnSyncToFlywheel"
        className={`popup-btn primary${syncing ? ' is-busy' : ''}`}
        onClick={onSync}
        disabled={!authorizationStatus.authorized || syncing}
      >
        {syncing ? '同步中...' : '全部同步到工作台'}
      </button>
    </div>
  );
}
