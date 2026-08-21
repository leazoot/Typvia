// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRulesSection } from './app-rules-section';

const appRuleList = vi.fn();
const appRuleCreate = vi.fn();
const appRuleUpdate = vi.fn();
const appRuleDelete = vi.fn();
const searchLibrary = vi.fn();

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  appRuleList: (limit: number, offset: number) => appRuleList(limit, offset) as Promise<unknown>,
  appRuleCreate: (input: unknown) => appRuleCreate(input) as Promise<unknown>,
  appRuleUpdate: (id: string, input: unknown) => appRuleUpdate(id, input) as Promise<unknown>,
  appRuleDelete: (id: string) => appRuleDelete(id) as Promise<unknown>,
  searchLibrary: (query: string, limit: number) => searchLibrary(query, limit) as Promise<unknown>,
}));

const RULE = {
  id: 'r-1',
  snippetId: 's-1',
  snippetTitle: 'Sig',
  platform: 'macos',
  appIdentifier: 'com.google.Chrome',
  ruleType: 'disable',
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('AppRulesSection', () => {
  it('shows the empty state when there are no rules', async () => {
    appRuleList.mockResolvedValue([]);
    render(<AppRulesSection />);
    expect(await screen.findByText('No rules yet.')).toBeDefined();
  });

  it('lists a rule with its snippet, app, and effect', async () => {
    appRuleList.mockResolvedValue([RULE]);
    render(<AppRulesSection />);
    const list = within(await screen.findByRole('list', { name: 'App rules' }));
    expect(list.getByText('Sig')).toBeDefined();
    expect(list.getByText('com.google.Chrome')).toBeDefined();
    expect(list.getByText('Hide here')).toBeDefined();
  });

  it('creates a rule from a picked snippet', async () => {
    appRuleList.mockResolvedValue([]);
    searchLibrary.mockResolvedValue([{ id: 's-1', title: 'Sig' }]);
    appRuleCreate.mockResolvedValue(RULE);
    render(<AppRulesSection />);
    await screen.findByText('No rules yet.');

    fireEvent.change(screen.getByLabelText('Find a snippet'), { target: { value: 'si' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Sig' }));
    fireEvent.change(screen.getByLabelText('App identifier'), {
      target: { value: 'com.google.Chrome' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add rule' }));

    await waitFor(() =>
      expect(appRuleCreate).toHaveBeenCalledWith({
        snippetId: 's-1',
        appIdentifier: 'com.google.Chrome',
        ruleType: 'disable',
      }),
    );
  });

  it('reports a refused create without pretending success', async () => {
    appRuleList.mockResolvedValue([]);
    searchLibrary.mockResolvedValue([{ id: 's-1', title: 'Sig' }]);
    appRuleCreate.mockRejectedValue(new Error('this rule already exists'));
    render(<AppRulesSection />);
    await screen.findByText('No rules yet.');

    fireEvent.change(screen.getByLabelText('Find a snippet'), { target: { value: 'si' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Sig' }));
    fireEvent.change(screen.getByLabelText('App identifier'), {
      target: { value: 'com.google.Chrome' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add rule' }));

    expect(await screen.findByText('No rule was added — this rule already exists.')).toBeDefined();
  });

  it('removes a rule only after the inline confirm', async () => {
    appRuleList.mockResolvedValue([RULE]);
    appRuleDelete.mockResolvedValue(undefined);
    render(<AppRulesSection />);

    fireEvent.click(await screen.findByRole('button', { name: 'Remove' }));
    expect(appRuleDelete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Remove rule' }));
    await waitFor(() => expect(appRuleDelete).toHaveBeenCalledWith('r-1'));
  });

  it('edits a rule inline', async () => {
    appRuleList.mockResolvedValue([RULE]);
    appRuleUpdate.mockResolvedValue({ ...RULE, ruleType: 'show_only' });
    render(<AppRulesSection />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    // The editor's input renders above the add form's; both share the label.
    const editorInput = screen.getAllByLabelText('App identifier')[0]!;
    fireEvent.change(editorInput, { target: { value: 'com.apple.Safari' } });
    fireEvent.click(screen.getAllByRole('button', { name: 'Show only here' })[0]!);
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(appRuleUpdate).toHaveBeenCalledWith('r-1', {
        appIdentifier: 'com.apple.Safari',
        ruleType: 'show_only',
      }),
    );
  });
});
