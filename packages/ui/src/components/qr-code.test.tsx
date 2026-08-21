// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { QrCode } from './qr-code';

afterEach(cleanup);

describe('QrCode', () => {
  it('announces the label and hides the modules from the accessibility tree', () => {
    const { container } = render(<QrCode value="typvia://pair/abc" label="Pairing code" />);
    const svg = screen.getByRole('img', { name: 'Pairing code' });
    expect(svg.tagName.toLowerCase()).toBe('svg');
    expect(svg.getAttribute('width')).toBe('132');
    expect(container.querySelectorAll('path')).toHaveLength(1);
  });

  it('sizes the viewBox to the symbol plus a four-module quiet zone', () => {
    const { container } = render(<QrCode value="a" size={264} label="Pairing code" />);
    const svg = container.querySelector('svg');
    // Version 1 is 21 modules wide, so 21 + 4 + 4 = 29 viewBox units.
    expect(svg?.getAttribute('viewBox')).toBe('0 0 29 29');
    expect(svg?.getAttribute('width')).toBe('264');
    expect(svg?.getAttribute('shape-rendering')).toBe('crispEdges');
  });

  it('paints only from the paper and ink tokens', () => {
    const { container } = render(<QrCode value="a" label="Pairing code" />);
    expect(container.querySelector('rect')?.getAttribute('fill')).toBe('var(--color-paper)');
    expect(container.querySelector('path')?.getAttribute('fill')).toBe('var(--color-ink)');
  });
});
