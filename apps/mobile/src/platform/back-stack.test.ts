// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { installAndroidBackBridge, pushBackHandler } from './back-stack';

function fireHostBack(): boolean {
  const hook = window.__TYPVIA_ANDROID_BACK__;
  if (hook === undefined) throw new Error('back bridge not installed');
  return hook();
}

afterEach(() => {
  delete window.__TYPVIA_ANDROID_BACK__;
});

describe('Android back bridge', () => {
  it('reports the gesture unconsumed while nothing is registered', () => {
    installAndroidBackBridge();

    expect(fireHostBack()).toBe(false);
  });

  it('routes the gesture to the newest handler only', () => {
    installAndroidBackBridge();
    const below = vi.fn();
    const top = vi.fn();
    const releaseBelow = pushBackHandler(below);
    const releaseTop = pushBackHandler(top);

    expect(fireHostBack()).toBe(true);
    expect(top).toHaveBeenCalledTimes(1);
    expect(below).not.toHaveBeenCalled();

    releaseTop();
    releaseBelow();
  });

  it('a released handler no longer receives the gesture', () => {
    installAndroidBackBridge();
    const handler = vi.fn();
    const release = pushBackHandler(handler);
    release();

    expect(fireHostBack()).toBe(false);
    expect(handler).not.toHaveBeenCalled();
  });

  it('releasing a lower handler keeps the top one in place', () => {
    installAndroidBackBridge();
    const below = vi.fn();
    const top = vi.fn();
    const releaseBelow = pushBackHandler(below);
    const releaseTop = pushBackHandler(top);
    releaseBelow();

    expect(fireHostBack()).toBe(true);
    expect(top).toHaveBeenCalledTimes(1);

    releaseTop();
    expect(fireHostBack()).toBe(false);
  });
});
