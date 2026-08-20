import { describe, expect, it } from 'vitest';
import { changedLineCount, diffLines } from './line-diff';

describe('diffLines', () => {
  it('reports identical bodies as all-same rows', () => {
    const rows = diffLines('a\nb', 'a\nb');
    expect(rows).toEqual([
      { kind: 'same', line: 1, text: 'a' },
      { kind: 'same', line: 2, text: 'b' },
    ]);
    expect(changedLineCount(rows)).toBe(0);
  });

  it('pairs a replaced line as removed-then-added at the change point', () => {
    const rows = diffLines('intro\nlook for bugs\noutro', 'intro\nfocus on correctness\noutro');
    expect(rows).toEqual([
      { kind: 'same', line: 1, text: 'intro' },
      { kind: 'removed', line: 2, text: 'look for bugs' },
      { kind: 'added', line: 2, text: 'focus on correctness' },
      { kind: 'same', line: 3, text: 'outro' },
    ]);
    expect(changedLineCount(rows)).toBe(2);
  });

  it('keeps unchanged runs aligned around insertions', () => {
    const rows = diffLines('one\ntwo', 'one\nadded\ntwo');
    expect(rows).toEqual([
      { kind: 'same', line: 1, text: 'one' },
      { kind: 'added', line: 2, text: 'added' },
      { kind: 'same', line: 3, text: 'two' },
    ]);
  });

  it('handles an empty side without phantom rows', () => {
    expect(diffLines('', '')).toEqual([]);
    expect(diffLines('', 'new line')).toEqual([{ kind: 'added', line: 1, text: 'new line' }]);
    expect(diffLines('old line', '')).toEqual([{ kind: 'removed', line: 1, text: 'old line' }]);
  });

  it('numbers removed lines by the old side and added lines by the new side', () => {
    const rows = diffLines('a\nb\nc', 'a\nc\nd');
    expect(rows).toEqual([
      { kind: 'same', line: 1, text: 'a' },
      { kind: 'removed', line: 2, text: 'b' },
      { kind: 'same', line: 2, text: 'c' },
      { kind: 'added', line: 3, text: 'd' },
    ]);
  });
});
