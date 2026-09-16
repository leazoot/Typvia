// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FirstSnippetStep } from './first-snippet-step';
import { OnboardingPage } from './onboarding-page';
import { TryStep } from './try-step';

const onboardingComplete = vi.fn();
const clipboardReadText = vi.fn();
const createSnippet = vi.fn();
const accessibilityStatus = vi.fn();
const openAccessibilitySettings = vi.fn();

vi.mock('@typvia/shared', () => ({
  onboardingComplete: () => onboardingComplete() as Promise<void>,
  clipboardReadText: () => clipboardReadText() as Promise<string | null>,
  createSnippet: (input: unknown) => createSnippet(input) as Promise<unknown>,
  accessibilityStatus: () => accessibilityStatus() as Promise<boolean>,
  openAccessibilitySettings: () => openAccessibilitySettings() as Promise<void>,
}));

let summonListener: (() => void) | null = null;
vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, cb: () => void) => {
    if (event === 'panel:summoned') summonListener = cb;
    return Promise.resolve(() => {
      summonListener = null;
    });
  },
}));

function renderPage(onDone = vi.fn()) {
  render(
    <MemoryRouter>
      <OnboardingPage onDone={onDone} />
    </MemoryRouter>,
  );
  return onDone;
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  summonListener = null;
});

describe('OnboardingPage', () => {
  it('opens on the first of four steps with a visible way out', () => {
    renderPage();
    expect(screen.getByRole('group', { name: 'Step 1 of 4 · Installed' })).toBeDefined();
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Installed.');
    expect(screen.getByRole('button', { name: 'Skip setup' })).toBeDefined();
  });

  it('speaks one language only under a zh locale', () => {
    render(
      <I18nProvider locale="zh">
        <MemoryRouter>
          <OnboardingPage onDone={vi.fn()} />
        </MemoryRouter>
      </I18nProvider>,
    );
    expect(screen.getByRole('group', { name: '第 1 步,共 4 步 · 装好了' })).toBeDefined();
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('装好了。');
    expect(screen.queryByText('Installed.')).toBeNull();
    expect(screen.getByRole('button', { name: '跳过设置' })).toBeDefined();
  });

  it('skip setup keeps the marker and hands over to the shell', async () => {
    onboardingComplete.mockResolvedValue(undefined);
    const onDone = renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(onboardingComplete).toHaveBeenCalled();
  });

  it('keeps the reader in charge when the marker cannot be saved', async () => {
    onboardingComplete.mockRejectedValue(new Error('io'));
    const onDone = renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    expect(await screen.findByRole('alert')).toBeDefined();
    expect(onDone).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Continue anyway' }));
    expect(onDone).toHaveBeenCalled();
  });

  it('offers the Accessibility page, lets the step be skipped, and goes back from the line', async () => {
    accessibilityStatus.mockResolvedValue(false);
    openAccessibilitySettings.mockResolvedValue(undefined);
    clipboardReadText.mockResolvedValue(null);
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: /^Start/ }));
    expect(await screen.findByRole('heading', { name: 'Let it type for you.' })).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: /Open that page/ }));
    await waitFor(() => expect(openAccessibilitySettings).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(/this page notices by itself/)).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: /Skip · just copy/ }));
    expect(await screen.findByRole('heading', { name: 'Give it its first words.' })).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Back to Installed' }));
    expect(screen.getByRole('heading', { name: 'Installed.' })).toBeDefined();
  });

  it('notices the grant by itself when the window comes back', async () => {
    accessibilityStatus.mockResolvedValue(false);
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: /^Start/ }));
    expect(await screen.findByRole('heading', { name: 'Let it type for you.' })).toBeDefined();
    accessibilityStatus.mockResolvedValue(true);
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
    });
    expect(await screen.findByRole('heading', { name: 'It can type for you now.' })).toBeDefined();
  });
});

describe('FirstSnippetStep', () => {
  it('prefills from the clipboard and saves it', async () => {
    clipboardReadText.mockResolvedValue('docker logs -f api\nsecond line');
    createSnippet.mockResolvedValue({ id: 's-1' });
    const onContinue = vi.fn();
    render(<FirstSnippetStep onContinue={onContinue} onImport={vi.fn()} />);

    expect(await screen.findByText(/We found this on your clipboard/)).toBeDefined();
    expect(screen.getByLabelText<HTMLInputElement>('Snippet title').value).toBe(
      'docker logs -f api',
    );
    fireEvent.click(screen.getByRole('button', { name: /Save it/ }));
    await waitFor(() =>
      expect(createSnippet).toHaveBeenCalledWith({
        title: 'docker logs -f api',
        body: 'docker logs -f api\nsecond line',
        snippetType: 'text',
      }),
    );
    expect(onContinue).toHaveBeenCalled();
  });

  it('starts empty when the clipboard offers nothing usable', async () => {
    clipboardReadText.mockResolvedValue(null);
    render(<FirstSnippetStep onContinue={vi.fn()} onImport={vi.fn()} />);
    expect(await screen.findByText(/Nothing usable on the clipboard/)).toBeDefined();
    const save = screen.getByRole('button', { name: /Save it/ });
    expect(save.hasAttribute('disabled')).toBe(true);
    fireEvent.change(screen.getByLabelText('Snippet content'), {
      target: { value: 'kubectl get pods' },
    });
    expect(save.hasAttribute('disabled')).toBe(false);
  });

  it('reports a failed save without losing the words', async () => {
    clipboardReadText.mockResolvedValue('some text');
    createSnippet.mockRejectedValue(new Error('db'));
    const onContinue = vi.fn();
    render(<FirstSnippetStep onContinue={onContinue} onImport={vi.fn()} />);
    await screen.findByText(/We found this on your clipboard/);
    fireEvent.click(screen.getByRole('button', { name: /Save it/ }));
    expect(await screen.findByText(/Nothing was saved/)).toBeDefined();
    expect(screen.getByLabelText<HTMLTextAreaElement>('Snippet content').value).toBe('some text');
    expect(onContinue).not.toHaveBeenCalled();
  });

  it('sends the reader to importing from another tool', async () => {
    clipboardReadText.mockResolvedValue(null);
    const onImport = vi.fn();
    render(<FirstSnippetStep onContinue={vi.fn()} onImport={onImport} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Bring them in from another tool' }));
    expect(onImport).toHaveBeenCalled();
  });
});

describe('TryStep', () => {
  it('says it worked only after the panel really opened', async () => {
    const onFinish = vi.fn();
    render(<TryStep onFinish={onFinish} />);
    expect(screen.getByRole('status').textContent).toBe('Waiting for the press…');
    expect(summonListener).not.toBeNull();
    await act(async () => {
      summonListener?.();
    });
    expect(screen.getByRole('status').textContent).toBe(
      'It came up just now. That is the whole gesture.',
    );
    fireEvent.click(screen.getByRole('button', { name: /Start using Typvia/ }));
    expect(onFinish).toHaveBeenCalled();
  });

  it('explains the Accessibility fallback when nothing happened', () => {
    render(<TryStep onFinish={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Nothing happened?' }));
    expect(screen.getByText(/needs Accessibility/)).toBeDefined();
    expect(screen.getByText(/inserts land on your clipboard/)).toBeDefined();
  });
});
