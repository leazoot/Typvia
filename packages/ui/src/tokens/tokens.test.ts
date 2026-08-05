// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import upstream from '../../../../design/design_handoff_typvia/tokens.json';
import {
  applyTheme,
  BREAKPOINTS,
  CN_LEADING_DELTA,
  cnLeading,
  cssVariables,
  designTokens,
  TYPE_MARKS,
} from './index';

// WCAG 2.x relative luminance / contrast ratio, for the contrast contract.
function contrastRatio(hexA: string, hexB: string): number {
  const lum = (hex: string): number => {
    const channels = [1, 3, 5].map((offset) => {
      const channel = parseInt(hex.slice(offset, offset + 2), 16) / 255;
      return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
    }) as [number, number, number];
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
  };
  const [bright, dim] = [lum(hexA), lum(hexB)].sort((a, b) => b - a) as [number, number];
  return (bright + 0.05) / (dim + 0.05);
}

describe('token source parity', () => {
  it('vendored tokens.json matches the design handoff value for value', () => {
    expect(designTokens).toEqual(upstream);
  });
});

describe('cssVariables', () => {
  const light = cssVariables('light');
  const dark = cssVariables('dark');

  it('exposes every non-annotation color token of each theme', () => {
    for (const [theme, vars] of [
      ['light', light],
      ['dark', dark],
    ] as const) {
      for (const key of Object.keys(designTokens.color[theme])) {
        if (key.startsWith('$')) continue;
        const name = `--color-${key.replace(/[A-Z]/g, (u) => `-${u.toLowerCase()}`)}`;
        expect(vars[name], `${theme} ${name}`).toBeDefined();
      }
    }
  });

  it('maps representative values with correct units', () => {
    expect(light['--color-paper']).toBe('#FCFCFA');
    expect(light['--color-hairline-hover']).toBe('#D8D8D1');
    expect(dark['--color-paper']).toBe('#191A1A');
    expect(dark['--color-lifted']).toBe('#242525');
    expect(light['--radius-row']).toBe('9px');
    expect(light['--space-11']).toBe('11px');
    expect(light['--space-row-height-desktop']).toBe('52px');
    expect(light['--duration-press']).toBe('80ms');
    expect(light['--duration-ambient-loop']).toBe('2600ms');
    expect(light['--curve-standard']).toBe('cubic-bezier(.2, .7, .2, 1)');
    expect(light['--elevation-e1-row']).toBe(designTokens.elevation.e1Row);
    expect(light['--font-mono']).toBe('"IBM Plex Mono", monospace');
    expect(light['--type-editor-body-size']).toBe('14.5px');
    expect(light['--type-editor-body-leading']).toBe('2.05');
    expect(light['--scrim-modal']).toBe('rgba(27,28,29,.28)');
    expect(light['--focus-ring']).toBe(
      '0 0 0 2px var(--focus-ring-surface, var(--color-paper)), 0 0 0 3.5px var(--color-accent)',
    );
  });

  it('keeps theme-specific keys theme-specific (no invented fallbacks)', () => {
    expect(dark['--color-tray']).toBeUndefined();
    expect(dark['--color-hairline-hover']).toBeUndefined();
    expect(light['--color-lifted']).toBeUndefined();
  });
});

describe('contrast contract (design handoff §Accessibility)', () => {
  const light = designTokens.color.light;
  const dark = designTokens.color.dark;

  it('all three light text tiers and the accent clear 4.5:1 on paper', () => {
    for (const hex of [light.ink, light.secondary, light.meta, light.accent]) {
      expect(contrastRatio(hex, light.paper)).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('graphite uses its own accent #7FA3C4; the light accent fails there', () => {
    expect(dark.accent).toBe('#7FA3C4');
    expect(contrastRatio(dark.accent, dark.paper)).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(light.accent, dark.paper)).toBeLessThan(4.5);
  });

  it('all three graphite text tiers clear 4.5:1 on graphite paper', () => {
    for (const hex of [dark.ink, dark.secondary, dark.meta]) {
      expect(contrastRatio(hex, dark.paper)).toBeGreaterThanOrEqual(4.5);
    }
  });
});

describe('applyTheme', () => {
  it('switches themes without leaving stale variables and stamps data-theme', () => {
    const root = document.createElement('div');
    applyTheme(root, 'light');
    expect(root.style.getPropertyValue('--color-tray')).toBe('#F6F6F2');
    expect(root.dataset['theme']).toBe('light');

    applyTheme(root, 'dark');
    expect(root.style.getPropertyValue('--color-paper')).toBe('#191A1A');
    expect(root.style.getPropertyValue('--color-lifted')).toBe('#242525');
    expect(root.style.getPropertyValue('--color-tray')).toBe('');
    expect(root.dataset['theme']).toBe('dark');
  });
});

describe('supporting constants', () => {
  it('applies the CN line-height rule (+0.15 over the EN value)', () => {
    expect(CN_LEADING_DELTA).toBe(0.15);
    expect(cnLeading(designTokens.type.body.leading)).toBeCloseTo(1.85);
  });

  it('exposes the eight type marks and the breakpoints', () => {
    expect(Object.keys(TYPE_MARKS)).toEqual(['TX', 'CD', 'CM', 'PR', 'TP', 'SC', 'AI', 'LK']);
    expect(BREAKPOINTS).toEqual({
      reference: 1360,
      drawerPreview: 1160,
      railCollapse: 840,
      landscapeTwoPane: 700,
    });
  });
});
