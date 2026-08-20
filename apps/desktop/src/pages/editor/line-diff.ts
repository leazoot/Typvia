/**
 * Line-level diff for the History view. LCS-based so
 * unchanged runs stay aligned; snippet bodies are small, so the quadratic
 * table is fine — with a guard that falls back to whole-body replace when a
 * pathological input would make the table large.
 */

export interface DiffRow {
  kind: 'same' | 'removed' | 'added';
  /** 1-based line number in the side this row belongs to (old for removed, new otherwise). */
  line: number;
  text: string;
}

/** Above this many lines per side, alignment is not worth the quadratic table. */
const MAX_LCS_LINES = 2000;

function splitLines(text: string): string[] {
  return text === '' ? [] : text.split('\n');
}

/**
 * Diffs two bodies into display rows: removed lines of `before` (in order)
 * interleaved with added lines of `after`, unchanged lines once.
 */
export function diffLines(before: string, after: string): DiffRow[] {
  const a = splitLines(before);
  const b = splitLines(after);
  if (a.length > MAX_LCS_LINES || b.length > MAX_LCS_LINES) {
    return [
      ...a.map((text, i): DiffRow => ({ kind: 'removed', line: i + 1, text })),
      ...b.map((text, i): DiffRow => ({ kind: 'added', line: i + 1, text })),
    ];
  }

  // lcs[i][j] = LCS length of a[i..] and b[j..].
  const lcs: number[][] = Array.from({ length: a.length + 1 }, () =>
    new Array<number>(b.length + 1).fill(0),
  );
  for (let i = a.length - 1; i >= 0; i--) {
    const row = lcs[i];
    const next = lcs[i + 1];
    if (row === undefined || next === undefined) continue;
    for (let j = b.length - 1; j >= 0; j--) {
      row[j] = a[i] === b[j] ? (next[j + 1] ?? 0) + 1 : Math.max(next[j] ?? 0, row[j + 1] ?? 0);
    }
  }

  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      rows.push({ kind: 'same', line: j + 1, text: b[j] ?? '' });
      i++;
      j++;
    } else if ((lcs[i + 1]?.[j] ?? 0) >= (lcs[i]?.[j + 1] ?? 0)) {
      rows.push({ kind: 'removed', line: i + 1, text: a[i] ?? '' });
      i++;
    } else {
      rows.push({ kind: 'added', line: j + 1, text: b[j] ?? '' });
      j++;
    }
  }
  for (; i < a.length; i++) rows.push({ kind: 'removed', line: i + 1, text: a[i] ?? '' });
  for (; j < b.length; j++) rows.push({ kind: 'added', line: j + 1, text: b[j] ?? '' });
  return rows;
}

/** "N lines changed" summary for the diff header. */
export function changedLineCount(rows: DiffRow[]): number {
  return rows.filter((row) => row.kind !== 'same').length;
}
