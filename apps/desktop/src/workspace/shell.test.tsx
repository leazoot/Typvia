// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { afterEach, describe, expect, it } from 'vitest';
import { WorkspaceShell } from './shell';

function Probe() {
  const location = useLocation();
  return <p>at {location.pathname}</p>;
}

function renderShell(path: string | string[] = '/') {
  const entries = typeof path === 'string' ? [path] : path;
  return render(
    <MemoryRouter initialEntries={entries} initialIndex={entries.length - 1}>
      <WorkspaceShell>
        <Routes>
          <Route path="*" element={<Probe />} />
        </Routes>
      </WorkspaceShell>
    </MemoryRouter>,
  );
}

afterEach(cleanup);

describe('window title bar', () => {
  it('carries the places and the summon shortcut, with no room title', () => {
    renderShell('/');
    const bar = screen.getByRole('banner');
    expect(within(bar).getByText('⌘⇧V')).toBeDefined();
    expect(within(bar).queryByText('Typvia — Library')).toBeNull();
    const snippets = within(bar).getByRole('button', { name: 'Snippets' });
    expect(snippets.getAttribute('aria-current')).toBe('page');
  });

  it('keeps a place marked on the pages that belong to it', () => {
    renderShell('/sync/pair');
    const bar = screen.getByRole('banner');
    expect(within(bar).getByRole('button', { name: 'Settings' }).getAttribute('aria-current')).toBe(
      'page',
    );
    expect(within(bar).getByRole('button', { name: 'Snippets' }).getAttribute('aria-current')).toBe(
      null,
    );
  });

  it('goes straight to a place on click, without a menu', async () => {
    renderShell('/');
    fireEvent.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Vault' }));
    expect(await screen.findByText('at /vault')).toBeDefined();
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('offers no way back on the library itself', () => {
    renderShell('/');
    expect(within(screen.getByRole('banner')).queryByRole('button', { name: 'Back' })).toBeNull();
  });

  it('steps back to the page the window came from', async () => {
    renderShell(['/trash', '/settings']);
    fireEvent.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Back' }));
    expect(await screen.findByText('at /trash')).toBeDefined();
  });

  it('goes to the library from a page with nothing behind it', async () => {
    renderShell('/settings');
    fireEvent.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Back' }));
    expect(await screen.findByText('at /')).toBeDefined();
  });
});

describe('workspace keyboard map', () => {
  it('jumps to a room with its ⌘-digit and to settings with ⌘,', async () => {
    renderShell('/');
    fireEvent.keyDown(document, { key: '2', metaKey: true });
    expect(await screen.findByText('at /vault')).toBeDefined();
    fireEvent.keyDown(document, { key: '1', metaKey: true });
    expect(await screen.findByText('at /')).toBeDefined();
    fireEvent.keyDown(document, { key: ',', metaKey: true });
    expect(await screen.findByText('at /settings')).toBeDefined();
  });

  it('opens the editor with ⌘⇧N and the trash with ⌘⇧⌫', async () => {
    renderShell('/');
    fireEvent.keyDown(document, { key: 'N', metaKey: true, shiftKey: true });
    expect(await screen.findByText('at /editor')).toBeDefined();
    fireEvent.keyDown(document, { key: 'Backspace', metaKey: true, shiftKey: true });
    expect(await screen.findByText('at /trash')).toBeDefined();
  });
});

describe('command palette', () => {
  it('opens with ⌘K, filters, and navigates', async () => {
    renderShell('/');
    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Search Typvia' });
    expect(within(palette).getByRole('button', { name: /Trash/ })).toBeDefined();

    fireEvent.change(within(palette).getByPlaceholderText('Search Typvia…'), {
      target: { value: 'new' },
    });
    expect(within(palette).queryByRole('button', { name: /Trash/ })).toBeNull();
    fireEvent.click(within(palette).getByRole('button', { name: /New snippet/ }));
    expect(await screen.findByText('at /editor')).toBeDefined();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('runs the highlighted entry on Enter and closes on Escape', async () => {
    renderShell('/');
    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    const field = await screen.findByPlaceholderText('Search Typvia…');
    fireEvent.change(field, { target: { value: 'vault' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(await screen.findByText('at /vault')).toBeDefined();

    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    await screen.findByRole('dialog', { name: 'Search Typvia' });
    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('keeps the template builder reachable even though it is not a room', async () => {
    renderShell('/vault');
    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Search Typvia' });
    fireEvent.click(within(palette).getByRole('button', { name: /Template builder/ }));
    expect(await screen.findByText('at /templates')).toBeDefined();
  });

  it('keeps sync and the shortcut table reachable without a menu', async () => {
    renderShell('/');
    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    let palette = await screen.findByRole('dialog', { name: 'Search Typvia' });
    fireEvent.click(within(palette).getByRole('button', { name: /Sync & devices/ }));
    expect(await screen.findByText('at /sync')).toBeDefined();
    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    palette = await screen.findByRole('dialog', { name: 'Search Typvia' });
    fireEvent.click(within(palette).getByRole('button', { name: /All shortcuts/ }));
    expect(await screen.findByText('at /shortcuts')).toBeDefined();
  });
});
