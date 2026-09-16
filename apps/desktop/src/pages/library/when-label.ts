// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { trFor, type Locale } from '@typvia/ui';

const WEEKDAYS_ZH = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];
const WEEKDAYS_EN = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTHS_EN = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

function pad(value: number): string {
  return String(value).padStart(2, '0');
}

/** Local midnight `days` days away from the day `at` falls on. */
function midnight(at: number, days = 0): number {
  const date = new Date(at);
  date.setHours(0, 0, 0, 0);
  date.setDate(date.getDate() + days);
  return date.getTime();
}

/** Monday-first, the way both the Chinese and the ISO calendar read a week. */
function weekStart(at: number, weeks = 0): number {
  const sinceMonday = (new Date(at).getDay() + 6) % 7;
  return midnight(at, -sinceMonday + weeks * 7);
}

/**
 * When something happened, the way the library says it: "今天 09:41",
 * "昨天 14:02", "周一 10:03", "上周五 17:30", then a plain date. The clock is
 * kept while it still helps tell two moments apart and dropped once the day
 * alone does.
 */
export function whenLabel(at: number, now: number, locale: Locale): string {
  const tr = trFor(locale);
  const date = new Date(at);
  const clock = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  const weekdayZh = WEEKDAYS_ZH[date.getDay()] ?? '';
  const weekdayEn = WEEKDAYS_EN[date.getDay()] ?? '';

  if (at >= midnight(now)) return tr(`Today ${clock}`, `今天 ${clock}`);
  if (at >= midnight(now, -1)) return tr(`Yesterday ${clock}`, `昨天 ${clock}`);
  if (at >= weekStart(now)) return tr(`${weekdayEn} ${clock}`, `${weekdayZh} ${clock}`);
  if (at >= weekStart(now, -1)) return tr(`Last ${weekdayEn} ${clock}`, `上${weekdayZh} ${clock}`);

  const month = date.getMonth() + 1;
  const day = date.getDate();
  const monthEn = MONTHS_EN[date.getMonth()] ?? '';
  if (date.getFullYear() === new Date(now).getFullYear()) {
    return tr(`${monthEn} ${String(day)}`, `${String(month)} 月 ${String(day)} 日`);
  }
  const year = String(date.getFullYear());
  return tr(
    `${monthEn} ${String(day)}, ${year}`,
    `${year} 年 ${String(month)} 月 ${String(day)} 日`,
  );
}
