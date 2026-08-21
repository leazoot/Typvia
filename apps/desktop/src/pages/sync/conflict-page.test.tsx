// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { ConflictPair, Snippet } from '@typvia/shared';
import type * as SharedModule from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConflictPage } from './conflict-page';

const mocks = vi.hoisted(() => ({
  syncConflicts: vi.fn(),
  syncConflictResolve: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...mocks };
});

function snippet(id: string, title: string, body: string | null): Snippet {
  return {
    id,
    title,
    body,
    snippetType: 'text',
    securityLevel: body === null ? 'sensitive' : 'normal',
    description: null,
    folderId: null,
    trigger: null,
    triggerMode: null,
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 2,
    lastUsedAt: null,
    usageCount: 0,
    version: 1,
    deletedAt: null,
  };
}

function pair(sensitive = false): ConflictPair {
  return {
    source: snippet('source-1', 'Weekly update', sensitive ? null : 'Shipped\nRisks\nAsks'),
    copy: snippet(
      'copy-1',
      'Weekly update (conflict on Pixel 8)',
      sensitive ? null : 'Shipped\nRisks\nBlockers',
    ),
    sensitive,
  };
}

function renderPage() {
  return render(
    <MemoryRouter initialEntries={['/sync/conflicts']}>
      <ConflictPage />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.syncConflicts.mockResolvedValue([pair()]);
  mocks.syncConflictResolve.mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('Conflict resolution', () => {
  it('shows both versions whole and readable, side by side', async () => {
    renderPage();

    expect(await screen.findByText('Weekly update changed in two places.')).toBeTruthy();
    expect(screen.getByText(/Shipped\s+Risks\s+Asks/)).toBeTruthy();
    expect(screen.getByText(/Shipped\s+Risks\s+Blockers/)).toBeTruthy();
    expect(screen.getByLabelText('The version in use')).toBeTruthy();
    expect(screen.getByLabelText('The version set aside')).toBeTruthy();
    // Two "Keep this one" buttons: the choice is made by reading, not merging.
    expect(screen.getAllByRole('button', { name: 'Keep this one' })).toHaveLength(2);
  });

  it('keeps the version in use when its column is chosen', async () => {
    renderPage();
    const [keepSource] = await screen.findAllByRole('button', { name: 'Keep this one' });

    fireEvent.click(keepSource as HTMLElement);

    await waitFor(() => {
      expect(mocks.syncConflictResolve).toHaveBeenCalledWith('copy-1', 'source');
    });
  });

  it('keeps the version that arrived when its column is chosen', async () => {
    renderPage();
    const buttons = await screen.findAllByRole('button', { name: 'Keep this one' });

    fireEvent.click(buttons[1] as HTMLElement);

    await waitFor(() => {
      expect(mocks.syncConflictResolve).toHaveBeenCalledWith('copy-1', 'copy');
    });
  });

  it('can keep both as two snippets', async () => {
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Keep both as two snippets' }));

    await waitFor(() => {
      expect(mocks.syncConflictResolve).toHaveBeenCalledWith('copy-1', 'both');
    });
  });

  it('offers deciding later as a first-class option and promises nothing is deleted', async () => {
    renderPage();

    expect(
      await screen.findByRole('button', { name: /Decide later — both stay on their devices/ }),
    ).toBeTruthy();
    expect(screen.getByText(/Nothing is deleted/)).toBeTruthy();
  });

  it('never renders a sensitive body while comparing', async () => {
    mocks.syncConflicts.mockResolvedValue([pair(true)]);
    const { container } = renderPage();

    await screen.findByText('Weekly update changed in two places.');
    expect(container.querySelectorAll('.tv-conflict-locked')).toHaveLength(2);
    expect(screen.getAllByText('encrypted · not shown')).toHaveLength(2);
    expect(container.textContent).not.toContain('Blockers');
  });

  it('says what is still fine when resolving fails', async () => {
    const { IpcError } = await vi.importActual<typeof SharedModule>('@typvia/shared');
    mocks.syncConflictResolve.mockRejectedValue(
      new IpcError('permission_denied', 'unlock the vault first'),
    );
    renderPage();
    const [keepSource] = await screen.findAllByRole('button', { name: 'Keep this one' });

    fireEvent.click(keepSource as HTMLElement);

    expect(await screen.findByText(/Both versions are unchanged/)).toBeTruthy();
  });

  it('reports an empty decision list rather than an empty page', async () => {
    mocks.syncConflicts.mockResolvedValue([]);
    renderPage();

    expect(await screen.findByText('Nothing left to decide.')).toBeTruthy();
  });
});
