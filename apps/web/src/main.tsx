import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
// LIDS Token 与共享外壳样式的唯一来源（DEC-0010、WEB-REACT-001 §4.5 / R4）：直接引用，不复制第二份。
import '../../api/src/local_web/lids_tokens.css';
import '../../api/src/local_web/shell.css';
import { App } from './app/App';

const container = document.getElementById('root');
if (!container) throw new Error('页面缺少挂载点 #root');

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
