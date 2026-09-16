// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it } from 'vitest';
import { WorkspaceShell } from './shell';
import { useTitleBar } from './title-bar';

function Speaker() {
  const [note, setNote] = useState('Saved as you type');
  useTitleBar({ trailing: note }, note);
  return (
    <button type="button" onClick={() => setNote('Saved')}>
      save
    </button>
  );
}

afterEach(cleanup);

describe('title bar', () => {
  it('shows what the page on screen says, follows it, and goes back when it leaves', () => {
    const { rerender } = render(
      <MemoryRouter initialEntries={['/editor']}>
        <WorkspaceShell>
          <Speaker />
        </WorkspaceShell>
      </MemoryRouter>,
    );
    const bar = screen.getByRole('banner');
    expect(within(bar).getByText('Saved as you type')).toBeDefined();
    expect(within(bar).queryByText('⌘⇧V')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'save' }));
    expect(within(bar).getByText('Saved')).toBeDefined();

    rerender(
      <MemoryRouter initialEntries={['/editor']}>
        <WorkspaceShell>
          <p>no speaker</p>
        </WorkspaceShell>
      </MemoryRouter>,
    );
    expect(within(screen.getByRole('banner')).getByText('⌘⇧V')).toBeDefined();
  });
});
