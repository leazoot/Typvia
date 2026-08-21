// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OnboardingPage } from './onboarding-page';

const ipc = vi.hoisted(() => ({
  onboardingComplete: vi.fn(),
  openKeyboardSettings: vi.fn(),
  mobileBootstrap: vi.fn(),
  createSnippet: vi.fn(),
  updateSnippet: vi.fn(),
  listFolderChildren: vi.fn(),
  historyList: vi.fn(),
  aiProviderList: vi.fn(),
  aiActionList: vi.fn(),
  aiActionRun: vi.fn(),
  aiOrganize: vi.fn(),
  batchTagSnippets: vi.fn(),
}));

vi.mock('@typvia/shared', () => ipc);

const SAVED_SNIPPET = {
  id: 's-1',
  title: 'Shipping address',
  body: 'Rm 1204, Bldg 3',
  snippetType: 'text',
  securityLevel: 'normal',
  description: null,
  folderId: null,
  trigger: null,
  triggerMode: null,
  language: null,
  isFavorite: false,
  isPinned: false,
  isEnabled: true,
  createdAt: 1,
  updatedAt: 1,
  lastUsedAt: null,
  usageCount: 0,
  version: 1,
  deletedAt: null,
};

function renderPage(onDone = vi.fn(), onSnippetSaved = vi.fn()) {
  ipc.listFolderChildren.mockResolvedValue([]);
  ipc.mobileBootstrap.mockResolvedValue({ schemaVersion: 9, snippetTotal: 1 });
  render(<OnboardingPage onDone={onDone} onSnippetSaved={onSnippetSaved} />);
  return { onDone, onSnippetSaved };
}

/** Clicks through welcome and storage onto the first-snippet editor. */
function walkToEditor() {
  fireEvent.click(screen.getByRole('button', { name: 'Set up in six steps' }));
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
}

/** Clicks through to the keyboard step by skipping the editor. */
function walkToKeyboard() {
  walkToEditor();
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile OnboardingPage', () => {
  it('opens on the welcome statement with a visible way out', () => {
    renderPage();
    expect(screen.getByText('Step 1 of 6')).toBeDefined();
    expect(screen.getByRole('heading', { level: 1 }).textContent).toContain('Save once.');
    expect(screen.getByRole('button', { name: 'Skip setup' })).toBeDefined();
  });

  it('skip setup persists the marker and hands over to the shell', async () => {
    ipc.onboardingComplete.mockResolvedValue(undefined);
    const { onDone } = renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    await waitFor(() => {
      expect(onDone).toHaveBeenCalled();
    });
    expect(ipc.onboardingComplete).toHaveBeenCalled();
  });

  it('keeps the user in charge when the marker cannot be saved', async () => {
    ipc.onboardingComplete.mockRejectedValue(new Error('io'));
    const { onDone } = renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    expect(await screen.findByRole('alert')).toBeDefined();
    expect(onDone).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Continue anyway' }));
    expect(onDone).toHaveBeenCalled();
  });

  it('walks storage choice with the sync option honestly unavailable', () => {
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Set up in six steps' }));
    expect(screen.getByText('This iPhone only')).toBeDefined();
    expect(screen.getByText('Not available on mobile yet')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    // The first-snippet step is the real create page, not a second editor.
    expect(screen.getByRole('dialog', { name: 'New snippet' })).toBeDefined();
  });

  it('saves the first snippet through the real editor page and moves on', async () => {
    ipc.createSnippet.mockResolvedValue(SAVED_SNIPPET);
    const { onSnippetSaved } = renderPage();
    walkToEditor();

    fireEvent.change(screen.getByLabelText('Snippet content'), {
      target: { value: 'Rm 1204, Bldg 3' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Add the keyboard.')).toBeDefined();
    expect(ipc.createSnippet).toHaveBeenCalledWith(
      expect.objectContaining({ body: 'Rm 1204, Bldg 3' }),
    );
    expect(onSnippetSaved).toHaveBeenCalled();
  });

  it('cancel on the first-snippet step skips ahead without saving', () => {
    const { onSnippetSaved } = renderPage();
    walkToEditor();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.getByText('Add the keyboard.')).toBeDefined();
    expect(ipc.createSnippet).not.toHaveBeenCalled();
    expect(onSnippetSaved).not.toHaveBeenCalled();
  });

  it('the keyboard step opens Settings without claiming success', () => {
    ipc.openKeyboardSettings.mockResolvedValue(undefined);
    renderPage();
    walkToKeyboard();

    expect(screen.getByText('Settings → General → Keyboard')).toBeDefined();
    expect(screen.getByText('Keyboards → Add New Keyboard')).toBeDefined();
    expect(screen.getByText(/iOS gives the app no way to check for itself/)).toBeDefined();
    // The honest degradation comes before any ask: the app still works.
    expect(screen.getByText(/still saves and copies snippets/)).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: /Open Settings/ }));
    expect(ipc.openKeyboardSettings).toHaveBeenCalledTimes(1);
    // Opening Settings never advances by itself — iOS reports nothing back.
    expect(screen.getByText('Add the keyboard.')).toBeDefined();
  });

  it('the keyboard step continues without the jump ever firing', () => {
    renderPage();
    walkToKeyboard();
    fireEvent.click(screen.getByRole('button', { name: /I'll do it later — continue/ }));
    expect(screen.getByText('About Full Access.')).toBeDefined();
    expect(ipc.openKeyboardSettings).not.toHaveBeenCalled();
  });

  it('a failed Settings jump says what is still fine first', async () => {
    ipc.openKeyboardSettings.mockRejectedValue(new Error('no handler'));
    renderPage();
    walkToKeyboard();
    fireEvent.click(screen.getByRole('button', { name: /Open Settings/ }));
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('nothing is lost');
    expect(screen.getByText('Add the keyboard.')).toBeDefined();
  });

  it('the ledger states that nothing needs Full Access today', () => {
    renderPage();
    walkToKeyboard();
    fireEvent.click(screen.getByRole('button', { name: /I'll do it later — continue/ }));

    expect(screen.getByText('Works now — without Full Access')).toBeDefined();
    expect(screen.getByText('The keyboard shows your snippets')).toBeDefined();
    expect(screen.getByText('Works offline')).toBeDefined();
    expect(screen.getByText('Nothing, today')).toBeDefined();
    expect(screen.getByText(/Typvia currently requests no Full Access/)).toBeDefined();
    expect(screen.getByText(/no analytics and no network calls/)).toBeDefined();
  });

  it('the done step shows the real count and an honest keyboard line', async () => {
    ipc.onboardingComplete.mockResolvedValue(undefined);
    const { onDone } = renderPage();
    // The count is read when the done step mounts, so this override wins.
    ipc.mobileBootstrap.mockResolvedValue({ schemaVersion: 9, snippetTotal: 3 });
    walkToKeyboard();
    fireEvent.click(screen.getByRole('button', { name: /I'll do it later — continue/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

    expect(await screen.findByText('3 snippets in your library')).toBeDefined();
    // iOS has no enablement query API: the line claims readiness only
    // conditionally and says so.
    expect(screen.getByText(/If you added the keyboard in Settings/)).toBeDefined();
    expect(screen.getByText(/no way to\s+check from here/)).toBeDefined();
    expect(screen.getByText('Later')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Start using Typvia' }));
    await waitFor(() => {
      expect(onDone).toHaveBeenCalled();
    });
    expect(ipc.onboardingComplete).toHaveBeenCalled();
  });

  it('states honest fallback copy when the done-step count cannot be read', async () => {
    renderPage();
    ipc.mobileBootstrap.mockRejectedValue(new Error('db'));
    walkToKeyboard();
    fireEvent.click(screen.getByRole('button', { name: /I'll do it later — continue/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

    expect(await screen.findByText('Your library is ready')).toBeDefined();
    expect(screen.getByText(/your snippets are unaffected/)).toBeDefined();
  });
});
