// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { PanelApp } from './panel-app';

const hidePanel = vi.fn(() => Promise.resolve());
const panelReady = vi.fn(() => Promise.resolve());

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    hidePanel: () => hidePanel(),
    panelReady: () => panelReady(),
  };
});

// Capture the panel:show subscriber so a test can fire a summon.
let showHandler: (() => void) | null = null;
vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, cb: () => void) => {
    if (event === 'panel:show') showHandler = cb;
    return Promise.resolve(() => undefined);
  },
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  showHandler = null;
});

describe('PanelApp', () => {
  it('focuses the search field at frame 0', () => {
    render(<PanelApp />);
    expect(document.activeElement).toBe(screen.getByLabelText('Search snippets'));
  });

  it('hides the panel on Escape', () => {
    render(<PanelApp />);
    fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Escape' });
    expect(hidePanel).toHaveBeenCalledTimes(1);
  });

  it('clears the query and re-focuses on each summon', () => {
    render(<PanelApp />);
    const input = screen.getByLabelText('Search snippets');
    fireEvent.change(input, { target: { value: 'docker' } });
    expect((input as HTMLInputElement).value).toBe('docker');

    act(() => {
      showHandler?.();
    });

    const refocused = screen.getByLabelText('Search snippets');
    expect((refocused as HTMLInputElement).value).toBe('');
    expect(document.activeElement).toBe(refocused);
  });
});
