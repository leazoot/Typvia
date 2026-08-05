import { TYPE_MARKS } from '../tokens';
import './type-mark.css';

interface TypeMarkProps {
  /** One of the eight two-letter codes (TX/CD/CM/PR/TP/SC/AI/LK). */
  code: string;
}

/**
 * Two-letter mono type mark. Only `SC` (Secret) is accent-tinted; the rest
 * are neutral so the list survives greyscale. Screen readers get the full
 * word ("command"), never the letters.
 */
export function TypeMark({ code }: TypeMarkProps) {
  const word = TYPE_MARKS[code];
  return (
    <span
      className={code === 'SC' ? 'tv-type-mark is-secret' : 'tv-type-mark'}
      role="img"
      aria-label={word ?? code}
    >
      <span aria-hidden="true">{code}</span>
    </span>
  );
}
