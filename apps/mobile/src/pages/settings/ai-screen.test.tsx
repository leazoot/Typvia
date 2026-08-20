// @vitest-environment jsdom
import type { AiProvider } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MobileAiScreen } from './ai-screen';

const ipc = vi.hoisted(() => ({
  aiProviderList: vi.fn(),
  aiProviderSave: vi.fn(),
  aiProviderDelete: vi.fn(),
  aiApiKeySet: vi.fn(),
  aiApiKeyClear: vi.fn(),
}));

vi.mock('@typvia/shared', () => ipc);

function provider(overrides: Partial<AiProvider> = {}): AiProvider {
  return {
    id: 'p1',
    name: 'Local Ollama',
    kind: 'ollama',
    baseUrl: 'http://127.0.0.1:11434/v1',
    model: 'llama3',
    timeoutMs: 30_000,
    hasApiKey: false,
    ...overrides,
  };
}

beforeEach(() => {
  ipc.aiProviderList.mockResolvedValue([]);
  ipc.aiProviderSave.mockResolvedValue(provider());
  ipc.aiProviderDelete.mockResolvedValue(undefined);
  ipc.aiApiKeySet.mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('MobileAiScreen', () => {
  it('adds a provider with the exact payload and no key call when blank', async () => {
    render(<MobileAiScreen onBack={() => {}} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Add provider' }));
    fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'llama3' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add provider' }));

    await waitFor(() => {
      expect(ipc.aiProviderSave).toHaveBeenCalledWith({
        id: null,
        name: 'Ollama',
        kind: 'ollama',
        baseUrl: '',
        model: 'llama3',
        timeoutMs: 0,
      });
    });
    expect(ipc.aiApiKeySet).not.toHaveBeenCalled();
  });

  it('stores an entered key against the saved provider id', async () => {
    render(<MobileAiScreen onBack={() => {}} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Add provider' }));
    fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'llama3' } });
    fireEvent.change(screen.getByLabelText('API key'), { target: { value: 'AKIA_FAKE_KEY' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add provider' }));

    await waitFor(() => {
      expect(ipc.aiApiKeySet).toHaveBeenCalledWith('p1', 'AKIA_FAKE_KEY');
    });
  });

  it('deletes only after the named second confirmation', async () => {
    ipc.aiProviderList.mockResolvedValue([provider()]);
    render(<MobileAiScreen onBack={() => {}} />);

    fireEvent.click(await screen.findByRole('button', { name: /Local Ollama/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete…' }));
    expect(ipc.aiProviderDelete).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Delete “Local Ollama”' }));
    await waitFor(() => {
      expect(ipc.aiProviderDelete).toHaveBeenCalledWith('p1');
    });
  });

  it('never renders a stored key back — only the fact that one exists', async () => {
    ipc.aiProviderList.mockResolvedValue([provider({ hasApiKey: true })]);
    render(<MobileAiScreen onBack={() => {}} />);

    fireEvent.click(await screen.findByRole('button', { name: /Local Ollama/ }));
    const key = screen.getByLabelText('API key') as HTMLInputElement;
    expect(key.value).toBe('');
    expect(key.placeholder).toContain('A key is stored');
    expect(screen.getByRole('button', { name: 'Remove the stored key' })).toBeDefined();
  });
});
