import React from 'react';

export default class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error) {
    return { hasError: true, error };
  }

  componentDidCatch(error, errorInfo) {
    console.error('[Dashboard ErrorBoundary]', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="dashboard-error-fallback" style={{ padding: 24, textAlign: 'center' }}>
          <h2 style={{ marginBottom: 12 }}>数据加载出错</h2>
          <p style={{ marginBottom: 16, color: '#666' }}>请刷新页面重试。如果问题持续，请联系支持。</p>
          <button
            onClick={() => window.location.reload()}
            style={{ padding: '8px 16px', cursor: 'pointer' }}
          >
            刷新页面
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
