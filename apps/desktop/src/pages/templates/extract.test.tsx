// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AiProvider, VariableProposal } from '@typvia/shared';
import { Extract } from './extract';

const aiProviderList = vi.fn();
const aiExtractVariables = vi.fn();
const templateSaveFields = vi.fn();
const updateSnippet = vi.fn();

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  aiProviderList: () => aiProviderList() as Promise<AiProvider[]>,
  aiExtractVariables: (...args: unknown[]) =>
    aiExtractVariables(...args) as Promise<VariableProposal[]>,
  templateSaveFields: (...args: unknown[]) => templateSaveFields(...args) as Promise<unknown>,
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

const PROPOSALS: VariableProposal[] = [
  { name: '技术栈', original: 'React', fieldType: 'single_line_text', defaultValue: 'React' },
  { name: '模块名称', original: '登录模块', fieldType: 'single_line_text', defaultValue: null },
];

const BODY = '请检查 React 项目的登录模块。';

function renderExtract(onApply = vi.fn(), body = BODY) {
  render(
    <MemoryRouter>
      <Extract body={body} onApply={onApply} />
    </MemoryRouter>,
  );
  return onApply;
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

describe('Extract strip', () => {
  it('is disabled while the text is blank', () => {
    renderExtract(vi.fn(), '  ');
    const run = screen.getByRole('button', { name: /Extract variables/ });
    expect((run as HTMLButtonElement).disabled).toBe(true);
  });

  it('says hand-marking still works when no provider is configured', async () => {
    aiProviderList.mockResolvedValue([]);
    renderExtract();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Extract variables/ }));
    });
    expect(screen.getByRole('status').textContent).toContain('Marking by hand still works');
    expect(aiExtractVariables).not.toHaveBeenCalled();
  });

  it('reports failure with the text-is-unchanged framing', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiExtractVariables.mockRejectedValue(new Error('the provider could not be reached'));
    renderExtract();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Extract variables/ }));
    });
    expect(screen.getByRole('status').textContent).toContain('Your text is unchanged');
  });

  it('offers proposals one by one and never persists anything itself', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiExtractVariables.mockResolvedValue(PROPOSALS);
    const onApply = renderExtract();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Extract variables/ }));
    });
    // Proposals arrived; nothing was applied and nothing was saved.
    expect(onApply).not.toHaveBeenCalled();
    expect(templateSaveFields).not.toHaveBeenCalled();
    expect(updateSnippet).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: /技术栈/ }));
    expect(onApply).toHaveBeenCalledWith(PROPOSALS[0]);
    // The applied proposal leaves the list; the other stays offered.
    expect(screen.queryByRole('button', { name: /技术栈/ })).toBeNull();
    expect(screen.getByRole('button', { name: /模块名称/ })).toBeDefined();
    expect(templateSaveFields).not.toHaveBeenCalled();
  });

  it('says so plainly when there is nothing to extract', async () => {
    aiProviderList.mockResolvedValue([PROVIDER]);
    aiExtractVariables.mockResolvedValue([]);
    renderExtract();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Extract variables/ }));
    });
    expect(screen.getByRole('status').textContent).toContain('Nothing to extract');
  });
});
