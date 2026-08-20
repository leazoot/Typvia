// @vitest-environment jsdom
import type { AiAction, AiProvider } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AiStrip } from './ai-strip';

const ipc = vi.hoisted(() => ({
  aiProviderList: vi.fn(),
  aiActionList: vi.fn(),
  aiActionRun: vi.fn(),
  aiOrganize: vi.fn(),
  batchTagSnippets: vi.fn(),
  createSnippet: vi.fn(),
}));

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  ...ipc,
}));

function provider(): AiProvider {
  return {
    id: 'p1',
    name: 'Local Ollama',
    kind: 'ollama',
    baseUrl: 'http://127.0.0.1:11434/v1',
    model: 'llama3',
    timeoutMs: 30_000,
    hasApiKey: false,
  };
}

function action(): AiAction {
  return {
    id: 'builtin.rewrite',
    name: 'Rewrite',
    promptTemplate: 'Rewrite the text.',
    providerId: 'p1',
    model: null,
    inputSource: 'snippet',
    outputMode: 'replace',
    permissionScope: 'mask_secrets',
    temperature: null,
    isBuiltin: true,
  };
}

function strip(overrides: Partial<Parameters<typeof AiStrip>[0]> = {}) {
  return (
    <AiStrip
      snippetId={null}
      title="Greeting"
      body="hello there"
      snippetType="text"
      folderId={null}
      trigger=""
      description={null}
      onApply={() => {}}
      onBodyChange={() => {}}
      {...overrides}
    />
  );
}

beforeEach(() => {
  ipc.aiProviderList.mockResolvedValue([provider()]);
  ipc.aiActionList.mockResolvedValue([action()]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('AiStrip', () => {
  it('is hidden entirely with AI off — no greyed controls', async () => {
    ipc.aiProviderList.mockResolvedValue([]);
    const { container } = render(strip());

    await waitFor(() => {
      expect(ipc.aiProviderList).toHaveBeenCalled();
    });
    expect(container.querySelector('.tv-med-ai')).toBeNull();
    expect(screen.queryByText('Organize with AI')).toBeNull();
  });

  it('organizes the draft and applies a field only on tap', async () => {
    const onApply = vi.fn();
    ipc.aiOrganize.mockResolvedValue({
      title: 'Warm greeting',
      description: null,
      snippetType: null,
      securityLevel: null,
      trigger: null,
      tags: [],
      folder: null,
    });
    render(strip({ onApply }));

    fireEvent.click(await screen.findByRole('button', { name: 'Organize with AI' }));
    await waitFor(() => {
      expect(ipc.aiOrganize).toHaveBeenCalledWith('p1', {
        title: 'Greeting',
        body: 'hello there',
        description: null,
        isSensitive: false,
      });
    });

    expect(onApply).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole('button', { name: /Warm greeting/ }));
    expect(onApply).toHaveBeenCalledWith({ title: 'Warm greeting' });
  });

  it('runs the action on the content and applies nothing until confirm', async () => {
    const onBodyChange = vi.fn();
    ipc.aiActionRun.mockResolvedValue({ output: 'Hello there!', maskedKinds: [] });
    render(strip({ onBodyChange }));

    fireEvent.click(await screen.findByRole('button', { name: 'Run on content' }));
    await waitFor(() => {
      expect(ipc.aiActionRun).toHaveBeenCalledWith('builtin.rewrite', {
        text: 'hello there',
        source: 'snippet',
        isSensitive: false,
      });
    });

    expect(await screen.findByText('Hello there!')).toBeDefined();
    expect(onBodyChange).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Replace input' }));
    expect(onBodyChange).toHaveBeenCalledWith('Hello there!');
    expect(await screen.findByText('Content replaced.')).toBeDefined();
  });

  it('names the stripped secret kinds on the pending result', async () => {
    ipc.aiActionRun.mockResolvedValue({ output: 'Cleaned.', maskedKinds: ['aws_key'] });
    render(strip());

    fireEvent.click(await screen.findByRole('button', { name: 'Run on content' }));
    expect(await screen.findByText(/Secrets stripped before sending · aws_key/)).toBeDefined();
  });

  it('a failed run says the content is unchanged first', async () => {
    ipc.aiActionRun.mockRejectedValue(new Error('the model was not found'));
    render(strip());

    fireEvent.click(await screen.findByRole('button', { name: 'Run on content' }));
    expect(
      await screen.findByText(/Your content is unchanged — the model was not found/),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeDefined();
  });
});
