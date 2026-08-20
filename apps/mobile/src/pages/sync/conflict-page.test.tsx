// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as SharedModule from '@typvia/shared';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MobileConflictPage } from './conflict-page';

const ipc = vi.hoisted(() => ({
  syncConflicts: vi.fn(),
  syncConflictResolve: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...ipc };
});

function snippet(id: string, body: string | null): SharedModule.Snippet {
  return {
    id,
    title: 'Weekly update',
    body,
    snippetType: 'text',
    securityLevel: body === null ? 'sensitive' : 'normal',
    description: null,
    folderId: null,
    trigger: ';weekly',
    triggerMode: 'delimiter',
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 2,
    lastUsedAt: null,
    usageCount: 0,
    version: 2,
    deletedAt: null,
  };
}

const noop = () => undefined;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile conflict arbitration', () => {
  it('stacks both versions whole and keeps Decide later a first-class action', async () => {
    ipc.syncConflicts.mockResolvedValue([
      {
        source: snippet('s-1', 'Version A body'),
        copy: snippet('s-2', 'Version B body'),
        sensitive: false,
      },
    ]);
    let backs = 0;

    render(
      <MobileConflictPage
        onBack={() => {
          backs += 1;
        }}
      />,
    );

    expect(await screen.findByLabelText('The version in use')).toBeDefined();
    expect(screen.getByLabelText('The version set aside')).toBeDefined();
    expect(screen.getByText('Version A body')).toBeDefined();
    expect(screen.getByText('Version B body')).toBeDefined();

    fireEvent.click(
      screen.getByRole('button', { name: 'Decide later — both stay on their devices' }),
    );
    expect(backs).toBe(1);
    expect(ipc.syncConflictResolve).not.toHaveBeenCalled();
  });

  it('records the decision against the copy, whichever side wins', async () => {
    ipc.syncConflicts.mockResolvedValue([
      {
        source: snippet('s-1', 'Version A body'),
        copy: snippet('s-2', 'Version B body'),
        sensitive: false,
      },
    ]);
    ipc.syncConflictResolve.mockResolvedValue(undefined);

    render(<MobileConflictPage onBack={noop} />);

    const keep = await screen.findAllByRole('button', { name: 'Keep this one' });
    fireEvent.click(keep[1] as HTMLElement);

    await waitFor(() => {
      expect(ipc.syncConflictResolve).toHaveBeenCalledWith('s-2', 'copy');
    });
  });

  it('never puts a sensitive body in the page while comparing', async () => {
    ipc.syncConflicts.mockResolvedValue([
      { source: snippet('s-1', null), copy: snippet('s-2', null), sensitive: true },
    ]);

    const { container } = render(<MobileConflictPage onBack={noop} />);

    expect(await screen.findByLabelText('The version in use')).toBeDefined();
    expect(container.textContent).not.toContain('Version A body');
    expect(screen.getAllByText('encrypted · not shown')).toHaveLength(2);
  });

  it('says there is nothing left to decide once the list is empty', async () => {
    ipc.syncConflicts.mockResolvedValue([]);

    render(<MobileConflictPage onBack={noop} />);

    expect(await screen.findByText('Nothing left to decide.')).toBeDefined();
  });
});
