// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { onboardingStatus } from '@typvia/shared';
import { useEffect, useState } from 'react';
import { Route, Routes } from 'react-router';
import { BrowserIntegrationProvider } from './browser-integration/browser-integration-context';
import { EspansoProvider } from './espanso/espanso-context';
import { ActionsPage } from './pages/actions';
import { EditorPage, HistoryPage } from './pages/editor';
import { LibraryPage } from './pages/library';
import { OnboardingPage } from './pages/onboarding';
import { SettingsPage } from './pages/settings';
import { ShortcutsPage } from './pages/shortcuts';
import { ConflictPage, PairPage, RecoverPage, RecoveryPage, SyncPage } from './pages/sync';
import { TemplateBuilder, TemplatesPage } from './pages/templates';
import { TrashPage } from './pages/trash';
import { VaultPage } from './pages/vault';
import { VaultProvider } from './vault/vault-context';
import { WorkspaceShell } from './workspace/shell';

// App shell: the window title bar, the menu bar that navigates and the ⌘K
// palette, over the routed pages. The Library is the main window at the root.
export function App() {
  // null = status pending (nothing rendered — the flow decision is a cut, not
  // a flash of nav). A failed read fails open into the shell: the app staying
  // usable matters more than re-offering setup.
  const [onboarded, setOnboarded] = useState<boolean | null>(null);
  useEffect(() => {
    onboardingStatus()
      .then((status) => setOnboarded(status.completed))
      .catch(() => setOnboarded(true));
  }, []);

  if (onboarded === null) return null;
  if (!onboarded) return <OnboardingPage onDone={() => setOnboarded(true)} />;
  return (
    <BrowserIntegrationProvider>
      <EspansoProvider>
        <VaultProvider>
          <WorkspaceShell>
            <Routes>
              <Route path="/" element={<LibraryPage />} />
              <Route path="/editor" element={<EditorPage />} />
              <Route path="/editor/:id" element={<EditorPage />} />
              <Route path="/editor/:id/history" element={<HistoryPage />} />
              <Route path="/vault" element={<VaultPage />} />
              <Route path="/trash" element={<TrashPage />} />
              <Route path="/ai" element={<ActionsPage />} />
              <Route path="/templates" element={<TemplatesPage />} />
              <Route path="/templates/:id" element={<TemplateBuilder />} />
              <Route path="/sync" element={<SyncPage />} />
              <Route path="/sync/pair" element={<PairPage />} />
              <Route path="/sync/conflicts" element={<ConflictPage />} />
              <Route path="/sync/recovery" element={<RecoveryPage />} />
              <Route path="/sync/recover" element={<RecoverPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/shortcuts" element={<ShortcutsPage />} />
            </Routes>
          </WorkspaceShell>
        </VaultProvider>
      </EspansoProvider>
    </BrowserIntegrationProvider>
  );
}
