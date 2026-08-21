// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { getCurrentWindow } from '@tauri-apps/api/window';
import { applyStoredUiPrefs, UiPrefsProvider } from '@typvia/ui';
import '@typvia/ui/base.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router';
import { App } from './App';
import { installBackspaceGuard } from './backspace-guard';
import { PANEL_WINDOW_LABEL, PanelApp } from './pages/panel';

// First paint already carries the stored theme and language; UiPrefsProvider
// keeps them live (including cross-window changes) from mount on.
applyStoredUiPrefs(document.documentElement);
installBackspaceGuard(window);

const container = document.getElementById('root');
if (container === null) {
  throw new Error('missing #root element in index.html');
}

// The resident panel window (tauri.conf.json) loads the same bundle; it mounts
// only the command panel, never the full navigation shell.
const isPanel = getCurrentWindow().label === PANEL_WINDOW_LABEL;

// The panel window is transparent: its page background must stay
// clear so only the scrim and the floating card paint. The class scopes the
// override to this window — the main window keeps the paper base.
if (isPanel) {
  document.documentElement.classList.add('tv-window-panel');
}

createRoot(container).render(
  <StrictMode>
    <UiPrefsProvider>
      {isPanel ? (
        <PanelApp />
      ) : (
        <BrowserRouter>
          <App />
        </BrowserRouter>
      )}
    </UiPrefsProvider>
  </StrictMode>,
);
