// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import { whenLabel } from './when-label';

// Wednesday 16 September 2026, noon, local time.
const NOW = new Date(2026, 8, 16, 12, 0).getTime();
const at = (month: number, day: number, hour = 9, minute = 41, year = 2026) =>
  new Date(year, month - 1, day, hour, minute).getTime();

describe('whenLabel', () => {
  it('keeps the clock for today, yesterday and the days of this week', () => {
    expect(whenLabel(at(9, 16), NOW, 'zh')).toBe('今天 09:41');
    expect(whenLabel(at(9, 15, 14, 2), NOW, 'zh')).toBe('昨天 14:02');
    expect(whenLabel(at(9, 14, 10, 3), NOW, 'zh')).toBe('周一 10:03');
  });

  it('names last week by its weekday, starting the week on Monday', () => {
    expect(whenLabel(at(9, 11, 17, 30), NOW, 'zh')).toBe('上周五 17:30');
    expect(whenLabel(at(9, 7, 8, 0), NOW, 'en')).toBe('Last Mon 08:00');
    // The Sunday just before this Monday still belongs to last week.
    expect(whenLabel(at(9, 13, 22, 15), NOW, 'en')).toBe('Last Sun 22:15');
  });

  it('drops to a date once the day alone tells moments apart', () => {
    expect(whenLabel(at(8, 29), NOW, 'zh')).toBe('8 月 29 日');
    expect(whenLabel(at(8, 29), NOW, 'en')).toBe('Aug 29');
    expect(whenLabel(at(12, 31, 9, 0, 2025), NOW, 'zh')).toBe('2025 年 12 月 31 日');
    expect(whenLabel(at(12, 31, 9, 0, 2025), NOW, 'en')).toBe('Dec 31, 2025');
  });
});
