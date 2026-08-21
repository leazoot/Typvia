// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useMemo } from 'react';
import { qrMatrix } from './qr-matrix';

/** Light margin required around a symbol for scanners to lock on (ISO/IEC 18004 §6.3.8). */
const QUIET_ZONE = 4;

interface QrCodeProps {
  /** Payload to encode; UTF-8 length must stay within `QR_MAX_BYTES`. */
  value: string;
  /** Rendered edge length in px (default 132, the design's pairing frame). */
  size?: number;
  /** Spoken description of what the code does — the symbol itself reads as nothing. */
  label: string;
}

/** One `M x y h1 v1 h-1 z` sub-path per dark module, in symbol coordinates. */
function modulePath(matrix: readonly (readonly boolean[])[]): string {
  const parts: string[] = [];
  for (let y = 0; y < matrix.length; y++) {
    const row = matrix[y];
    if (row === undefined) {
      continue;
    }
    for (let x = 0; x < row.length; x++) {
      if (row[x] === true) {
        parts.push(`M${x + QUIET_ZONE} ${y + QUIET_ZONE}h1v1h-1z`);
      }
    }
  }
  return parts.join('');
}

/**
 * QR symbol for device pairing, drawn as inline SVG in paper/ink so it inverts
 * with the theme. One module equals one viewBox unit, so the symbol stays
 * pixel-crisp at any `size`. Throws if `value` exceeds the encoder capacity.
 */
export function QrCode({ value, size = 132, label }: QrCodeProps) {
  const { path, extent } = useMemo(() => {
    const matrix = qrMatrix(value);
    return { path: modulePath(matrix), extent: matrix.length + QUIET_ZONE * 2 };
  }, [value]);

  return (
    <svg
      className="tv-qr-code"
      width={size}
      height={size}
      viewBox={`0 0 ${extent} ${extent}`}
      shapeRendering="crispEdges"
      role="img"
      aria-label={label}
    >
      <rect width={extent} height={extent} fill="var(--color-paper)" />
      <path d={path} fill="var(--color-ink)" />
    </svg>
  );
}
