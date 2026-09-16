// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { APP_ROUTES } from '@typvia/shared';
import { cleanup, render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { App } from './App';

const onboardingStatusMock = vi.fn<() => Promise<{ completed: boolean }>>();
onboardingStatusMock.mockResolvedValue({ completed: true });

// IPC is mocked at the typed wrapper layer (frontend testing rule) so the
// real Library screen can mount without a Tauri host.
vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    onboardingStatus: () => onboardingStatusMock(),
    onboardingComplete: () => Promise.resolve(),
    libraryCounts: () =>
      Promise.resolve({ total: 0, recent: 0, starred: 0, unsorted: 0, trash: 0, folders: [] }),
    countSnippets: () => Promise.resolve(0),
    listSnippetPage: () => Promise.resolve([]),
    listFolderChildren: () => Promise.resolve([]),
    listTags: () => Promise.resolve([]),
    syncStatus: () => Promise.reject(new Error('no host in tests')),
    templateVariables: () => Promise.resolve([]),
    detectSensitive: () => Promise.resolve([]),
    listTrash: () => Promise.resolve([]),
    purgeExpiredTrash: () => Promise.resolve(0),
    vaultStatus: () =>
      Promise.resolve({
        initialized: true,
        unlocked: true,
        unlockedAt: 1000,
        lastActivityAt: 1000,
        idleTimeoutMs: 300000,
      }),
    vaultList: () => Promise.resolve([]),
  };
});

afterEach(cleanup);

describe('app shell routing', () => {
  it('opens on the Library as the main window, with the places in the title bar', async () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByLabelText('Search snippets')).toBeDefined();
    expect(screen.getByRole('radio', { name: /All snippets/ })).toBeDefined();
    const places = screen.getByRole('navigation', { name: 'Places' });
    expect(
      within(places).getByRole('button', { name: 'Snippets' }).getAttribute('aria-current'),
    ).toBe('page');
    expect(screen.queryByText('Typvia — Library')).toBeNull();
  });

  it('offers first-run onboarding until its marker exists', async () => {
    onboardingStatusMock.mockResolvedValueOnce({ completed: false });
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByRole('button', { name: 'Skip setup' })).toBeDefined();
    expect(screen.queryByLabelText('Search snippets')).toBeNull();
  });

  it('reaches the real editor screen at /editor', async () => {
    render(
      <MemoryRouter initialEntries={['/editor']}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByLabelText('Snippet title')).toBeDefined();
  });

  it('reaches the real Trash screen at /trash', async () => {
    render(
      <MemoryRouter initialEntries={['/trash']}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByRole('heading', { name: 'The trash is empty.' })).toBeDefined();
  });

  it('reaches the real Vault screen at /vault', async () => {
    render(
      <MemoryRouter initialEntries={['/vault']}>
        <App />
      </MemoryRouter>,
    );
    // The unlocked vault counts down to its relock and says it is empty.
    expect(await screen.findByText(/^Unlocked \d+:\d\d$/)).toBeDefined();
    expect(await screen.findByText(/^The vault is empty\./)).toBeDefined();
  });

  it('reaches the real AI actions page at /ai', async () => {
    render(
      <MemoryRouter initialEntries={['/ai']}>
        <App />
      </MemoryRouter>,
    );
    // No host in tests: the page says the actions are safe and offers a retry.
    expect(await screen.findByRole('heading', { name: 'Your actions are safe.' })).toBeDefined();
  });

  it.each(
    APP_ROUTES.filter(
      (route) =>
        route.path !== '/editor' &&
        route.path !== '/' &&
        route.path !== '/trash' &&
        route.path !== '/vault' &&
        route.path !== '/sync' &&
        // Settings and AI actions are real pages with their own suites, not
        // placeholders.
        route.path !== '/settings' &&
        route.path !== '/ai',
    ).map((route) => [route.labelEn, route]),
  )('reaches the %s placeholder page at its route', async (_label, route) => {
    render(
      <MemoryRouter initialEntries={[route.path]}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByRole('heading', { level: 1, name: route.labelEn })).toBeDefined();
  });
});
