// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { usePaperMenu, type MenuEntry } from './menu';

function Harness({ entries }: { entries: readonly MenuEntry[] }) {
  const menu = usePaperMenu('Actions');
  return (
    <>
      <button type="button" onClick={() => menu.open({ x: 10, y: 10 }, entries)}>
        open
      </button>
      {menu.node}
    </>
  );
}

afterEach(cleanup);

function openWith(entries: readonly MenuEntry[]) {
  render(<Harness entries={entries} />);
  fireEvent.click(screen.getByRole('button', { name: 'open' }));
  return screen.getByRole('menu', { name: 'Actions' });
}

describe('paper menu', () => {
  it('walks usable rows with the arrow keys, skipping disabled rows and status lines', () => {
    const copy = vi.fn();
    const menu = openWith([
      { kind: 'item', label: 'Insert', reason: 'Not here' },
      { kind: 'separator' },
      { kind: 'item', label: 'Copy', keys: '⌥⏎', onSelect: copy },
      { kind: 'status', label: '4 · the oldest has 26 days left' },
    ]);
    expect(document.activeElement).toBe(menu);
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    const row = within(menu).getByRole('menuitem', { name: /Copy/ });
    expect(row.className).toContain('is-active');
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(row.className).toContain('is-active');
    fireEvent.keyDown(menu, { key: 'Enter' });
    expect(copy).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('opens a submenu with → and closes only the submenu with esc', () => {
    const move = vi.fn();
    const menu = openWith([
      {
        kind: 'item',
        label: 'Move to collection',
        submenu: [{ kind: 'item', label: 'Mail', onSelect: move }],
      },
    ]);
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    fireEvent.keyDown(menu, { key: 'ArrowRight' });
    const sub = screen.getByRole('menu', { name: 'Move to collection' });
    fireEvent.keyDown(sub, { key: 'Escape' });
    expect(screen.queryByRole('menu', { name: 'Move to collection' })).toBeNull();
    expect(screen.getByRole('menu', { name: 'Actions' })).toBeDefined();

    fireEvent.keyDown(menu, { key: 'ArrowRight' });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Mail' }));
    expect(move).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('jumps to a row by its first letter and marks the single choice with a dot', () => {
    const menu = openWith([
      { kind: 'item', label: 'Typing', chosen: true, onSelect: () => undefined },
      { kind: 'item', label: 'Paste', chosen: false, onSelect: () => undefined },
    ]);
    fireEvent.keyDown(menu, { key: 'p' });
    const paste = within(menu).getByRole('menuitemradio', { name: 'Paste' });
    expect(paste.className).toContain('is-active');
    expect(paste.getAttribute('aria-checked')).toBe('false');
    const typing = within(menu).getByRole('menuitemradio', { name: 'Typing' });
    expect(typing.querySelector('.tpi-menu-dot')?.getAttribute('data-on')).toBe('true');
  });

  it('marks a destructive row so it turns solid only when it is the one about to run', () => {
    const menu = openWith([
      { kind: 'item', label: 'Delete snippet', danger: true, onSelect: () => undefined },
    ]);
    const row = within(menu).getByRole('menuitem', { name: /Delete snippet/ });
    expect(row.className).toContain('is-danger');
    expect(row.className).not.toContain('is-active');
    fireEvent.mouseEnter(row);
    expect(row.className).toContain('is-active');
  });
});
