/**
 * MV3 service worker: stateless relay between the popup and the native
 * host, plus the one-shot DOM insertion. Each host call opens a fresh
 * native-messaging port and drops it with the response — no keepalive, no
 * state to lose when the worker is torn down.
 */
import { HOST_NAME } from './messages.js';
import type { HostRequest, HostResponse, InsertReply, WorkerMessage } from './messages.js';

/** One request/response round trip over a throwaway native port. */
function callHost(request: HostRequest): Promise<HostResponse> {
  return new Promise((resolve) => {
    const port = chrome.runtime.connectNative(HOST_NAME);
    let settled = false;
    port.onMessage.addListener((response) => {
      settled = true;
      port.disconnect();
      resolve(response as HostResponse);
    });
    port.onDisconnect.addListener(() => {
      // Host missing / not registered: a stable in-band code, not a throw.
      if (!settled) resolve({ ok: false, code: 'host_unreachable' });
    });
    port.postMessage(request);
  });
}

/**
 * Inserts into the focused editable element of the page. Runs serialized
 * inside the page (isolated world), one-shot: no listeners left behind, no
 * keystroke observation — it only ever writes.
 */
function insertIntoActiveElement(text: string): boolean {
  const element = document.activeElement;
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
    const prototype =
      element instanceof HTMLInputElement
        ? HTMLInputElement.prototype
        : HTMLTextAreaElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(prototype, 'value')?.set;
    if (setter === undefined) return false;
    const start = element.selectionStart ?? element.value.length;
    const end = element.selectionEnd ?? start;
    const next = element.value.slice(0, start) + text + element.value.slice(end);
    // The prototype setter + input event is what framework-controlled
    // inputs (React et al.) accept as a user edit.
    setter.call(element, next);
    try {
      const caret = start + text.length;
      element.setSelectionRange(caret, caret);
    } catch {
      // Some input types (e.g. email) refuse selection APIs; harmless.
    }
    element.dispatchEvent(new Event('input', { bubbles: true }));
    return true;
  }
  if (element instanceof HTMLElement && element.isContentEditable) {
    if (document.execCommand('insertText', false, text)) return true;
    // beforeinput fallback: an editor that handles insertText cancels the
    // event (preventDefault), so "cancelled" means it took the insertion.
    const handled = !element.dispatchEvent(
      new InputEvent('beforeinput', {
        inputType: 'insertText',
        data: text,
        bubbles: true,
        cancelable: true,
      }),
    );
    return handled;
  }
  return false;
}

async function insert(text: string): Promise<InsertReply> {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (tab?.id === undefined) return { inserted: false };
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      func: insertIntoActiveElement,
      args: [text],
    });
    return { inserted: results[0]?.result === true };
  } catch {
    // Restricted page (browser UI, store) — the popup falls back to copy.
    return { inserted: false };
  }
}

chrome.runtime.onMessage.addListener((message: WorkerMessage, _sender, sendResponse) => {
  if (message.kind === 'host') {
    void callHost(message.request).then(sendResponse);
    return true;
  }
  if (message.kind === 'insert') {
    void insert(message.text).then(sendResponse);
    return true;
  }
  return false;
});
