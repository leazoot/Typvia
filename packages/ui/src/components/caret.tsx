// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import './caret.css';

interface CaretProps {
  /** Bar height in px; the design pairs it with the neighbouring text size. */
  height: number;
}

/**
 * The 2px accent caret — brand mark, focus origin, empty state and insert
 * confirmation. Always steady: a blinking bar reads as a text cursor, and the
 * product never fakes one. Decorative everywhere: its meaning is always also
 * carried by text, so it is aria-hidden.
 */
export function Caret({ height }: CaretProps) {
  return <span aria-hidden="true" className="tv-caret" style={{ height: `${height}px` }} />;
}
