// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SearchLine } from './search-line';
import { ListTray, SelectionPlate, SnippetRow } from './snippet-row';
import { StatusDot } from './status-dot';
import { Toast } from './toast';
import { TopNav } from './top-nav';
import { TypeMark } from './type-mark';

afterEach(cleanup);

const NAV_ITEMS = [
  { key: '/', label: 'Home' },
  { key: '/library', label: 'Library' },
];

describe('TopNav', () => {
  it('marks the active item with aria-current and fires onNavigate', () => {
    const onNavigate = vi.fn();
    render(<TopNav items={NAV_ITEMS} activeKey="/library" onNavigate={onNavigate} />);
    const active = screen.getByRole('button', { name: 'Library' });
    expect(active.getAttribute('aria-current')).toBe('page');
    expect(screen.getByRole('button', { name: 'Home' }).getAttribute('aria-current')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Home' }));
    expect(onNavigate).toHaveBeenCalledWith('/');
  });

  it('renders text only — no icons anywhere in navigation', () => {
    const { container } = render(<TopNav items={NAV_ITEMS} activeKey="/" onNavigate={() => {}} />);
    expect(container.querySelectorAll('svg, img')).toHaveLength(0);
  });
});

describe('SearchLine', () => {
  it('is a real input with the decorative caret only while empty', () => {
    const onChange = vi.fn();
    const { container, rerender } = render(
      <SearchLine
        scale="library"
        value=""
        onChange={onChange}
        placeholder="Search"
        label="Search snippets"
      />,
    );
    expect(container.querySelector('.tv-caret')).not.toBeNull();

    const input = screen.getByRole('textbox', { name: 'Search snippets' });
    fireEvent.change(input, { target: { value: 'docker' } });
    expect(onChange).toHaveBeenCalledWith('docker');

    rerender(
      <SearchLine
        scale="library"
        value="docker"
        onChange={onChange}
        placeholder="Search"
        label="Search snippets"
      />,
    );
    expect(container.querySelector('.tv-caret')).toBeNull();
  });

  it('carries its scale as a class for the type-token mapping', () => {
    const { container } = render(
      <SearchLine scale="hero" value="" onChange={() => {}} placeholder="Search" label="Search" />,
    );
    expect(container.querySelector('.tv-search-line-hero')).not.toBeNull();
  });
});

describe('TypeMark', () => {
  it('announces the full word, never the two letters', () => {
    render(<TypeMark code="CM" />);
    expect(screen.getByRole('img', { name: 'Command' })).toBeDefined();
    expect(screen.queryByRole('img', { name: 'CM' })).toBeNull();
  });

  it('tints only the Secret mark', () => {
    const { container } = render(
      <>
        <TypeMark code="SC" />
        <TypeMark code="TX" />
      </>,
    );
    expect(container.querySelectorAll('.tv-type-mark.is-secret')).toHaveLength(1);
  });
});

describe('SnippetRow and SelectionPlate', () => {
  it('shows the trigger at rest and ↵ + action + caret when selected', () => {
    const { rerender } = render(
      <SnippetRow mark="CM" title="Docker tail logs" trigger=";dockerlog" />,
    );
    expect(screen.getByText(';dockerlog')).toBeDefined();

    rerender(<SnippetRow mark="CM" title="Docker tail logs" trigger=";dockerlog" selected />);
    expect(screen.queryByText(';dockerlog')).toBeNull();
    expect(screen.getByText('↵')).toBeDefined();
    expect(screen.getByText('Open')).toBeDefined();
  });

  it('travels as one plate translated by row height, and hides with no selection', () => {
    const { rerender } = render(
      <ListTray>
        <SelectionPlate index={3} />
      </ListTray>,
    );
    expect(screen.getByTestId('selection-plate').style.transform).toBe('translateY(156px)');

    rerender(
      <ListTray>
        <SelectionPlate index={null} />
      </ListTray>,
    );
    expect(screen.queryByTestId('selection-plate')).toBeNull();
  });
});

describe('StatusDot', () => {
  it('always pairs the colour with a word', () => {
    render(<StatusDot kind="warning" label="Needs setup · 待配置" />);
    expect(screen.getByText('Needs setup · 待配置')).toBeDefined();
  });

  it('gives security a square outline class, distinct from state dots', () => {
    const { container } = render(
      <>
        <StatusDot kind="secure" label="Secure" />
        <StatusDot kind="success" label="Synced" />
      </>,
    );
    expect(container.querySelectorAll('.tv-status-secure')).toHaveLength(1);
    expect(container.querySelectorAll('.tv-status-success')).toHaveLength(1);
  });
});

describe('Toast', () => {
  it('is a polite status by default and assertive for inserts', () => {
    const { rerender } = render(<Toast message="Saved" />);
    expect(screen.getByRole('status').getAttribute('aria-live')).toBe('polite');

    rerender(<Toast message="Inserted into VS Code" assertive />);
    const toast = screen.getByRole('status');
    expect(toast.getAttribute('aria-live')).toBe('assertive');
    expect(toast.textContent).toContain('Inserted into VS Code');
  });
});
