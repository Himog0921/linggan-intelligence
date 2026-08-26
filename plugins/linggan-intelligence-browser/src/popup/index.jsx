import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';
import { createPopupStartupBoundary } from './startupRecovery.js';

const PopupStartupBoundary = createPopupStartupBoundary(React);

const container = document.getElementById('root');
if (container) {
  const root = createRoot(container);
  root.render(
    <PopupStartupBoundary>
      <App />
    </PopupStartupBoundary>,
  );
}
