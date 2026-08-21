// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { APP_ROUTES, onboardingStatus } from '@typvia/shared';
import { Page, PlaceholderPage } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { Route, Routes } from 'react-router';
import { BrowserIntegrationProvider } from './browser-integration/browser-integration-context';
import { EspansoProvider } from './espanso/espanso-context';
import { ActionsPage } from './pages/actions';
import { EditorPage, HistoryPage } from './pages/editor';
import { HomePage } from './pages/home';
import { LibraryPage } from './pages/library';
import { OnboardingPage } from './pages/onboarding';
import { SettingsPage } from './pages/settings';
import { ConflictPage, PairPage, RecoverPage, RecoveryPage, SyncPage } from './pages/sync';
import { TemplateBuilder, TemplatesPage } from './pages/templates';
import { TrashPage } from './pages/trash';
import { VaultPage } from './pages/vault';
import { VaultProvider } from './vault/vault-context';
import { WorkspaceShell } from './workspace/shell';

// App shell: the workspace header (context navigation + ⌘K palette) over the
// routed pages.
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
              <Route path="/editor/:id" element={<EditorPage />} />
              <Route path="/sync/pair" element={<PairPage />} />
              <Route path="/sync/conflicts" element={<ConflictPage />} />
              <Route path="/sync/recovery" element={<RecoveryPage />} />
              <Route path="/sync/recover" element={<RecoverPage />} />
              <Route path="/editor/:id/history" element={<HistoryPage />} />
              <Route path="/templates/:id" element={<TemplateBuilder />} />
              {APP_ROUTES.map((route) =>
                route.path === '/library' ? (
                  <Route key={route.path} path={route.path} element={<LibraryPage />} />
                ) : route.path === '/editor' ? (
                  <Route key={route.path} path={route.path} element={<EditorPage />} />
                ) : route.path === '/' ? (
                  <Route key={route.path} path={route.path} element={<HomePage />} />
                ) : route.path === '/trash' ? (
                  <Route key={route.path} path={route.path} element={<TrashPage />} />
                ) : route.path === '/settings' ? (
                  <Route key={route.path} path={route.path} element={<SettingsPage />} />
                ) : route.path === '/templates' ? (
                  <Route key={route.path} path={route.path} element={<TemplatesPage />} />
                ) : route.path === '/vault' ? (
                  <Route key={route.path} path={route.path} element={<VaultPage />} />
                ) : route.path === '/sync' ? (
                  <Route key={route.path} path={route.path} element={<SyncPage />} />
                ) : route.path === '/ai' ? (
                  <Route key={route.path} path={route.path} element={<ActionsPage />} />
                ) : (
                  <Route
                    key={route.path}
                    path={route.path}
                    element={
                      <Page>
                        <PlaceholderPage labelEn={route.labelEn} labelCn={route.labelCn} />
                      </Page>
                    }
                  />
                ),
              )}
            </Routes>
          </WorkspaceShell>
        </VaultProvider>
      </EspansoProvider>
    </BrowserIntegrationProvider>
  );
}
