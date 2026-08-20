import { applyStoredUiPrefs, UiPrefsProvider } from '@typvia/ui';
import '@typvia/ui/base.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { installAndroidBackBridge, isAndroid, syncAndroidSafeArea } from './platform';

// First paint already carries the stored theme and language; UiPrefsProvider
// keeps them live (including the OS appearance flips `system` follows).
applyStoredUiPrefs(document.documentElement);

// Android host glue (difference layer): system-back routing and the
// host-measured safe-area insets. iOS needs neither — env() reports the real
// insets there and iOS has no system back gesture to route.
if (isAndroid) {
  installAndroidBackBridge();
  syncAndroidSafeArea();
}

const root = document.getElementById('root');
if (!root) {
  throw new Error('missing #root element');
}
createRoot(root).render(
  <StrictMode>
    <UiPrefsProvider>
      <App />
    </UiPrefsProvider>
  </StrictMode>,
);
