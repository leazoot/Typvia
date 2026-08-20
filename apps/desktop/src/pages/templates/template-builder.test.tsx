// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter, Route, Routes } from 'react-router';
import { TemplateBuilder } from './template-builder';

const getSnippet = vi.fn();
const templateFields = vi.fn();
const templateVariables = vi.fn();
const templatePreview = vi.fn();
const templateSaveFields = vi.fn();
const updateSnippet = vi.fn();

vi.mock('@typvia/shared', () => ({
  getSnippet: (id: string) => getSnippet(id) as Promise<unknown>,
  templateFields: (id: string) => templateFields(id) as Promise<unknown>,
  templateVariables: (body: string) => templateVariables(body) as Promise<unknown>,
  templatePreview: (body: string, fields: unknown, values: unknown) =>
    templatePreview(body, fields, values) as Promise<unknown>,
  templateSaveFields: (id: string, fields: unknown) =>
    templateSaveFields(id, fields) as Promise<unknown>,
  updateSnippet: (input: unknown) => updateSnippet(input) as Promise<unknown>,
  // The AI extraction strip only calls these on click; quiet stubs suffice.
  aiProviderList: () => Promise.resolve([]),
  aiExtractVariables: () => Promise.resolve([]),
}));

function snippet(body: string) {
  return {
    id: 's1',
    title: 'Bug report',
    body,
    snippetType: 'template',
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
}

function renderBuilder() {
  return render(
    <MemoryRouter initialEntries={['/templates/s1']}>
      <Routes>
        <Route path="/templates/:id" element={<TemplateBuilder />} />
      </Routes>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('TemplateBuilder', () => {
  it('shows one field card per body variable, in order', async () => {
    getSnippet.mockResolvedValue(snippet('### {{module}} · {{severity}}'));
    templateFields.mockResolvedValue([]);
    templateVariables.mockResolvedValue(['module', 'severity']);
    templatePreview.mockResolvedValue('### ‹module› · ‹severity›');

    const { container } = renderBuilder();

    await waitFor(() => expect(container.querySelectorAll('.tv-field-card')).toHaveLength(2));
    const names = [...container.querySelectorAll('.tv-field-name')].map((n) => n.textContent);
    expect(names).toEqual(['module', 'severity']);
    // The Builder never invents a card that is not in the body.
    expect(screen.queryByText('ghost')).toBeNull();
  });

  it('shows the no-variables guidance when the body has none', async () => {
    getSnippet.mockResolvedValue(snippet('plain text, nothing to fill'));
    templateFields.mockResolvedValue([]);
    templateVariables.mockResolvedValue([]);
    templatePreview.mockResolvedValue('plain text, nothing to fill');

    renderBuilder();

    await waitFor(() => expect(screen.getByText(/No variables yet/)).toBeDefined());
  });

  it('saves the body and the field set together', async () => {
    getSnippet.mockResolvedValue(snippet('### {{module}}'));
    templateFields.mockResolvedValue([]);
    templateVariables.mockResolvedValue(['module']);
    templatePreview.mockResolvedValue('### ‹module›');
    updateSnippet.mockResolvedValue(snippet('### {{module}}'));
    templateSaveFields.mockResolvedValue([
      {
        id: 'f1',
        name: 'module',
        label: 'module',
        fieldType: 'single_line_text',
        defaultValue: null,
        options: [],
        validation: null,
        isRequired: false,
        sortOrder: 0,
        platformOverrides: null,
      },
    ]);

    const { container } = renderBuilder();
    await waitFor(() => expect(container.querySelectorAll('.tv-field-card')).toHaveLength(1));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(templateSaveFields).toHaveBeenCalledTimes(1));
    expect(updateSnippet).toHaveBeenCalledTimes(1);
    expect(templateSaveFields.mock.calls[0]?.[0]).toBe('s1');
  });
});
