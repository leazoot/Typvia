// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { I18nProvider } from '../i18n';
import { SearchLine } from './search-line';
import { StatusDot } from './status-dot';
import { Toast } from './toast';
import { TypeMark } from './type-mark';

afterEach(cleanup);

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
    // No decorative caret ever — the native input caret is the only cursor.
    expect(container.querySelector('.tv-caret')).toBeNull();

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

  it('announces the Chinese full word under the zh locale, mark unchanged', () => {
    render(
      <I18nProvider locale="zh">
        <TypeMark code="CM" />
      </I18nProvider>,
    );
    const mark = screen.getByRole('img', { name: '命令' });
    expect(mark.textContent).toBe('CM');
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
