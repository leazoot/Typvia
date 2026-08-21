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

function renderShell(path = '/') {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <WorkspaceShell>
        <Routes>
          <Route path="*" element={<Probe />} />
        </Routes>
      </WorkspaceShell>
    </MemoryRouter>,
  );
}

afterEach(cleanup);

describe('workspace navigation', () => {
  it('shows three destinations and names the room the user is in', () => {
    renderShell('/vault');
    expect(screen.getByRole('button', { name: 'Home' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Settings' })).toBeDefined();
    // The switcher wears the current room's name, not a generic label.
    const switcher = screen.getByRole('button', { name: 'Vault', expanded: false });
    expect(switcher.getAttribute('aria-haspopup')).toBe('menu');
  });

  it('falls back to the workspace label outside the four rooms', () => {
    renderShell('/');
    expect(screen.getByRole('button', { name: 'Workspace' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Home' }).getAttribute('aria-current')).toBe('page');
  });

  it('switches rooms from the popover and marks the current one', async () => {
    renderShell('/library');
    fireEvent.click(screen.getByRole('button', { name: 'Library', expanded: false }));
    const menu = screen.getByRole('menu', { name: 'Workspace' });
    const items = within(menu).getAllByRole('menuitem');
    expect(items).toHaveLength(4);
    expect(
      ['TX', 'SC', 'AI', '↺'].every((mark, index) => items[index]?.textContent?.startsWith(mark)),
    ).toBe(true);
    // The room the user is in shows a dot instead of its shortcut.
    expect(within(items[0]!).getByRole('img', { name: 'Current' })).toBeDefined();
    expect(within(items[1]!).getByText('⌘2')).toBeDefined();

    fireEvent.click(within(menu).getByRole('menuitem', { name: /AI Actions/ }));
    expect(await screen.findByText('at /ai')).toBeDefined();
    expect(screen.queryByRole('menu')).toBeNull();
  });
});

describe('workspace keyboard map', () => {
  it('jumps to a room with its ⌘-digit and to settings with ⌘,', async () => {
    renderShell('/');
    fireEvent.keyDown(document, { key: '2', metaKey: true });
    expect(await screen.findByText('at /vault')).toBeDefined();
    fireEvent.keyDown(document, { key: ',', metaKey: true });
    expect(await screen.findByText('at /settings')).toBeDefined();
  });

  it('opens the editor for a new snippet with ⌘N', async () => {
    renderShell('/');
    fireEvent.keyDown(document, { key: 'n', metaKey: true });
    expect(await screen.findByText('at /editor')).toBeDefined();
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
    renderShell('/');
    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Search Typvia' });
    fireEvent.click(within(palette).getByRole('button', { name: /Template builder/ }));
    expect(await screen.findByText('at /templates')).toBeDefined();
  });
});
