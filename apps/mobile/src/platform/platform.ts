/**
 * Platform detection for the shared mobile SPA. The WebView user agent is
 * the only signal the page has; the difference layer keys presentation-only
 * choices off it — business rules stay in the Rust core.
 */
export function isAndroidUserAgent(userAgent: string): boolean {
  return userAgent.includes('Android');
}

/** True when the SPA runs inside the Android WebView host. */
export const isAndroid: boolean = isAndroidUserAgent(navigator.userAgent);

/**
 * Device word for the onboarding copy ("Everything stays on this …"). The
 * design file draws the desktop wording ("this Mac") and each platform
 * substitutes its own device word — iOS says iPhone, Android says phone.
 */
export function deviceWordFor(android: boolean): string {
  return android ? 'phone' : 'iPhone';
}

export const deviceWord: string = deviceWordFor(isAndroid);

/**
 * Biometric gate word for the vault copy. The design draws the iOS wording
 * ("Unlock with Face ID"); Android substitutes its own gate name — the
 * emulator/Pixel line calls it fingerprint, and the Keystore key demands a
 * STRONG biometric, which fingerprint is everywhere.
 */
export function biometricWordFor(android: boolean): string {
  return android ? 'Fingerprint' : 'Face ID';
}

/** Chinese reading of the same gate word ("Face ID" stays untranslated). */
export function biometricWordZhFor(android: boolean): string {
  return android ? '指纹' : 'Face ID';
}

export const biometricWord: string = biometricWordFor(isAndroid);
export const biometricWordZh: string = biometricWordZhFor(isAndroid);
