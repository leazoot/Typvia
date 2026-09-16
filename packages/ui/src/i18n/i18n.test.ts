// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import { counted, trFor } from './index';

describe('counted', () => {
  it('says the singular when there is one of something', () => {
    expect(counted(trFor('en'), 1, 'result', 'results', '1 条结果')).toBe('1 result');
  });

  it('says the plural for none and for many', () => {
    expect(counted(trFor('en'), 0, 'result', 'results', '0 条结果')).toBe('0 results');
    expect(counted(trFor('en'), 2, 'result', 'results', '2 条结果')).toBe('2 results');
  });

  /** Chinese has no plural, so its half is passed whole and never branches. */
  it('renders the Chinese half exactly as it was written', () => {
    expect(counted(trFor('zh'), 1, 'result', 'results', '1 条结果')).toBe('1 条结果');
    expect(counted(trFor('zh'), 9, 'result', 'results', '9 条结果')).toBe('9 条结果');
  });
});
