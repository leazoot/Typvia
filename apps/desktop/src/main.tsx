import { getCurrentWindow } from '@tauri-apps/api/window';
import { followSystemTheme } from '@typvia/ui';
import '@typvia/ui/base.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router';
import { App } from './App';
import { PANEL_WINDOW_LABEL, PanelApp } from './pages/panel';

followSystemTheme(document.documentElement);

const container = document.getElementById('root');
if (container === null) {
  throw new Error('missing #root element in index.html');
}

// The resident panel window (tauri.conf.json) loads the same bundle; it mounts
// only the command panel, never the full navigation shell.
const isPanel = getCurrentWindow().label === PANEL_WINDOW_LABEL;

createRoot(container).render(
  <StrictMode>
    {isPanel ? (
      <PanelApp />
    ) : (
      <BrowserRouter>
        <App />
      </BrowserRouter>
    )}
  </StrictMode>,
);
