// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EspansoProvider, useEspanso } from '../espanso/espanso-context';
import {
  BrowserIntegrationProvider,
  notifyBrowserIntegrationMutation,
  useBrowserIntegration,
} from './browser-integration-context';

const browserIntegrationStatus = vi.fn();
const browserIntegrationEnable = vi.fn();
const browserIntegrationDisable = vi.fn();
const browserIntegrationSync = vi.fn();
const espansoStatus = vi.fn();
const espansoSync = vi.fn();

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  browserIntegrationStatus: () => browserIntegrationStatus() as Promise<unknown>,
  browserIntegrationEnable: () => browserIntegrationEnable() as Promise<unknown>,
  browserIntegrationDisable: () => browserIntegrationDisable() as Promise<unknown>,
  browserIntegrationSync: () => browserIntegrationSync() as Promise<unknown>,
  espansoStatus: () => espansoStatus() as Promise<unknown>,
  espansoSync: () => espansoSync() as Promise<unknown>,
}));

function StatusProbe() {
  const { status } = useBrowserIntegration();
  return <output>{status === null ? 'unknown' : status.enabled ? 'on' : 'off'}</output>;
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

beforeEach(() => {
  browserIntegrationStatus.mockResolvedValue({ enabled: false, hostInstalled: false });
  browserIntegrationSync.mockResolvedValue(undefined);
  espansoStatus.mockResolvedValue({
    state: 'running',
    version: '2.2.1',
    configPath: '/tmp/typvia.yml',
    enabled: false,
    triggerCount: 0,
  });
});

describe('BrowserIntegrationProvider', () => {
  it('loads the switch status on mount', async () => {
    render(
      <BrowserIntegrationProvider>
        <StatusProbe />
      </BrowserIntegrationProvider>,
    );
    await waitFor(() => expect(screen.getByRole('status').textContent).toBe('off'));
  });
});

describe('notifyBrowserIntegrationMutation', () => {
  it('coalesces a burst of mutations into one snapshot sync', async () => {
    vi.useFakeTimers();
    notifyBrowserIntegrationMutation();
    notifyBrowserIntegrationMutation();
    notifyBrowserIntegrationMutation();
    expect(browserIntegrationSync).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(browserIntegrationSync).toHaveBeenCalledTimes(1);
  });

  it('fires from the espanso mutation signal even while espanso is off', async () => {
    // The snapshot poke must not sit behind the espanso enabled gate: the
    // browser integration can be on while espanso is off, and the host
    // command no-ops in the opposite case.
    let notify: (() => void) | null = null;
    function Grab() {
      notify = useEspanso().notifyMutation;
      return null;
    }
    render(
      <EspansoProvider>
        <Grab />
      </EspansoProvider>,
    );
    await waitFor(() => expect(notify).not.toBeNull());

    vi.useFakeTimers();
    act(() => notify?.());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    expect(browserIntegrationSync).toHaveBeenCalledTimes(1);
    expect(espansoSync).not.toHaveBeenCalled();
  });
});
