import React from 'react';

const LOCAL_ORIGIN = 'http://localhost:3000';

export default function FlywheelSection({
  flywheelUrl,
  flywheelStatus,
  stationStatus = {},
  testing = false,
  onUrlChange,
  onTest,
}) {
  const statusText = {
    unconfigured: '未检查',
    configured: '本机地址已保存',
    testing: '检查中…',
    connected: '本机可访问',
    disconnected: '本机不可访问',
  }[flywheelStatus] || '待检查';
  const readinessHint = stationStatus.authorizationMessage
    || 'Linggan 的真实采集接收合同尚未逐项接通；此页没有执行任何平台访问。';

  return (
    <div className="flywheel-section">
      <div className="context-section flywheel-card">
        <div className="section-heading">
          <h2>Linggan Intelligence</h2>
          <div className="flywheel-heading-side">
            <div className="flywheel-preset-row">
              <button
                type="button"
                className="flywheel-preset-chip active"
                onClick={() => onUrlChange?.(LOCAL_ORIGIN)}
              >
                本机 3000
              </button>
            </div>
            <span id="flywheelStatus" className={`flywheel-status ${flywheelStatus}`}>{statusText}</span>
          </div>
        </div>
        <div className="flywheel-url-row">
          <input
            id="flywheelUrl"
            type="text"
            className="flywheel-input"
            value={flywheelUrl || LOCAL_ORIGIN}
            onChange={(event) => onUrlChange?.(event.target.value)}
            aria-label="Linggan 本机地址"
          />
          <button
            id="btnFlywheelTest"
            className={`popup-btn outline small${testing ? ' is-busy' : ''}`}
            onClick={onTest}
            disabled={testing}
          >
            {testing ? '检查中…' : '检查连接'}
          </button>
        </div>
        <p className="station-hint">旧插件的完整操作界面已迁入 Linggan；这里仅连接本机 Linggan，不会再连接内容工作台。</p>
      </div>

      <div className="station-panel">
        <div className="station-title-row">
          <span style={{ fontSize: '12px', fontWeight: 900 }}>Linggan 适配状态</span>
          <span id="pluginAuthorizationStatus" className="flywheel-status unconfigured">待逐项接通</span>
        </div>
        <p id="pluginAuthorizationHint" className="station-hint">{readinessHint}</p>
        <div className="station-diagnostics" aria-label="Linggan adapter status">
          <div className="station-diagnostic-grid">
            <div className="station-diagnostic-item"><span>迁入界面</span><strong>已保留</strong></div>
            <div className="station-diagnostic-item"><span>旧工作台连接</span><strong>已切断</strong></div>
            <div className="station-diagnostic-item"><span>自动任务/工位</span><strong>未接通</strong></div>
            <div className="station-diagnostic-item"><span>真实平台采集</span><strong>未授权执行</strong></div>
          </div>
        </div>
      </div>

      <div className="station-panel">
        <div className="station-title-row">
          <span style={{ fontSize: '12px', fontWeight: 900 }}>本机暂存与恢复</span>
          <span id="stationStatus" className="flywheel-status unconfigured">浏览器本地</span>
        </div>
        <p id="stationHint" className="station-hint">
          原插件的本地暂存、恢复、Dashboard 与页面控件仍随包保留；它们不是 Linggan 的最终事实库，也不会自动同步。
        </p>
      </div>
    </div>
  );
}
