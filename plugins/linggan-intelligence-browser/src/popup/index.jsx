import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';

class PopupStartupBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = { failed: false };
  }

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    if (this.state.failed) {
      return (
        <main className="popup-startup-failure" aria-live="polite">
          <p className="popup-startup-failure-eyebrow">LINGGAN / v0.4.1</p>
          <h1>插件界面未能启动</h1>
          <p>本机状态目前未知。本次没有读取、采集或传输任何数据。</p>
          <p>请在 Chrome 的“扩展程序”页面重新加载 Linggan Intelligence Browser v0.4.1，然后再打开此窗口。</p>
        </main>
      );
    }
    return this.props.children;
  }
}

const container = document.getElementById('root');
if (container) {
  const root = createRoot(container);
  root.render(
    <PopupStartupBoundary>
      <App />
    </PopupStartupBoundary>,
  );
}
