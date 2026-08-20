// @vitest-environment jsdom
import { fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { installBackspaceGuard } from './backspace-guard';

// fireEvent returns false when preventDefault was called on the event.
describe('installBackspaceGuard', () => {
  let uninstall: (() => void) | null = null;

  afterEach(() => {
    uninstall?.();
    uninstall = null;
    document.body.innerHTML = '';
  });

  const press = (target: Element | Document, key = 'Backspace') =>
    fireEvent.keyDown(target, { key });

  it('swallows Backspace when no editable element has focus', () => {
    uninstall = installBackspaceGuard(window);
    expect(press(document.body)).toBe(false);
  });

  it('swallows Backspace on buttons and non-text inputs', () => {
    uninstall = installBackspaceGuard(window);
    const button = document.createElement('button');
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    document.body.append(button, checkbox);
    expect(press(button)).toBe(false);
    expect(press(checkbox)).toBe(false);
  });

  it('swallows Backspace on read-only and disabled fields', () => {
    uninstall = installBackspaceGuard(window);
    const readonly = document.createElement('input');
    readonly.readOnly = true;
    const disabled = document.createElement('textarea');
    disabled.disabled = true;
    document.body.append(readonly, disabled);
    expect(press(readonly)).toBe(false);
    expect(press(disabled)).toBe(false);
  });

  it('leaves Backspace alone inside editable fields', () => {
    uninstall = installBackspaceGuard(window);
    const input = document.createElement('input');
    const textarea = document.createElement('textarea');
    document.body.append(input, textarea);
    expect(press(input)).toBe(true);
    expect(press(textarea)).toBe(true);
  });

  it('leaves other keys alone everywhere', () => {
    uninstall = installBackspaceGuard(window);
    expect(press(document.body, 'Escape')).toBe(true);
    expect(press(document.body, 'a')).toBe(true);
  });

  it('stops guarding after uninstall', () => {
    const teardown = installBackspaceGuard(window);
    teardown();
    expect(press(document.body)).toBe(true);
  });
});
