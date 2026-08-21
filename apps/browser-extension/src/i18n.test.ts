// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import { isZh, makeTr } from './i18n';

describe('extension single-language rendering', () => {
  it('keys off the browser language, zh variants included', () => {
    expect(isZh('zh-CN')).toBe(true);
    expect(isZh('zh-Hans-CN')).toBe(true);
    expect(isZh('ZH-TW')).toBe(true);
    expect(isZh('en-US')).toBe(false);
    expect(isZh('ja')).toBe(false);
  });

  it('renders exactly one language, never both', () => {
    expect(makeTr('zh-CN')('Search', '搜索')).toBe('搜索');
    expect(makeTr('en-US')('Search', '搜索')).toBe('Search');
  });
});
