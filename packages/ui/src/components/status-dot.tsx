// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import './status-dot.css';

export type StatusKind = 'success' | 'warning' | 'error' | 'secure';

interface StatusDotProps {
  kind: StatusKind;
  /** Colour is never load-bearing: every status is paired with a word. */
  label: string;
}

/**
 * Status indicator: a 7px round dot for state, a 7px square outline for
 * security — the one state where a misread has consequences gets a
 * different shape, not just a different colour.
 */
export function StatusDot({ kind, label }: StatusDotProps) {
  return (
    <span className={`tv-status tv-status-${kind}`}>
      <span aria-hidden="true" className="tv-status-dot" />
      {label}
    </span>
  );
}
