// @vitest-environment jsdom
//
// Android composition of the shared onboarding flow. The platform
// module is mocked wholesale so the same page renders its Android reading:
// five steps, no Full Access ledger, IME copy in the keyboard step. The iOS
// composition keeps its own file (onboarding-page.test.tsx) untouched.
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

vi.mock('../../platform', () => ({
  isAndroid: true,
  deviceWord: 'phone',
  pushBackHandler: () => () => {},
}));

function renderPage(onDone = vi.fn()) {
  ipc.listFolderChildren.mockResolvedValue([]);
  ipc.mobileBootstrap.mockResolvedValue({ schemaVersion: 9, snippetTotal: 10 });
  render(<OnboardingPage onDone={onDone} />);
  return { onDone };
}

/** Clicks through welcome, storage and the editor skip onto the IME step. */
function walkToKeyboard() {
  fireEvent.click(screen.getByRole('button', { name: 'Set up in five steps' }));
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile OnboardingPage on Android', () => {
  it('counts five honest steps and speaks in the Android device word', () => {
    renderPage();
    expect(screen.getByText('Step 1 of 5')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Set up in five steps' })).toBeDefined();
    expect(screen.getByText(/stays on this phone/)).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Set up in five steps' }));
    expect(screen.getByText('This phone only')).toBeDefined();
  });

  it('teaches enabling the IME and the switch gesture in one honest step', () => {
    renderPage();
    walkToKeyboard();

    expect(screen.getByText('Step 4 of 5')).toBeDefined();
    expect(screen.getByText('Turn on the Typvia bar.')).toBeDefined();
    expect(
      screen.getByText('Settings → System → Languages & input → On-screen keyboard'),
    ).toBeDefined();
    expect(screen.getByText('Manage keyboards → switch on Typvia')).toBeDefined();
    // Android's own generic warning is named here — where it will appear —
    // with the verified zero-permission fact, not reassurance.
    expect(screen.getByText(/standard warning about input methods/)).toBeDefined();
    expect(screen.getByText(/no Android permissions, not even internet/)).toBeDefined();
    // The switch gesture is rehearsed against a real text line.
    expect(screen.getByText('Then switch to it')).toBeDefined();
    expect(screen.getByLabelText('Type here to test')).toBeDefined();
    expect(screen.getByText(/doesn't detect it by\s+itself/)).toBeDefined();
  });

  it('the settings jump fires the real intent without claiming success', () => {
    ipc.openKeyboardSettings.mockResolvedValue(undefined);
    renderPage();
    walkToKeyboard();

    fireEvent.click(screen.getByRole('button', { name: /Open input settings/ }));
    expect(ipc.openKeyboardSettings).toHaveBeenCalledTimes(1);
    // Opening settings never advances by itself — enablement is not queried.
    expect(screen.getByText('Turn on the Typvia bar.')).toBeDefined();
  });

  it('a failed settings jump says what is still fine first', async () => {
    ipc.openKeyboardSettings.mockRejectedValue(new Error('no handler'));
    renderPage();
    walkToKeyboard();
    fireEvent.click(screen.getByRole('button', { name: /Open input settings/ }));
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('nothing is lost');
    expect(screen.getByText('Turn on the Typvia bar.')).toBeDefined();
  });

  it('skips the Full Access ledger: the keyboard step leads straight to done', async () => {
    ipc.onboardingComplete.mockResolvedValue(undefined);
    const { onDone } = renderPage();
    walkToKeyboard();
    fireEvent.click(screen.getByRole('button', { name: /I'll do it later — continue/ }));

    // No iOS-only ledger anywhere on Android.
    expect(screen.queryByText('About Full Access.')).toBeNull();
    expect(screen.getByText('Step 5 of 5')).toBeDefined();
    expect(await screen.findByText('10 snippets in your library')).toBeDefined();
    // Android status lines: bar wording, switcher gesture, fingerprint.
    expect(screen.getByText('Typvia bar')).toBeDefined();
    expect(screen.getByText(/keyboard icon in the navigation bar/)).toBeDefined();
    expect(screen.getByText(/doesn't check the\s+system list/)).toBeDefined();
    expect(screen.getByText(/your fingerprint on each use/)).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Start using Typvia' }));
    await waitFor(() => {
      expect(onDone).toHaveBeenCalled();
    });
    expect(ipc.onboardingComplete).toHaveBeenCalled();
  });
});
