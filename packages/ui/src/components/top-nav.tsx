import { Caret } from './caret';
import './top-nav.css';

export interface TopNavItem {
  key: string;
  label: string;
}

interface TopNavProps {
  items: readonly TopNavItem[];
  activeKey: string;
  onNavigate: (key: string) => void;
}

/**
 * Desktop top navigation: text plus caret, no icons, 46px tall (Phase 6
 * "Top nav"). Active = ink + blinking caret; rest = secondary. Never an
 * underline, never a pill. Router wiring stays in the app shell.
 */
export function TopNav({ items, activeKey, onNavigate }: TopNavProps) {
  return (
    <nav aria-label="Main" className="tv-top-nav">
      <ul>
        {items.map((item) => {
          const active = item.key === activeKey;
          return (
            <li key={item.key}>
              <button
                type="button"
                aria-current={active ? 'page' : undefined}
                className={active ? 'tv-top-nav-item is-active' : 'tv-top-nav-item'}
                onClick={() => onNavigate(item.key)}
              >
                {item.label}
                {active && <Caret height={11} />}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
