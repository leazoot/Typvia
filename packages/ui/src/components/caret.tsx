import './caret.css';

interface CaretProps {
  /** Bar height in px; the design pairs it with the neighbouring text size. */
  height: number;
  /** Blinking is the brand's resting state; steady for static uses. */
  blinking?: boolean;
}

/**
 * The 2px accent caret — brand mark, focus origin, empty state and insert
 * confirmation. Decorative everywhere: its meaning is always also in text,
 * so it is aria-hidden (design handoff §Screen readers).
 */
export function Caret({ height, blinking = true }: CaretProps) {
  return (
    <span
      aria-hidden="true"
      className={blinking ? 'tv-caret tv-caret-blink' : 'tv-caret'}
      style={{ height: `${height}px` }}
    />
  );
}
