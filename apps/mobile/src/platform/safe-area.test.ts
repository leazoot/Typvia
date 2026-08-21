// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { syncAndroidSafeArea } from './safe-area';

afterEach(() => {
  delete window.TypviaInsets;
  delete window.__TYPVIA_INSETS_CHANGED__;
});

describe('syncAndroidSafeArea', () => {
  it('is a no-op without the host bridge (iOS, dev shell)', () => {
    const target = document.createElement('div');
    syncAndroidSafeArea(target);

    expect(target.style.getPropertyValue('--tv-inset-top')).toBe('');
    expect(window.__TYPVIA_INSETS_CHANGED__).toBeUndefined();
  });

  it('mirrors the host insets into the --tv-inset-* variables', () => {
    window.TypviaInsets = { cssPx: () => '24,48' };
    const target = document.createElement('div');
    syncAndroidSafeArea(target);

    expect(target.style.getPropertyValue('--tv-inset-top')).toBe('24px');
    expect(target.style.getPropertyValue('--tv-inset-bottom')).toBe('48px');
  });

  it('re-reads the bridge when the host pushes an inset change', () => {
    let answer = '24,48';
    window.TypviaInsets = { cssPx: () => answer };
    const target = document.createElement('div');
    syncAndroidSafeArea(target);

    answer = '0,32';
    window.__TYPVIA_INSETS_CHANGED__?.();
    expect(target.style.getPropertyValue('--tv-inset-top')).toBe('0px');
    expect(target.style.getPropertyValue('--tv-inset-bottom')).toBe('32px');
  });

  it('keeps the previous values when the bridge answer is malformed', () => {
    let answer = '24,48';
    window.TypviaInsets = { cssPx: () => answer };
    const target = document.createElement('div');
    syncAndroidSafeArea(target);

    answer = 'garbage';
    window.__TYPVIA_INSETS_CHANGED__?.();
    expect(target.style.getPropertyValue('--tv-inset-top')).toBe('24px');
    expect(target.style.getPropertyValue('--tv-inset-bottom')).toBe('48px');
  });
});
