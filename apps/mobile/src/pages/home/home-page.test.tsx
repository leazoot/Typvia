// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as SharedModule from '@typvia/shared';
import { I18nProvider } from '@typvia/ui';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HomePage } from './home-page';

const ipc = vi.hoisted(() => ({
  listSnippetPage: vi.fn(),
  syncStatus: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...ipc };
});

function snippet(overrides: Partial<Snippet>): Snippet {
  return {
    id: 's-1',
    title: 'Refund reply — CN',
    body: '您好,退款已提交,预计 3–5 个工作日到账。',
    snippetType: 'text',
    securityLevel: 'normal',
    description: null,
    folderId: null,
    trigger: ';refund',
    triggerMode: 'delimiter',
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 1,
    lastUsedAt: 10,
    usageCount: 3,
    version: 1,
    deletedAt: null,
    ...overrides,
  };
}

beforeEach(() => {
  ipc.syncStatus.mockRejectedValue(new Error('not wired in this test'));
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile HomePage', () => {
  it('renders recent strips with the type-word meta line', async () => {
    ipc.listSnippetPage.mockResolvedValue([snippet({})]);
    const { container } = render(
      <HomePage total={12481} onOpenSearch={() => {}} onOpen={() => {}} />,
    );

    expect(await screen.findByText('Refund reply — CN')).toBeDefined();
    // Editorial strip: no trigger slot; the meta line carries the type word.
    expect(screen.queryByText(';refund')).toBeNull();
    expect(container.querySelector('.tv-strip-meta')?.textContent).toContain('Text');
    // Recent strips come from the host's 'recent' (last-used) view.
    expect(ipc.listSnippetPage).toHaveBeenCalledWith('recent', null, null, 20, 0);
  });

  it('shows a locked dots row and no body preview for a sensitive strip', async () => {
    ipc.listSnippetPage.mockResolvedValue([
      snippet({
        id: 's-2',
        title: 'Prod read replica',
        body: null,
        snippetType: 'sensitive',
        securityLevel: 'sensitive',
        trigger: null,
      }),
    ]);
    const onOpen = vi.fn();
    const { container } = render(<HomePage total={1} onOpenSearch={() => {}} onOpen={onOpen} />);

    const title = await screen.findByText('Prod read replica');
    expect(screen.getByText('Locked')).toBeDefined();
    expect(container.querySelector('.tv-strip-dots')).not.toBeNull();
    expect(container.querySelector('.tv-strip-preview')).toBeNull();
    const strip = title.closest('button');
    expect(strip?.getAttribute('aria-disabled')).toBe('true');

    // Tapping a locked strip does nothing — the detail screen never opens.
    fireEvent.click(strip as HTMLElement);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it('opens the detail screen when a normal strip is tapped', async () => {
    ipc.listSnippetPage.mockResolvedValue([snippet({})]);
    const onOpen = vi.fn();
    render(<HomePage total={1} onOpenSearch={() => {}} onOpen={onOpen} />);

    fireEvent.click(await screen.findByText('Refund reply — CN'));
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen.mock.calls[0]?.[0]?.id).toBe('s-1');
  });

  it('opens the search screen from the search line', async () => {
    ipc.listSnippetPage.mockResolvedValue([]);
    const onOpenSearch = vi.fn();
    render(<HomePage total={5} onOpenSearch={onOpenSearch} onOpen={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: 'Search snippets' }));
    expect(onOpenSearch).toHaveBeenCalledTimes(1);
  });

  it('shows skeleton strips while the recent list loads — never a spinner', () => {
    ipc.listSnippetPage.mockReturnValue(new Promise(() => {}));
    const { container } = render(<HomePage total={5} onOpenSearch={() => {}} onOpen={() => {}} />);

    expect(container.querySelectorAll('.tv-strip-skeleton').length).toBeGreaterThan(0);
    expect(container.querySelectorAll('svg, img')).toHaveLength(0);
  });

  it('shows the first-use state when the library is empty', async () => {
    ipc.listSnippetPage.mockResolvedValue([]);
    render(<HomePage total={0} onOpenSearch={() => {}} onOpen={() => {}} />);

    expect(await screen.findByText('No snippets yet')).toBeDefined();
    expect(screen.getByText(/Save your first snippet with the centre button/)).toBeDefined();
  });

  it('says the data is safe first when the recent list fails to load', async () => {
    ipc.listSnippetPage.mockRejectedValueOnce(new Error('io'));
    ipc.listSnippetPage.mockResolvedValueOnce([snippet({})]);
    render(<HomePage total={5} onOpenSearch={() => {}} onOpen={() => {}} />);

    expect(await screen.findByText('Your snippets are safe on this device.')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByText('Refund reply — CN')).toBeDefined();
  });

  it('states only local facts in the status line while sync is off', async () => {
    ipc.listSnippetPage.mockResolvedValue([]);
    render(<HomePage total={5} onOpenSearch={() => {}} onOpen={() => {}} />);

    expect(await screen.findByText('5 snippets on this device')).toBeDefined();
    expect(screen.queryByText(/Synced/)).toBeNull();
  });

  it('renders one language only — Chinese under the zh locale, no twins', async () => {
    ipc.listSnippetPage.mockResolvedValue([]);
    render(
      <I18nProvider locale="zh">
        <HomePage total={0} onOpenSearch={() => {}} onOpen={() => {}} />
      </I18nProvider>,
    );

    expect(await screen.findByText('还没有片段')).toBeDefined();
    expect(screen.getByText(/用中间的新建按钮保存第一条片段/)).toBeDefined();
    expect(screen.queryByText('No snippets yet')).toBeNull();
  });
});
