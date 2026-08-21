// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AiAction, AiActionRunResult, AiProvider } from '@typvia/shared';
import { ActionsPage } from './actions-page';

const aiActionList = vi.fn();
const aiActionSave = vi.fn();
const aiActionDelete = vi.fn();
const aiActionRun = vi.fn();
const aiProviderList = vi.fn();
const createSnippet = vi.fn();

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  aiActionList: () => aiActionList() as Promise<AiAction[]>,
  aiActionSave: (...args: unknown[]) => aiActionSave(...args) as Promise<AiAction>,
  aiActionDelete: (...args: unknown[]) => aiActionDelete(...args) as Promise<void>,
  aiActionRun: (...args: unknown[]) => aiActionRun(...args) as Promise<AiActionRunResult>,
  aiProviderList: () => aiProviderList() as Promise<AiProvider[]>,
  createSnippet: (...args: unknown[]) => createSnippet(...args) as Promise<unknown>,
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

function action(overrides: Partial<AiAction> = {}): AiAction {
  return {
    id: 'builtin.rewrite',
    name: 'Rewrite',
    promptTemplate: 'Rewrite the text in a professional register.',
    providerId: 'p1',
    model: null,
    inputSource: 'selection',
    outputMode: 'replace',
    permissionScope: 'normal_only',
    temperature: null,
    isBuiltin: true,
    ...overrides,
  };
}

async function renderPage() {
  render(
    <MemoryRouter>
      <ActionsPage />
    </MemoryRouter>,
  );
  await act(async () => {});
}

/** Opens the folded test bench, which is where a run lives now. */
async function openBench() {
  fireEvent.click(screen.getByRole('button', { name: /Test action/ }));
  return screen.getByLabelText('Test input');
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('ActionsPage — the studio', () => {
  it('offers setup instead of a greyed list when AI is off', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([]);
    await renderPage();
    expect(screen.queryByText('Rewrite')).toBeNull();
    expect(screen.getByText('Everything else works without AI.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Set up in Settings' })).toBeTruthy();
  });

  it('lists actions by name alone and states the flow in one sentence', async () => {
    aiActionList.mockResolvedValue([action(), action({ id: 'a2', name: 'Translate' })]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    await renderPage();
    // No "AI" prefix on every row — the whole page is AI actions.
    const list = screen.getByRole('complementary', { name: 'Actions' });
    expect(within(list).getByText('Translate')).toBeTruthy();

    const flow = screen.getByLabelText('Flow');
    expect(within(flow).getByRole('button', { name: 'Input: Selection' })).toBeTruthy();
    expect(within(flow).getByRole('button', { name: 'Model: Local Ollama / llama3' })).toBeTruthy();
    expect(within(flow).getByRole('button', { name: /Output: Confirm → replace/ })).toBeTruthy();
  });

  it('changes the flow from its token popover, never a native select', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionSave.mockResolvedValue(action());
    await renderPage();
    expect(document.querySelectorAll('select')).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Input: Selection' }));
    fireEvent.click(screen.getByRole('button', { name: 'Clipboard' }));
    expect(screen.getByRole('button', { name: 'Input: Clipboard' })).toBeTruthy();
  });

  it('keeps temperature out of sight until Advanced is opened', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    await renderPage();
    const advanced = screen.getByRole('button', { name: /Advanced/ });
    expect(advanced.getAttribute('aria-expanded')).toBe('false');
    // Nothing technical is on the surface: the fold holds it, closed.
    expect(screen.getByText('Temperature').closest('.tvw-fold')).not.toBeNull();
    fireEvent.click(advanced);
    expect(advanced.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('button', { name: '0.7' })).toBeTruthy();
  });

  it('saves itself after an edit, with the exact payload', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionSave.mockResolvedValue(action({ name: 'Rewrite formally' }));
    await renderPage();

    fireEvent.click(screen.getByRole('button', { name: 'Rewrite' }));
    const name = screen.getByLabelText('Action name');
    fireEvent.change(name, { target: { value: 'Rewrite formally' } });
    // No save button anywhere: the studio persists on its own.
    expect(screen.queryByRole('button', { name: /Save action/ })).toBeNull();

    await waitFor(
      () =>
        expect(aiActionSave).toHaveBeenCalledWith({
          id: 'builtin.rewrite',
          name: 'Rewrite formally',
          promptTemplate: 'Rewrite the text in a professional register.',
          providerId: 'p1',
          model: null,
          inputSource: 'selection',
          outputMode: 'replace',
          permissionScope: 'normal_only',
          temperature: null,
        }),
      { timeout: 2000 },
    );
  });

  it('hides delete in the ··· menu, after a divider', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionDelete.mockResolvedValue(undefined);
    await renderPage();

    const composer = screen.getByRole('region', { name: 'Action composer' });
    expect(within(composer).queryByRole('button', { name: /Delete/ })).toBeNull();
    fireEvent.click(within(composer).getByRole('button', { name: 'Action options' }));
    // The menu is portaled out of the composer so no panel can clip it.
    const items = within(screen.getByRole('menu')).getAllByRole('menuitem');
    expect(items[items.length - 1]?.textContent).toBe('Delete');
    await act(async () => {
      fireEvent.click(items[items.length - 1]!);
    });
    expect(aiActionDelete).toHaveBeenCalledWith('builtin.rewrite');
  });

  it('asks what a new action should do instead of opening an empty form', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionSave.mockResolvedValue(action({ id: 'new-1', name: 'Summarize' }));
    await renderPage();

    fireEvent.click(screen.getByRole('button', { name: '＋' }));
    expect(screen.getByText('What should this action do?')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Summarize' }));
    // The template seeds a real draft that saves itself.
    expect(screen.getByLabelText('Instruction')).toHaveProperty(
      'value',
      'Summarize the text below in three sentences.',
    );
    await waitFor(
      () =>
        expect(aiActionSave).toHaveBeenCalledWith(
          expect.objectContaining({ id: null, name: 'Summarize' }),
        ),
      { timeout: 2000 },
    );
  });
});

describe('ActionsPage — the test bench', () => {
  it('stays folded until asked for, then applies nothing until confirmed', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionRun.mockResolvedValue({ output: 'Polished text.', maskedKinds: [] });
    await renderPage();
    expect(screen.queryByLabelText('Test input')).toBeNull();

    const before = await openBench();
    fireEvent.change(before, { target: { value: 'hey — fix going out tmrw' } });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Run/ }));
    });
    expect(aiActionRun).toHaveBeenCalledWith('builtin.rewrite', {
      text: 'hey — fix going out tmrw',
      source: 'selection',
      isSensitive: false,
    });
    expect(screen.getByText('Polished text.')).toBeTruthy();
    expect((before as HTMLTextAreaElement).value).toBe('hey — fix going out tmrw');
    expect(createSnippet).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '↵ Replace input' }));
    });
    expect((before as HTMLTextAreaElement).value).toBe('Polished text.');
  });

  it('names the secret kinds stripped before sending', async () => {
    aiActionList.mockResolvedValue([action({ permissionScope: 'mask_secrets' })]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionRun.mockResolvedValue({ output: 'Done.', maskedKinds: ['aws_access_key'] });
    await renderPage();
    fireEvent.change(await openBench(), { target: { value: 'the key is …' } });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Run/ }));
    });
    expect(screen.getByText(/Secrets stripped before sending · aws_access_key/)).toBeTruthy();
  });

  it('reports a failed run inline, saying what is still unchanged', async () => {
    aiActionList.mockResolvedValue([action()]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionRun.mockRejectedValue(new Error('the model was not found'));
    await renderPage();
    fireEvent.change(await openBench(), { target: { value: 'some text' } });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Run/ }));
    });
    expect(screen.getByText('The provider did not answer')).toBeTruthy();
    expect(screen.getByText(/Your text is unchanged — the model was not found/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Provider settings' })).toBeTruthy();
  });

  it('saves the result as a snippet only on confirm in new_snippet mode', async () => {
    aiActionList.mockResolvedValue([action({ outputMode: 'new_snippet', name: 'Summarize' })]);
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiActionRun.mockResolvedValue({ output: 'A summary.', maskedKinds: [] });
    createSnippet.mockResolvedValue({});
    await renderPage();
    fireEvent.change(await openBench(), { target: { value: 'long text' } });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Run/ }));
    });
    expect(createSnippet).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: '↵ Save as snippet' }));
    });
    expect(createSnippet).toHaveBeenCalledWith({
      title: 'Summarize',
      body: 'A summary.',
      snippetType: 'text',
    });
    expect(screen.getByText(/Saved to your library/)).toBeTruthy();
  });
});
