// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { APP_ROUTES } from '@typvia/shared';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
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
  it('renders the three navigation destinations, rooms behind the switcher', async () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    await screen.findByRole('button', { name: 'Home' });
    expect(screen.getByRole('button', { name: 'Settings' })).toBeDefined();
    const workspace = screen.getByRole('button', { name: 'Workspace' });
    expect(workspace.getAttribute('aria-expanded')).toBe('false');
    // The rooms stay folded until the switcher is opened.
    expect(screen.queryByRole('button', { name: /Vault/ })).toBeNull();
    fireEvent.click(workspace);
    for (const label of [/Library/, /Vault/, /AI Actions/, /Trash/]) {
      expect(screen.getByRole('menuitem', { name: label })).toBeDefined();
    }
    // The editor is not a nav destination: it opens from a
    // snippet row, the Library's New button, or ⌘N.
    expect(screen.queryByRole('menuitem', { name: /Snippet editor/ })).toBeNull();
    // Sync lives under Settings, not in the top navigation.
    expect(screen.queryByRole('menuitem', { name: /Sync & devices/ })).toBeNull();
  });

  it('offers first-run onboarding until its marker exists', async () => {
    onboardingStatusMock.mockResolvedValueOnce({ completed: false });
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByRole('button', { name: 'Skip setup' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Workspace' })).toBeNull();
  });

  it('navigates when a room is picked and the switcher takes its name', async () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    fireEvent.click(await screen.findByRole('button', { name: 'Workspace' }));
    fireEvent.click(screen.getByRole('menuitem', { name: /Vault/ }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Vault' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Vault' })).toBeDefined();
  });

  it('reaches the real Library screen at /library', async () => {
    render(
      <MemoryRouter initialEntries={['/library']}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByLabelText('Search snippets')).toBeDefined();
  });

  it('reaches the real editor screen at /editor', async () => {
    render(
      <MemoryRouter initialEntries={['/editor']}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByLabelText('Snippet title')).toBeDefined();
  });

  it('reaches the real Home screen at /', async () => {
    render(
      <MemoryRouter initialEntries={['/']}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByLabelText('Search snippets')).toBeDefined();
    expect(screen.getByText('What are you looking for?')).toBeDefined();
  });

  it('reaches the real Trash screen at /trash', async () => {
    render(
      <MemoryRouter initialEntries={['/trash']}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByText('The trash is clean.')).toBeDefined();
  });

  it('reaches the real Vault screen at /vault', async () => {
    render(
      <MemoryRouter initialEntries={['/vault']}>
        <App />
      </MemoryRouter>,
    );
    // The unlocked vault renders its page heading and the titles-only note.
    expect(await screen.findByRole('heading', { level: 1, name: 'Vault' })).toBeDefined();
    expect(
      await screen.findByText('Titles only — secret contents never enter the search index.'),
    ).toBeDefined();
  });

  it.each(
    APP_ROUTES.filter(
      (route) =>
        route.path !== '/library' &&
        route.path !== '/editor' &&
        route.path !== '/' &&
        route.path !== '/trash' &&
        route.path !== '/vault' &&
        route.path !== '/sync' &&
        // Settings is a real page with its own heading ("Preferences") and
        // suite (settings-page.test.tsx), not a placeholder.
        route.path !== '/settings',
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
