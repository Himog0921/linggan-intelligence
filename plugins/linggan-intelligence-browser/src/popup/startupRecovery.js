// 弹窗界面起不来时的兜底说明。
//
// 这段话里唯一的机器事实是**版本号**，而它此前是写死的：界面会说着一个早就不是本机的
// 版本（写这句话时是 v0.4.8，实际已装的是 0.8.54）。人在「扩展程序」页面找版本号时，
// 看到的会是两个不同的数字，于是不知道该信哪个。
//
// 版本取自本窗口所属扩展自己的清单（`chrome.runtime.getManifest()`）——它就是 Chrome
// 正在运行的那一份，因此不会过期。取不到就说不知道：不拿写死的数字冒充，也不因为读不到
// 就干脆不提示人重载。

/// 界面显示给使用者看的版本标签。读不到清单时是 `v未知`。
export function popupPluginVersionLabel() {
  try {
    const version = String(globalThis.chrome?.runtime?.getManifest?.()?.version || '').trim();
    // 只接受版本号的形状（`0.8.54`、`1.2.3.4`）；别的一律说不知道。
    return /^[0-9]+(?:\.[0-9]+){1,3}$/.test(version) ? `v${version}` : 'v未知';
  } catch {
    return 'v未知';
  }
}

/// 这段说明是**每次渲染时**算出来的，不是模块加载时定死的：版本以当下加载的那一份为准。
export function popupStartupRecovery() {
  const version = popupPluginVersionLabel();
  return Object.freeze({
    version,
    title: '插件界面未能启动',
    state: '本机状态目前未知，无法确认是否已读取页面信息。',
    effect: '本提示没有发起新的采集或传输。',
    recovery: `请在 Chrome 的“扩展程序”页面重新加载 Linggan Intelligence Browser（本机已安装 ${version}），然后再打开此窗口。`,
  });
}

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
      const recovery = popupStartupRecovery();
      return React.createElement(
        'main',
        { className: 'popup-startup-failure', 'aria-live': 'polite' },
        React.createElement('p', { className: 'popup-startup-failure-eyebrow' }, `LINGGAN / ${recovery.version}`),
        React.createElement('h1', null, recovery.title),
        React.createElement('p', null, recovery.state),
        React.createElement('p', null, recovery.effect),
        React.createElement('p', null, recovery.recovery),
      );
    }
  };
}
