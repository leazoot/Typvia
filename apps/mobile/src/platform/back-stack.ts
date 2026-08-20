/**
 * Android system-back semantics for the shared SPA. The host activity
 * (MainActivity.kt) asks the page before running the default back — which
 * would background the app — through the `__TYPVIA_ANDROID_BACK__` hook:
 * layers register a handler while a dismissable surface is open (editor
 * page, strip action popover) and the newest open surface consumes the
 * gesture. With nothing registered the default stands, so back at a tab
 * root leaves the app.
 */
type BackHandler = () => void;

const stack: BackHandler[] = [];

/** Registers `handler` above the current ones; returns its release. */
export function pushBackHandler(handler: BackHandler): () => void {
  stack.push(handler);
  return () => {
    const index = stack.lastIndexOf(handler);
    if (index !== -1) stack.splice(index, 1);
  };
}

declare global {
  interface Window {
    /** Host back hook: true = the SPA consumed the gesture. */
    __TYPVIA_ANDROID_BACK__?: (() => boolean) | undefined;
  }
}

/** Installs the host-visible hook; called once at boot on Android. */
export function installAndroidBackBridge(): void {
  window.__TYPVIA_ANDROID_BACK__ = () => {
    const top = stack.at(-1);
    if (top === undefined) return false;
    top();
    return true;
  };
}
