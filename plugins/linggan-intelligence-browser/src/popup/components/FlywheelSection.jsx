import React from 'react';

const LOCAL_ORIGIN = 'http://localhost:3000';

export default function FlywheelSection({
  flywheelStatus,
  testing = false,
  onTest,
}) {
  const statusText = {
    unconfigured: '未检查',
    configured: '本机地址已保存',
    testing: '检查中…',
    connected: '本机可访问',
    disconnected: '本机不可访问',
  }[flywheelStatus] || '待检查';

  return (
    <div className="flywheel-section">
      <div className="context-section flywheel-card">
        <div className="section-heading">
          <h2>Linggan Intelligence</h2>
          <div className="flywheel-heading-side">
            <div className="flywheel-preset-row"><span className="flywheel-preset-chip active">本机 3000</span></div>
            <span id="flywheelStatus" className={`flywheel-status ${flywheelStatus}`}>{statusText}</span>
          </div>
        </div>
        <div className="flywheel-url-row">
          <output id="flywheelUrl" className="flywheel-input" aria-label="Linggan 本机地址">{LOCAL_ORIGIN}</output>
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
          <span id="pluginAuthorizationStatus" className="flywheel-status configured">LOCAL_TRUSTED</span>
        </div>
        <p id="pluginAuthorizationHint" className="station-hint">测试阶段仅向本机 Linggan 交付已读取的页面材料；不使用内容工作台账号、工位、租约或人工授权。</p>
        <div className="station-diagnostics" aria-label="Linggan adapter status">
          <div className="station-diagnostic-grid">
            <div className="station-diagnostic-item"><span>迁入界面</span><strong>已保留</strong></div>
            <div className="station-diagnostic-item"><span>旧工作台连接</span><strong>已切断</strong></div>
            <div className="station-diagnostic-item"><span>执行模式</span><strong>手动采集</strong></div>
            <div className="station-diagnostic-item"><span>交付方式</span><strong>本机回执</strong></div>
          </div>
        </div>
      </div>

      <div className="station-panel">
        <div className="station-title-row">
          <span style={{ fontSize: '12px', fontWeight: 900 }}>本机暂存与恢复</span>
          <span id="stationStatus" className="flywheel-status unconfigured">浏览器本地</span>
        </div>
        <p id="stationHint" className="station-hint">
          原插件的本地暂存、恢复、Dashboard 与页面控件仍随包保留；待交付不等于 Linggan 已接纳，交付回执才决定最终状态。
        </p>
      </div>
    </div>
  );
}
