// @vitest-environment jsdom
import type { AiProvider, OrganizeSuggestion } from '@typvia/shared';
import { afterEach, describe, expect, it } from 'vitest';
import { trFor } from '../i18n';
import { countWords } from './action-labels';
import { egressClassLabel, egressEntryTime, egressTodayCount } from './egress';
import { suggestionFields, type OrganizeCurrent } from './organize-fields';
import { AI_PROVIDER_PREF, chooseProvider, rememberProvider } from './provider-choice';

function provider(id: string): AiProvider {
  return {
    id,
    name: id,
    kind: 'ollama',
    baseUrl: 'http://127.0.0.1:11434',
    model: 'llama3',
    timeoutMs: 30_000,
    hasApiKey: false,
  };
}

function suggestion(overrides: Partial<OrganizeSuggestion> = {}): OrganizeSuggestion {
  return {
    title: null,
    description: null,
    snippetType: null,
    securityLevel: null,
    trigger: null,
    tags: [],
    folder: null,
    ...overrides,
  };
}

const CURRENT: OrganizeCurrent = {
  title: 'Greeting',
  description: null,
  snippetType: 'text',
  trigger: null,
  triggerMode: null,
  folderId: null,
};

describe('chooseProvider', () => {
  afterEach(() => {
    localStorage.clear();
  });

  it('prefers the wanted id, then the remembered one, then list order', () => {
    const providers = [provider('a'), provider('b'), provider('c')];
    expect(chooseProvider(providers, 'b')?.id).toBe('b');
    rememberProvider('c');
    expect(chooseProvider(providers)?.id).toBe('c');
    localStorage.setItem(AI_PROVIDER_PREF, 'gone');
    expect(chooseProvider(providers)?.id).toBe('a');
    expect(chooseProvider([])).toBeNull();
  });
});

describe('suggestionFields', () => {
  it('offers only fields that change something', () => {
    const fields = suggestionFields(
      suggestion({ title: 'Greeting', snippetType: 'markdown', trigger: ';hi' }),
      CURRENT,
    );
    expect(fields.map((f) => f.key)).toEqual(['type', 'trigger']);
    expect(fields[1]?.changes).toEqual({ trigger: ';hi', triggerMode: 'delimiter' });
  });

  it('never offers a trigger over one the user already chose', () => {
    const fields = suggestionFields(suggestion({ trigger: ';hi' }), {
      ...CURRENT,
      trigger: ';mine',
    });
    expect(fields).toEqual([]);
  });

  it('offers the folder by name and applies it by id', () => {
    const fields = suggestionFields(
      suggestion({
        folder: {
          id: 'f1',
          name: 'Work',
          parentId: null,
          sortOrder: 0,
          createdAt: 1,
          updatedAt: 1,
        },
      }),
      CURRENT,
    );
    expect(fields[0]?.value).toBe('Work');
    expect(fields[0]?.changes).toEqual({ folderId: 'f1' });
  });
});

describe('countWords', () => {
  it('counts whitespace-separated words', () => {
    expect(countWords('  one   two\nthree ')).toBe(3);
    expect(countWords('')).toBe(0);
  });
});

describe('egress helpers', () => {
  it('labels known wire classes in the locale and prints unknown ones as-is', () => {
    expect(egressClassLabel('completion', trFor('en'))).toBe('AI request');
    expect(egressClassLabel('connectivity', trFor('en'))).toBe('Connectivity check');
    expect(egressClassLabel('completion', trFor('zh'))).toBe('AI 请求');
    expect(egressClassLabel('connectivity', trFor('zh'))).toBe('联网检查');
    expect(egressClassLabel('embedding', trFor('en'))).toBe('embedding');
    expect(egressClassLabel('embedding', trFor('zh'))).toBe('embedding');
  });

  it('formats the entry time with the calendar of the locale', () => {
    const at = Date.UTC(2026, 2, 4, 12, 30);
    expect(egressEntryTime(at, 'en')).toMatch(/[A-Za-z]{3}/);
    expect(egressEntryTime(at, 'zh')).toMatch(/月/);
  });

  it('counts only entries since local midnight', () => {
    const midnight = new Date();
    midnight.setHours(0, 0, 0, 0);
    const row = (occurredAt: number) => ({
      id: 1,
      occurredAt,
      providerId: 'p1',
      requestClass: 'completion',
      requestBytes: 1,
    });
    expect(
      egressTodayCount([
        row(midnight.getTime()),
        row(midnight.getTime() + 1),
        row(midnight.getTime() - 1),
      ]),
    ).toBe(2);
  });
});
