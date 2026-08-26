export const POPUP_STARTUP_RECOVERY = Object.freeze({
  version: 'v0.4.5',
  title: '插件界面未能启动',
  state: '本机状态目前未知，无法确认是否已读取页面信息。',
  effect: '本提示没有发起新的采集或传输。',
  recovery: '请在 Chrome 的“扩展程序”页面重新加载 Linggan Intelligence Browser v0.4.5，然后再打开此窗口。',
});

export function createPopupStartupBoundary(React) {
  return class PopupStartupBoundary extends React.Component {
    constructor(props) {
      super(props);
      this.state = { failed: false };
    }

    static getDerivedStateFromError() {
      return { failed: true };
    }

    render() {
      if (!this.state.failed) return this.props.children;
      return React.createElement(
        'main',
        { className: 'popup-startup-failure', 'aria-live': 'polite' },
        React.createElement('p', { className: 'popup-startup-failure-eyebrow' }, `LINGGAN / ${POPUP_STARTUP_RECOVERY.version}`),
        React.createElement('h1', null, POPUP_STARTUP_RECOVERY.title),
        React.createElement('p', null, POPUP_STARTUP_RECOVERY.state),
        React.createElement('p', null, POPUP_STARTUP_RECOVERY.effect),
        React.createElement('p', null, POPUP_STARTUP_RECOVERY.recovery),
      );
    }
  };
}
