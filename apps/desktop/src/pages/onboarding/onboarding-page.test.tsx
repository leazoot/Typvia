// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DoneStep } from './done-step';
import { FirstSnippetStep } from './first-snippet-step';
import { OnboardingPage } from './onboarding-page';
import { ShortcutStep } from './shortcut-step';

const onboardingComplete = vi.fn();
const clipboardReadText = vi.fn();
const createSnippet = vi.fn();
const espansoStatus = vi.fn();

vi.mock('@typvia/shared', () => ({
  onboardingComplete: () => onboardingComplete() as Promise<void>,
  clipboardReadText: () => clipboardReadText() as Promise<string | null>,
  createSnippet: (input: unknown) => createSnippet(input) as Promise<unknown>,
  espansoStatus: () => espansoStatus() as Promise<unknown>,
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
  it('opens on the welcome statement with a visible way out', () => {
    onboardingComplete.mockResolvedValue(undefined);
    renderPage();
    expect(screen.getByText('Step 1 of 5')).toBeDefined();
    expect(screen.getByRole('heading', { level: 1 }).textContent).toContain('Save once.');
    expect(screen.getByRole('button', { name: 'Skip setup' })).toBeDefined();
  });

  it('renders a single Chinese-language welcome under a zh locale', () => {
    onboardingComplete.mockResolvedValue(undefined);
    render(
      <I18nProvider locale="zh">
        <MemoryRouter>
          <OnboardingPage onDone={vi.fn()} />
        </MemoryRouter>
      </I18nProvider>,
    );
    expect(screen.getByText('第 1 步，共 5 步')).toBeDefined();
    expect(screen.getByRole('heading', { level: 1 }).textContent).toContain('保存一次，');
    // Exactly one language: the English tagline is gone in zh.
    expect(screen.queryByText(/Save once\./)).toBeNull();
    expect(screen.getByRole('button', { name: '跳过设置' })).toBeDefined();
  });

  it('skip setup persists the marker and hands over to the shell', async () => {
    onboardingComplete.mockResolvedValue(undefined);
    const onDone = renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(onboardingComplete).toHaveBeenCalled();
  });

  it('keeps the user in charge when the marker cannot be saved', async () => {
    onboardingComplete.mockRejectedValue(new Error('io'));
    const onDone = renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    expect(await screen.findByRole('alert')).toBeDefined();
    expect(onDone).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Continue anyway' }));
    expect(onDone).toHaveBeenCalled();
  });

  it('walks storage choice with the sync option honestly unavailable', async () => {
    clipboardReadText.mockResolvedValue(null);
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: /Set up in four steps/ }));
    expect(screen.getByText('This Mac only')).toBeDefined();
    expect(screen.getByText('Arrives with the mobile apps — not available yet')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: /Continue/ }));
    expect(await screen.findByText('Save your first one.')).toBeDefined();
  });
});

describe('FirstSnippetStep', () => {
  it('prefills from the clipboard seed and saves it', async () => {
    clipboardReadText.mockResolvedValue('docker logs -f api\nsecond line');
    createSnippet.mockResolvedValue({ id: 's-1' });
    const onContinue = vi.fn();
    render(<FirstSnippetStep onContinue={onContinue} />);

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
    render(<FirstSnippetStep onContinue={vi.fn()} />);
    expect(await screen.findByText(/Nothing usable on the clipboard/)).toBeDefined();
    const save = screen.getByRole('button', { name: /Save it/ });
    expect(save.hasAttribute('disabled')).toBe(true);
    fireEvent.change(screen.getByLabelText('Snippet content'), {
      target: { value: 'kubectl get pods' },
    });
    expect(save.hasAttribute('disabled')).toBe(false);
  });

  it('reports a failed save without losing the text', async () => {
    clipboardReadText.mockResolvedValue('some text');
    createSnippet.mockRejectedValue(new Error('db'));
    const onContinue = vi.fn();
    render(<FirstSnippetStep onContinue={onContinue} />);
    await screen.findByText(/We found this on your clipboard/);
    fireEvent.click(screen.getByRole('button', { name: /Save it/ }));
    expect(await screen.findByText(/Nothing was saved/)).toBeDefined();
    expect(screen.getByLabelText<HTMLTextAreaElement>('Snippet content').value).toBe('some text');
    expect(onContinue).not.toHaveBeenCalled();
  });
});

describe('ShortcutStep', () => {
  it('only unlocks continue after the panel genuinely opened', async () => {
    const onContinue = vi.fn();
    render(<ShortcutStep onContinue={onContinue} />);
    const cta = screen.getByRole('button', { name: /It opened — continue/ });
    expect(cta.hasAttribute('disabled')).toBe(true);
    expect(summonListener).not.toBeNull();
    summonListener?.();
    await waitFor(() => expect(cta.hasAttribute('disabled')).toBe(false));
    fireEvent.click(cta);
    expect(onContinue).toHaveBeenCalled();
  });

  it('explains the accessibility fallback when nothing happened', () => {
    const onContinue = vi.fn();
    render(<ShortcutStep onContinue={onContinue} />);
    fireEvent.click(screen.getByRole('button', { name: 'Nothing happened?' }));
    expect(screen.getByText(/needs Accessibility/)).toBeDefined();
    expect(screen.getByText(/inserts\s+land on your clipboard/)).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Continue anyway' }));
    expect(onContinue).toHaveBeenCalled();
  });
});

describe('DoneStep', () => {
  it('states the real engine status and finishes into the app', async () => {
    espansoStatus.mockResolvedValue({
      state: 'off',
      version: '2.4.0',
      configPath: null,
      enabled: false,
      triggerCount: 0,
      coexistenceChoice: null,
    });
    const onFinish = vi.fn();
    render(<DoneStep onFinish={onFinish} />);
    expect(await screen.findByText(/one switch in Settings turns it on/)).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Set up now' }));
    expect(onFinish).toHaveBeenCalledWith('/settings');
    fireEvent.click(screen.getByRole('button', { name: /Start using Typvia/ }));
    expect(onFinish).toHaveBeenCalledWith();
  });
});
