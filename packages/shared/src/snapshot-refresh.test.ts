import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SNAPSHOT_REFRESH_DEBOUNCE_MS, createSnapshotRefresher } from './snapshot-refresh';

describe('createSnapshotRefresher', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('refreshes once on the trailing edge of the debounce window', async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    const refresher = createSnapshotRefresher(refresh);

    refresher.notifyMutation();
    await vi.advanceTimersByTimeAsync(SNAPSHOT_REFRESH_DEBOUNCE_MS - 1);
    expect(refresh).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('coalesces a burst of mutations into a single refresh', async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    const refresher = createSnapshotRefresher(refresh);

    refresher.notifyMutation();
    await vi.advanceTimersByTimeAsync(500);
    refresher.notifyMutation();
    await vi.advanceTimersByTimeAsync(500);
    refresher.notifyMutation();
    await vi.advanceTimersByTimeAsync(SNAPSHOT_REFRESH_DEBOUNCE_MS);

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('cancel drops a pending refresh', async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    const refresher = createSnapshotRefresher(refresh);

    refresher.notifyMutation();
    refresher.cancel();
    await vi.advanceTimersByTimeAsync(SNAPSHOT_REFRESH_DEBOUNCE_MS * 2);

    expect(refresh).not.toHaveBeenCalled();
  });

  it('absorbs a failed refresh and schedules again on the next mutation', async () => {
    const refresh = vi.fn().mockRejectedValueOnce(new Error('write failed'));
    refresh.mockResolvedValue(undefined);
    const refresher = createSnapshotRefresher(refresh);

    refresher.notifyMutation();
    await vi.advanceTimersByTimeAsync(SNAPSHOT_REFRESH_DEBOUNCE_MS);
    refresher.notifyMutation();
    await vi.advanceTimersByTimeAsync(SNAPSHOT_REFRESH_DEBOUNCE_MS);

    expect(refresh).toHaveBeenCalledTimes(2);
  });
});
