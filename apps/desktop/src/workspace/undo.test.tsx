// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { UndoProvider, useUndo, type NoteMood } from './undo';

function Speaker({ mood }: { mood?: NoteMood }) {
  const { say, offer } = useUndo();
  return (
    <>
      <button
        type="button"
        onClick={() =>
          say({ title: 'Said', body: 'What holds.', ...(mood === undefined ? {} : { mood }) })
        }
      >
        say
      </button>
      <button
        type="button"
        onClick={() =>
          offer({
            title: 'Offered',
            body: '⌘Z.',
            failure: 'Still safe.',
            undo: () => Promise.resolve(),
          })
        }
      >
        offer
      </button>
    </>
  );
}

function mascotOnNote(): string | null {
  return document.querySelector('.tpi-undo [data-state]')?.getAttribute('data-state') ?? null;
}

afterEach(cleanup);

describe('corner note mascot', () => {
  it('looks confused on a note about something that did not happen', () => {
    render(
      <UndoProvider>
        <Speaker />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'say' }));
    expect(mascotOnNote()).toBe('confused');
  });

  it('is happy when the note says it went through', () => {
    render(
      <UndoProvider>
        <Speaker mood="happy" />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'say' }));
    expect(mascotOnNote()).toBe('happy');
  });

  it('stays off a plain fact and off an undoable note', () => {
    render(
      <UndoProvider>
        <Speaker mood={null} />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'say' }));
    expect(mascotOnNote()).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'offer' }));
    expect(screen.getByText('Offered')).toBeDefined();
    expect(mascotOnNote()).toBeNull();
  });
});
