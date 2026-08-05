// @vitest-environment jsdom
import { APP_ROUTES } from '@typvia/shared';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it } from 'vitest';
import { App } from './App';

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

  it.each(APP_ROUTES.map((route) => [route.labelEn, route]))(
    'reaches the %s placeholder page at its route',
    (_label, route) => {
      render(
        <MemoryRouter initialEntries={[route.path]}>
          <App />
        </MemoryRouter>,
      );
      expect(screen.getByRole('heading', { level: 1, name: route.labelEn })).toBeDefined();
      expect(screen.getByText(route.labelCn)).toBeDefined();
    },
  );
});
