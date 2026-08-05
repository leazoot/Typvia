/**
 * Design tokens (Quiet Path). `tokens.json` here is a byte-for-byte vendor
 * copy of design/design_handoff_typvia/tokens.json — the design handoff owns
 * the values, and a test fails on any drift between the two files.
 *
 * CSS custom properties are generated from the JSON at runtime, so no value
 * is ever transcribed by hand. Theme-specific color keys follow the handoff
 * exactly: keys one theme lacks (e.g. `tray` on graphite, `lifted` on paper)
 * are intentionally absent there — components express those differences per
 * the design (selection is a shadow lift on paper, a two-step lightness lift
 * on graphite), never via an invented fallback color.
 */
import raw from './tokens.json';

export type ThemeName = 'light' | 'dark';

/** The full token tree as delivered (annotation keys start with `$`). */
export const designTokens = raw;

/** Two-letter snippet type marks; only `SC` is accent-tinted. */
export const TYPE_MARKS: Readonly<Record<string, string>> = Object.fromEntries(
  Object.entries(raw.typeMarks).filter(([key]) => !isAnnotation(key)),
);

/** Layout breakpoints in px. CSS variables cannot drive media queries, so these are TS constants. */
export const BREAKPOINTS = {
  reference: raw.breakpoint.reference,
  drawerPreview: raw.breakpoint.drawerPreview,
  railCollapse: raw.breakpoint.railCollapse,
  landscapeTwoPane: raw.breakpoint.landscapeTwoPane,
} as const;

/** CN line-height is always +0.15 over the EN value at the same size. */
export const CN_LEADING_DELTA = 0.15;

export function cnLeading(enLeading: number): number {
  return enLeading + CN_LEADING_DELTA;
}

function isAnnotation(key: string): boolean {
  return key.startsWith('$');
}

function kebab(key: string): string {
  return key.replace(/[A-Z]/g, (upper) => `-${upper.toLowerCase()}`);
}

interface TypeRole {
  size: number;
  weight: number;
  leading: number;
  tracking?: string;
  family?: string;
  case?: string;
}

/**
 * All design tokens of one theme as CSS custom properties.
 *
 * Naming: `--color-*`, `--radius-*` (px), `--space-*` (px; scale entries are
 * named by their value, e.g. `--space-11`), `--elevation-*`, `--curve-*`,
 * `--duration-*` (ms), `--font-*`, `--type-<role>-{size,weight,leading,tracking}`,
 * `--scrim-modal`, `--focus-ring` and `--cn-leading-delta`.
 *
 * `--focus-ring` resolves its inner gap against `--focus-ring-surface`
 * (defaulting to paper); a component on a sunken tray overrides that variable.
 */
export function cssVariables(theme: ThemeName): Record<string, string> {
  const vars: Record<string, string> = {};

  // Widening casts only: the JSON literal types narrow to these shapes.
  const colors = raw.color[theme] as Record<string, string>;
  for (const [key, value] of Object.entries(colors)) {
    if (!isAnnotation(key)) vars[`--color-${kebab(key)}`] = value;
  }

  for (const [key, value] of Object.entries(raw.font)) {
    if (!isAnnotation(key)) vars[`--font-${kebab(key)}`] = value;
  }

  const typeRoles = raw.type as Record<string, TypeRole>;
  for (const [role, spec] of Object.entries(typeRoles)) {
    const prefix = `--type-${kebab(role)}`;
    vars[`${prefix}-size`] = `${spec.size}px`;
    vars[`${prefix}-weight`] = `${spec.weight}`;
    vars[`${prefix}-leading`] = `${spec.leading}`;
    if (spec.tracking !== undefined) vars[`${prefix}-tracking`] = spec.tracking;
  }

  const radii = raw.radius as Record<string, number | string>;
  for (const [key, value] of Object.entries(radii)) {
    if (!isAnnotation(key)) vars[`--radius-${kebab(key)}`] = `${value}px`;
  }

  const { scale, ...namedSpace } = raw.space;
  for (const step of scale) vars[`--space-${step}`] = `${step}px`;
  for (const [key, value] of Object.entries(namedSpace)) {
    vars[`--space-${kebab(key)}`] = `${value}px`;
  }

  const elevations = raw.elevation as Record<string, string>;
  for (const [key, value] of Object.entries(elevations)) {
    if (!isAnnotation(key)) vars[`--elevation-${kebab(key)}`] = value;
  }

  for (const [key, value] of Object.entries(raw.motion.curve)) {
    vars[`--curve-${kebab(key)}`] = value;
  }
  for (const [key, value] of Object.entries(raw.motion.duration)) {
    vars[`--duration-${kebab(key)}`] = `${value}ms`;
  }

  vars['--scrim-modal'] = raw.scrim.modal;
  // Ink surfaces (toast, primary button) exist in both themes; on them the
  // caret always uses the graphite accent ("carries the caret in light
  // accent", Phase 6 Toast). Generated, not hand-copied.
  vars['--color-accent-on-ink'] = raw.color.dark.accent;
  vars['--focus-ring'] = raw.focus.ring
    .replace('<surface>', 'var(--focus-ring-surface, var(--color-paper))')
    .replace('<accent>', 'var(--color-accent)');
  vars['--cn-leading-delta'] = `${CN_LEADING_DELTA}`;

  return vars;
}

/**
 * Applies one theme's variables to `root` (normally `document.documentElement`)
 * and stamps `data-theme`. Variables the other theme defines but this one
 * lacks are removed, so switching never leaves stale values behind.
 */
export function applyTheme(root: HTMLElement, theme: ThemeName): void {
  const next = cssVariables(theme);
  const previous = cssVariables(theme === 'light' ? 'dark' : 'light');
  for (const name of Object.keys(previous)) {
    if (!(name in next)) root.style.removeProperty(name);
  }
  for (const [name, value] of Object.entries(next)) {
    root.style.setProperty(name, value);
  }
  root.dataset['theme'] = theme;
}

/**
 * Follows the OS appearance (light ↔ dark) until the returned cleanup is
 * called. An in-app appearance setting can later replace this by calling
 * `applyTheme` directly.
 */
export function followSystemTheme(root: HTMLElement): () => void {
  const query = window.matchMedia('(prefers-color-scheme: dark)');
  const apply = () => applyTheme(root, query.matches ? 'dark' : 'light');
  apply();
  query.addEventListener('change', apply);
  return () => query.removeEventListener('change', apply);
}
