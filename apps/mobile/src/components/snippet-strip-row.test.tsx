// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SnippetStripRow } from './snippet-strip-row';

afterEach(cleanup);

function snippet(overrides: Partial<Snippet> = {}): Snippet {
  return {
    id: 's-1',
    title: 'Nginx log tail',
    body: 'docker logs -f --tail 200 nginx',
    snippetType: 'command',
    securityLevel: 'normal',
    description: null,
    folderId: null,
    trigger: ';dlog',
    triggerMode: 'delimiter',
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 1,
    lastUsedAt: Date.now() - 2 * 60_000,
    usageCount: 24,
    version: 1,
    deletedAt: null,
    ...overrides,
  };
}

describe('SnippetStripRow', () => {
  it('renders the recency meta line and a mono preview for machine content', () => {
    const { container } = render(<SnippetStripRow snippet={snippet()} onOpen={() => {}} />);

    expect(container.querySelector('.tv-strip-meta')?.textContent).toBe('Command · 2 min');
    expect(container.querySelector('.tv-strip-preview')?.classList.contains('is-mono')).toBe(true);
  });

  it('uses the sans stack for prose previews', () => {
    const { container } = render(
      <SnippetStripRow
        snippet={snippet({ snippetType: 'text', body: 'Best, Lin · Product' })}
        onOpen={() => {}}
      />,
    );
    expect(container.querySelector('.tv-strip-preview')?.classList.contains('is-mono')).toBe(false);
  });

  it('drops the meta line and shows the trailing glyph in the library shape', () => {
    const { container } = render(
      <SnippetStripRow
        snippet={snippet()}
        onOpen={() => {}}
        meta="none"
        trailing={<span>24×</span>}
      />,
    );
    expect(container.querySelector('.tv-strip-meta')).toBeNull();
    expect(screen.getByText('24×')).toBeDefined();
  });

  it('opens on tap for a normal row', () => {
    const onOpen = vi.fn();
    render(<SnippetStripRow snippet={snippet()} onOpen={onOpen} />);
    fireEvent.click(screen.getByRole('button'));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it('renders a sensitive row locked: dots, label, inert', () => {
    const onOpen = vi.fn();
    const { container } = render(
      <SnippetStripRow
        snippet={snippet({ body: null, snippetType: 'sensitive', securityLevel: 'sensitive' })}
        onOpen={onOpen}
      />,
    );

    expect(container.querySelector('.tv-strip-dots')).not.toBeNull();
    expect(screen.getByText('Locked')).toBeDefined();
    expect(container.querySelector('.tv-strip-meta')).toBeNull();
    fireEvent.click(screen.getByRole('button'));
    expect(onOpen).not.toHaveBeenCalled();
  });

  it('speaks Chinese meta under the zh locale', () => {
    const { container } = render(
      <I18nProvider locale="zh">
        <SnippetStripRow snippet={snippet()} onOpen={() => {}} />
      </I18nProvider>,
    );
    expect(container.querySelector('.tv-strip-meta')?.textContent).toBe('命令 · 2 分钟前');
  });
});
