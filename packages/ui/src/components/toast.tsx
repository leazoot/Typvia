// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { Caret } from './caret';
import './toast.css';

interface ToastProps {
  /** States the outcome and names the destination, e.g. "Inserted into VS Code". */
  message: string;
  /**
   * Insert toasts are assertive — the user just changed another app's
   * content. Everything else is polite.
   */
  assertive?: boolean;
}

/**
 * Toast: an ink surface carrying the caret. Enter 190ms translateY 8→0 +
 * opacity; the 1600ms life and exit are the host's concern (toastLife /
 * overlayExit duration tokens).
 */
export function Toast({ message, assertive = false }: ToastProps) {
  return (
    <output className="tv-toast" role="status" aria-live={assertive ? 'assertive' : 'polite'}>
      <Caret height={13} />
      {message}
    </output>
  );
}
