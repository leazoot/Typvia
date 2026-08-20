import './caret.css';

interface CaretProps {
  /** Bar height in px; the design pairs it with the neighbouring text size. */
  height: number;
}

/**
 * The 2px accent caret — brand mark, focus origin, empty state and insert
 * confirmation. Always steady: a blinking bar reads as a text cursor, and the
 * product never fakes one. Decorative everywhere: its meaning is always also
 * carried by text, so it is aria-hidden.
 */
export function Caret({ height }: CaretProps) {
  return <span aria-hidden="true" className="tv-caret" style={{ height: `${height}px` }} />;
}
