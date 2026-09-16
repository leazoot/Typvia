// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AboutApp } from './about-app';

const libraryCounts = vi.fn();

vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('0.1.0') }));
vi.mock('@typvia/shared', () => ({
  libraryCounts: () => libraryCounts() as Promise<unknown>,
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('AboutApp', () => {
  it('shows the version and the real snippet count', async () => {
    libraryCounts.mockResolvedValue({ total: 3 });
    render(<AboutApp />);
    expect(await screen.findByText('0.1.0')).toBeDefined();
    expect(await screen.findByText('3 snippets, all on this Mac')).toBeDefined();
  });

  it('reads the count again when the window comes back', async () => {
    libraryCounts.mockResolvedValue({ total: 3 });
    render(<AboutApp />);
    await screen.findByText('3 snippets, all on this Mac');
    libraryCounts.mockResolvedValue({ total: 1 });
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
    });
    expect(await screen.findByText('1 snippet, all on this Mac')).toBeDefined();
  });

  it('opens the licences in place and folds them away again', async () => {
    libraryCounts.mockResolvedValue({ total: 0 });
    render(<AboutApp />);
    const licences = screen.getByRole('button', { name: 'Licences & thanks' });
    fireEvent.click(licences);
    expect(licences.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByLabelText('Licences & thanks').textContent).not.toBe('');
    fireEvent.click(licences);
    expect(screen.queryByLabelText('Licences & thanks', { selector: 'pre' })).toBeNull();
  });
});
