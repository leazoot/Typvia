// @vitest-environment jsdom
import { APP_ROUTES } from '@typvia/shared';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it } from 'vitest';
import { App } from './App';

afterEach(cleanup);

describe('app shell routing', () => {
  it('renders a navigation link for every top-level route', () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    for (const route of APP_ROUTES) {
      expect(screen.getByRole('link', { name: route.labelEn })).toBeDefined();
    }
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
