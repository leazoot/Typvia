// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AiProvider, OrganizeSuggestion } from '@typvia/shared';
import { Organize } from './organize';
import type { Draft } from './use-editor-draft';

const aiProviderList = vi.fn();
const aiOrganize = vi.fn();
const batchTagSnippets = vi.fn();
const createSnippet = vi.fn();
const updateSnippet = vi.fn();

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  aiProviderList: () => aiProviderList() as Promise<AiProvider[]>,
  aiOrganize: (...args: unknown[]) => aiOrganize(...args) as Promise<OrganizeSuggestion>,
  batchTagSnippets: (...args: unknown[]) => batchTagSnippets(...args) as Promise<void>,
  createSnippet: (...args: unknown[]) => createSnippet(...args) as Promise<unknown>,
  updateSnippet: (...args: unknown[]) => updateSnippet(...args) as Promise<unknown>,
}));

const PROVIDER: AiProvider = {
  id: 'p1',
  name: 'Local Ollama',
  kind: 'ollama',
  baseUrl: 'http://127.0.0.1:11434/v1',
  model: 'llama3',
  timeoutMs: 30000,
  hasApiKey: false,
};

const SUGGESTION: OrganizeSuggestion = {
  title: 'Compose up',
  description: 'Starts the stack.',
  snippetType: 'command',
  securityLevel: 'normal',
  trigger: ';dcu',
  tags: [{ id: 't1', name: 'docker', createdAt: 1 }],
  folder: null,
};

function draft(overrides: Partial<Draft> = {}): Draft {
  return {
    id: 's1',
    title: 'untitled',
    body: 'docker compose up -d',
    snippetType: 'text',
    description: null,
    folderId: null,
    trigger: null,
    triggerMode: null,
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    ...overrides,
  };
}

function renderStrip(d: Draft, onApply = vi.fn()) {
  render(
    <MemoryRouter>
      <Organize draft={d} onApply={onApply} />
    </MemoryRouter>,
  );
  return onApply;
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

describe('Organize strip', () => {
  it('is disabled while the draft body is blank', () => {
    renderStrip(draft({ body: '   ' }));
    const run = screen.getByRole('button', { name: /Organize/ });
    expect((run as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows the quiet loading state while the suggestion is in flight', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiOrganize.mockReturnValue(new Promise(() => undefined));
    renderStrip(draft());
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Organize/ }));
    });
    expect(screen.getByRole('status').textContent).toContain('Organizing');
  });

  it('offers Settings when no provider is configured, saying what still works', async () => {
    aiProviderList.mockResolvedValue([]);
    renderStrip(draft());
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Organize/ }));
    });
    expect(screen.getByRole('status').textContent).toContain('works without AI');
    expect(screen.getByRole('button', { name: 'Set up in Settings' })).toBeDefined();
    expect(aiOrganize).not.toHaveBeenCalled();
  });

  it('reports failure with the draft-is-safe framing and a retry', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiOrganize.mockRejectedValue(new Error('the provider could not be reached'));
    renderStrip(draft());
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Organize/ }));
    });
    expect(screen.getByRole('status').textContent).toContain('Your draft is safe');
    expect(screen.getByRole('status').textContent).toContain('could not be reached');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeDefined();
  });

  it('never stores a suggestion by itself — applying is the confirmation', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiOrganize.mockResolvedValue(SUGGESTION);
    const onApply = renderStrip(draft());
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Organize/ }));
    });
    // Suggestions arrived; nothing was applied and nothing was saved.
    expect(screen.getByText('Compose up')).toBeDefined();
    expect(onApply).not.toHaveBeenCalled();
    expect(createSnippet).not.toHaveBeenCalled();
    expect(updateSnippet).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: /Title/ }));
    expect(onApply).toHaveBeenCalledWith({ title: 'Compose up' });

    fireEvent.click(screen.getByRole('button', { name: 'Apply all' }));
    expect(onApply).toHaveBeenLastCalledWith({
      title: 'Compose up',
      description: 'Starts the stack.',
      snippetType: 'command',
      trigger: ';dcu',
      triggerMode: 'delimiter',
    });
  });

  it('says so plainly when the model offered nothing usable', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiOrganize.mockResolvedValue({
      title: null,
      description: null,
      snippetType: null,
      securityLevel: null,
      trigger: null,
      tags: [],
      folder: null,
    });
    renderStrip(draft());
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Organize/ }));
    });
    expect(screen.getByRole('status').textContent).toContain('No suggestions');
  });

  it('applies suggested tags through the existing tag command', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiOrganize.mockResolvedValue(SUGGESTION);
    batchTagSnippets.mockResolvedValue(undefined);
    renderStrip(draft());
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Organize/ }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Tags/ }));
    });
    expect(batchTagSnippets).toHaveBeenCalledWith(['s1'], 't1');
    expect(screen.getByText('Applied')).toBeDefined();
  });

  it('remembers the chosen provider as a UI preference', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiOrganize.mockResolvedValue(SUGGESTION);
    renderStrip(draft());
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Organize/ }));
    });
    expect(localStorage.getItem('tv.ai.provider')).toBe('p1');
    expect(aiOrganize).toHaveBeenCalledWith('p1', {
      title: 'untitled',
      body: 'docker compose up -d',
      description: null,
      isSensitive: false,
    });
  });
});
