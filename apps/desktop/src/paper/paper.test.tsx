// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Choice } from './choice';
import { Mascot } from './mascot';

afterEach(cleanup);

function Orders({ onChange }: { onChange: (value: string) => void }) {
  const [value, setValue] = useState<'recent' | 'added' | 'used'>('recent');
  return (
    <Choice
      label="Order"
      value={value}
      onChange={(next) => {
        setValue(next);
        onChange(next);
      }}
      options={[
        { value: 'recent', label: 'Recently used' },
        { value: 'added', label: 'Recently added' },
        { value: 'used', label: 'Most used', trailing: '41' },
      ]}
    />
  );
}

describe('Choice', () => {
  it('is one tab stop: only the chosen option is in the tab order', () => {
    render(<Orders onChange={vi.fn()} />);
    const radios = screen.getAllByRole('radio');
    expect(radios.map((radio) => radio.getAttribute('tabindex'))).toEqual(['0', '-1', '-1']);
    expect(screen.getByRole('radio', { name: 'Recently used' }).getAttribute('aria-checked')).toBe(
      'true',
    );
  });

  it('chooses on click with no confirmation step', () => {
    const onChange = vi.fn();
    render(<Orders onChange={onChange} />);
    fireEvent.click(screen.getByRole('radio', { name: /Most used/ }));
    expect(onChange).toHaveBeenCalledWith('used');
    expect(screen.getByRole('radio', { name: /Most used/ }).getAttribute('aria-checked')).toBe(
      'true',
    );
  });

  it('moves, chooses and follows focus with the arrow keys, wrapping at the ends', () => {
    const onChange = vi.fn();
    render(<Orders onChange={onChange} />);
    const first = screen.getByRole('radio', { name: 'Recently used' });
    fireEvent.keyDown(first, { key: 'ArrowRight' });
    expect(onChange).toHaveBeenLastCalledWith('added');
    expect(document.activeElement).toBe(screen.getByRole('radio', { name: 'Recently added' }));
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'ArrowLeft' });
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'ArrowUp' });
    expect(onChange).toHaveBeenLastCalledWith('used');
  });
});

describe('Mascot', () => {
  it('stays out of the accessibility tree and names its state for styling', () => {
    const { container } = render(<Mascot state="thinking" size={46} />);
    const svg = container.querySelector('svg');
    expect(svg?.getAttribute('aria-hidden')).toBe('true');
    expect(svg?.getAttribute('data-state')).toBe('thinking');
    expect(svg?.getAttribute('width')).toBe('46');
  });

  it('references a gradient id an SVG url() can actually resolve', () => {
    const { container } = render(<Mascot size={26} />);
    const id = container.querySelector('linearGradient')?.getAttribute('id') ?? '';
    expect(id).toMatch(/^[a-zA-Z0-9-]+$/);
    expect(container.querySelector('ellipse')?.getAttribute('style')).toContain(`#${id}`);
  });

  it('trades the caret antenna for a shackle when locked', () => {
    const { container } = render(<Mascot state="locked" size={26} />);
    expect(container.querySelector('rect[x="20.9"]')).toBeNull();
    expect(container.querySelector('path[d="M15.5 8a6.5 6.5 0 0113 0"]')).not.toBeNull();
  });
});
