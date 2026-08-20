// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShareIngestor } from './share-ingest';

function visibility(state: DocumentVisibilityState) {
  Object.defineProperty(document, 'visibilityState', {
    configurable: true,
    get: () => state,
  });
  document.dispatchEvent(new Event('visibilitychange'));
}

/** Flushes the drain's settled promise chain (then/catch/finally). */
function flush() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

afterEach(() => {
  Object.defineProperty(document, 'visibilityState', {
    configurable: true,
    get: () => 'visible',
  });
});

describe('createShareIngestor', () => {
  it('drains once immediately on start and reports a positive count', async () => {
    const ingest = vi.fn().mockResolvedValue(3);
    const onIngested = vi.fn();
    const ingestor = createShareIngestor(onIngested, ingest);

    ingestor.start();
    await vi.waitFor(() => {
      expect(onIngested).toHaveBeenCalledWith(3);
    });
    expect(ingest).toHaveBeenCalledTimes(1);
    ingestor.stop();
  });

  it('stays silent when the drain finds nothing', async () => {
    const ingest = vi.fn().mockResolvedValue(0);
    const onIngested = vi.fn();
    const ingestor = createShareIngestor(onIngested, ingest);

    ingestor.start();
    await vi.waitFor(() => {
      expect(ingest).toHaveBeenCalled();
    });
    expect(onIngested).not.toHaveBeenCalled();
    ingestor.stop();
  });

  it('drains again when the app returns to the foreground, not when it hides', async () => {
    const ingest = vi.fn().mockResolvedValue(0);
    const ingestor = createShareIngestor(() => undefined, ingest);
    ingestor.start();
    expect(ingest).toHaveBeenCalledTimes(1);
    await flush();

    visibility('hidden');
    expect(ingest).toHaveBeenCalledTimes(1);

    visibility('visible');
    await vi.waitFor(() => {
      expect(ingest).toHaveBeenCalledTimes(2);
    });
    ingestor.stop();
  });

  it('swallows drain failures (the inbox files retry on the next pass)', async () => {
    const ingest = vi.fn().mockRejectedValue(new Error('internal storage error'));
    const onIngested = vi.fn();
    const ingestor = createShareIngestor(onIngested, ingest);

    ingestor.start();
    await flush();
    expect(ingest).toHaveBeenCalled();
    expect(onIngested).not.toHaveBeenCalled();
    // A failed drain releases the in-flight guard: the next foreground
    // return tries again.
    visibility('visible');
    await vi.waitFor(() => {
      expect(ingest).toHaveBeenCalledTimes(2);
    });
    ingestor.stop();
  });

  it('ignores results and foreground returns after stop', async () => {
    let resolveDrain: (count: number) => void = () => undefined;
    const ingest = vi.fn(
      () =>
        new Promise<number>((resolve) => {
          resolveDrain = resolve;
        }),
    );
    const onIngested = vi.fn();
    const ingestor = createShareIngestor(onIngested, ingest);
    ingestor.start();

    ingestor.stop();
    resolveDrain(4);
    await Promise.resolve();
    expect(onIngested).not.toHaveBeenCalled();

    visibility('visible');
    expect(ingest).toHaveBeenCalledTimes(1);
  });
});
