import './filter-chips.css';

export interface FilterChip {
  key: string;
  label: string;
}

interface FilterChipRowProps {
  chips: readonly FilterChip[];
  activeKey: string;
  onSelect: (key: string) => void;
  /** Accessible name for the chip group. */
  label: string;
  /** Makes the whole row inert (announced, never hidden). */
  disabled?: boolean;
}

/**
 * Horizontally scrollable filter chip row for the Library.
 * The active chip is lifted — paper + hairline + e1 shadow on light, a
 * lightness lift on graphite — never a colour fill. Chips keep the 44pt
 * touch height.
 */
export function FilterChipRow({ chips, activeKey, onSelect, label, disabled }: FilterChipRowProps) {
  return (
    <div role="group" aria-label={label} className="tv-chip-row">
      {chips.map((chip) => {
        const active = chip.key === activeKey;
        return (
          <button
            key={chip.key}
            type="button"
            aria-pressed={active}
            aria-disabled={disabled === true || undefined}
            className={active ? 'tv-chip is-active' : 'tv-chip'}
            onClick={() => {
              if (disabled !== true) onSelect(chip.key);
            }}
          >
            {chip.label}
          </button>
        );
      })}
    </div>
  );
}
