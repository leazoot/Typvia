// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import {
  biometricWordFor,
  biometricWordZhFor,
  deviceWordFor,
  isAndroidUserAgent,
} from './platform';

describe('platform detection', () => {
  it('recognizes the Android WebView user agent', () => {
    const androidWebView =
      'Mozilla/5.0 (Linux; Android 14; Pixel 3a Build/UQ1A.240105.004; wv) AppleWebKit/537.36';
    expect(isAndroidUserAgent(androidWebView)).toBe(true);
  });

  it('treats the iOS WebKit user agent as non-Android', () => {
    const iosWebKit = 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15';
    expect(isAndroidUserAgent(iosWebKit)).toBe(false);
  });

  it('names the platform-appropriate device word', () => {
    expect(deviceWordFor(true)).toBe('phone');
    expect(deviceWordFor(false)).toBe('iPhone');
  });

  it('names the platform-appropriate biometric gate in both languages', () => {
    expect(biometricWordFor(true)).toBe('Fingerprint');
    expect(biometricWordFor(false)).toBe('Face ID');
    expect(biometricWordZhFor(true)).toBe('指纹');
    expect(biometricWordZhFor(false)).toBe('Face ID');
  });
});
