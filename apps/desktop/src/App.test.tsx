// @vitest-environment jsdom
import { APP_ROUTES } from '@typvia/shared';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { App } from './App';

// IPC is mocked at the typed wrapper layer (frontend testing rule) so the
// real Library screen can mount without a Tauri host.
vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    libraryCounts: () =>
      Promise.resolve({ total: 0, recent: 0, starred: 0, unsorted: 0, folders: [] }),
    countSnippets: () => Promise.resolve(0),
    listSnippetPage: () => Promise.resolve([]),
    listFolderChildren: () => Promise.resolve([]),
    listTags: () => Promise.resolve([]),
  };
});

afterEach(cleanup);

describe('app shell routing', () => {
  it('renders a navigation item for every top-level route', () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    for (const route of APP_ROUTES) {
      expect(screen.getByRole('button', { name: route.labelEn })).toBeDefined();
    }
  });

  it('navigates when a nav item is clicked and marks it current', () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Vault' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Vault' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Vault' }).getAttribute('aria-current')).toBe('page');
  });

  it('reaches the real Library screen at /library', async () => {
    render(
      <MemoryRouter initialEntries={['/library']}>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByLabelText('Search snippets')).toBeDefined();
  });

  it.each(
    APP_ROUTES.filter((route) => route.path !== '/library').map((route) => [route.labelEn, route]),
  )('reaches the %s placeholder page at its route', (_label, route) => {
    render(
      <MemoryRouter initialEntries={[route.path]}>
        <App />
      </MemoryRouter>,
    );
    expect(screen.getByRole('heading', { level: 1, name: route.labelEn })).toBeDefined();
    expect(screen.getByText(route.labelCn)).toBeDefined();
  });
});
