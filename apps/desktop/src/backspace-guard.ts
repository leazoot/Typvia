/** Input types where Backspace never edits text. */
const NON_TEXT_INPUT_TYPES = new Set([
  'button',
  'checkbox',
  'color',
  'file',
  'hidden',
  'image',
  'radio',
  'range',
  'reset',
  'submit',
]);

function editsText(target: EventTarget | null): boolean {
  if (target instanceof HTMLTextAreaElement) return !target.disabled && !target.readOnly;
  if (target instanceof HTMLInputElement) {
    return !target.disabled && !target.readOnly && !NON_TEXT_INPUT_TYPES.has(target.type);
  }
  return target instanceof HTMLElement && target.isContentEditable;
}

/**
 * The macOS webview treats Backspace outside an editable field as
 * history-back, and BrowserRouter shares that history — so a stray
 * Backspace pops the current route. Swallow exactly those presses;
 * deletion inside editable fields is untouched.
 */
export function installBackspaceGuard(win: Window): () => void {
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Backspace' && !editsText(event.target)) event.preventDefault();
  };
  win.addEventListener('keydown', onKeyDown, true);
  return () => {
    win.removeEventListener('keydown', onKeyDown, true);
  };
}
