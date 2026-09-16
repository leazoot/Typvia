// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import { useEffect, useRef } from 'react';

/**
 * Emptying the trash is the one thing on the desktop that asks before it
 * acts: it is the only delete nothing can bring back. The question carries
 * the real count, and focus starts on the answer that keeps everything.
 */
export function EmptyConfirm({
  count,
  onConfirm,
  onCancel,
}: {
  count: number;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const tr = useTr();
  const keep = useRef<HTMLButtonElement>(null);
  const clear = useRef<HTMLButtonElement>(null);
  useEffect(() => keep.current?.focus(), []);
  const n = String(count);

  return (
    <>
      <div aria-hidden="true" className="tvt-scrim" onClick={onCancel} />
      <div
        className="tvt-confirm"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tvt-confirm-title"
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            onCancel();
          } else if (event.key === 'Tab') {
            // Two answers, and focus stays between them.
            event.preventDefault();
            (document.activeElement === keep.current ? clear : keep).current?.focus();
          }
        }}
      >
        <p id="tvt-confirm-title" className="tpi-note-title">
          {tr(`Delete the ${n} in the trash for good?`, `把回收站里的 ${n} 条彻底删掉?`)}
        </p>
        <p className="tpi-note-body">
          {tr(
            'They will not come back, and their triggers and use counts go with them.',
            '删掉就找不回来了,触发词和用过的次数也一起没了。',
          )}
        </p>
        <div className="tvt-confirm-actions">
          <button ref={clear} type="button" className="tvt-destroy" onClick={onConfirm}>
            {tr(`Delete ${n} for good`, `彻底删掉 ${n} 条`)}
          </button>
          <button ref={keep} type="button" className="tpi-note-no" onClick={onCancel}>
            {tr('Not now', '先别')}
          </button>
        </div>
      </div>
    </>
  );
}
