import { describe, expect, it } from 'vitest';
import { QR_MAX_BYTES, qrMatrix } from './qr-matrix';

// The encoder is verified structurally: a Reed-Solomon syndrome check proves
// the codewords are valid RS codewords, and a decoder written from the spec
// (log/exp arithmetic and row/column mask conditions, i.e. deliberately not the
// encoder's formulation) proves the symbol reads back as the input.

function num(value: number | undefined): number {
  if (value === undefined) {
    throw new Error('test index out of range');
  }
  return value;
}

function flag(value: boolean | undefined): boolean {
  if (value === undefined) {
    throw new Error('test index out of range');
  }
  return value;
}

/** GF(256) log/exp tables over x^8 + x^4 + x^3 + x^2 + 1. */
const GF_EXP: number[] = new Array<number>(255).fill(0);
const GF_LOG: number[] = new Array<number>(256).fill(0);
for (let i = 0, value = 1; i < 255; i++) {
  GF_EXP[i] = value;
  GF_LOG[value] = i;
  value <<= 1;
  if (value >= 256) {
    value ^= 0x11d;
  }
}

function gfPow(exponent: number): number {
  return num(GF_EXP[exponent % 255]);
}

function gfMul(a: number, b: number): number {
  if (a === 0 || b === 0) {
    return 0;
  }
  return num(GF_EXP[(num(GF_LOG[a]) + num(GF_LOG[b])) % 255]);
}

/** Horner evaluation of the codeword polynomial, highest power first. */
function evaluatePolynomial(codewords: readonly number[], x: number): number {
  let value = 0;
  for (const codeword of codewords) {
    value = gfMul(value, x) ^ codeword;
  }
  return value;
}

// Level M block structure, transcribed from ISO/IEC 18004 table 9. The
// capacity-boundary test cross-checks these against published byte capacities.
const EC_PER_BLOCK: readonly number[] = [
  10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28,
  28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
];
const BLOCK_COUNT: readonly number[] = [
  1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26,
  28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
];

type Matrix = readonly (readonly boolean[])[];

function rowAt(matrix: Matrix, row: number): readonly boolean[] {
  const line = matrix[row];
  if (line === undefined) {
    throw new Error('test row out of range');
  }
  return line;
}

function moduleAt(matrix: Matrix, row: number, column: number): boolean {
  return flag(rowAt(matrix, row)[column]);
}

function blockAt(blocks: readonly number[][], index: number): number[] {
  const block = blocks[index];
  if (block === undefined) {
    throw new Error('test block out of range');
  }
  return block;
}

function versionOf(matrix: Matrix): number {
  return (matrix.length - 17) / 4;
}

function alignmentCenters(version: number): number[] {
  if (version === 1) {
    return [];
  }
  const count = Math.floor(version / 7) + 2;
  const size = version * 4 + 17;
  const step = version === 32 ? 26 : Math.ceil((version * 4 + 4) / (count * 2 - 2)) * 2;
  const centers = [6];
  for (let pos = size - 7; centers.length < count; pos -= step) {
    centers.splice(1, 0, pos);
  }
  return centers;
}

/** Every module reserved by finders, separators, timing, alignment, format and version areas. */
function isFunctionModule(version: number, row: number, column: number): boolean {
  const size = version * 4 + 17;
  const inTopLeft = row <= 8 && column <= 8;
  const inTopRight = row <= 8 && column >= size - 8;
  const inBottomLeft = row >= size - 8 && column <= 8;
  if (inTopLeft || inTopRight || inBottomLeft) {
    return true;
  }
  if (row === 6 || column === 6) {
    return true;
  }
  if (version >= 7) {
    if (row <= 5 && column >= size - 11 && column <= size - 9) {
      return true;
    }
    if (column <= 5 && row >= size - 11 && row <= size - 9) {
      return true;
    }
  }
  const centers = alignmentCenters(version);
  const last = centers.length - 1;
  for (let i = 0; i <= last; i++) {
    for (let j = 0; j <= last; j++) {
      const skipped = (i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0);
      if (skipped) {
        continue;
      }
      const centerRow = num(centers[i]);
      const centerColumn = num(centers[j]);
      if (Math.abs(row - centerRow) <= 2 && Math.abs(column - centerColumn) <= 2) {
        return true;
      }
    }
  }
  return false;
}

/** Data mask conditions expressed over (row, column) as in ISO/IEC 18004 table 10. */
function maskBit(mask: number, row: number, column: number): boolean {
  switch (mask) {
    case 0:
      return (row + column) % 2 === 0;
    case 1:
      return row % 2 === 0;
    case 2:
      return column % 3 === 0;
    case 3:
      return (row + column) % 3 === 0;
    case 4:
      return (Math.floor(row / 2) + Math.floor(column / 3)) % 2 === 0;
    case 5:
      return ((row * column) % 2) + ((row * column) % 3) === 0;
    case 6:
      return (((row * column) % 2) + ((row * column) % 3)) % 2 === 0;
    default:
      return (((row + column) % 2) + ((row * column) % 3)) % 2 === 0;
  }
}

/** Binary polynomial remainder — zero proves `value` is a codeword of `generator`. */
function polyMod(value: number, generator: number): number {
  const generatorBits = 32 - Math.clz32(generator);
  let remainder = value;
  for (
    let bits = 32 - Math.clz32(remainder);
    bits >= generatorBits;
    bits = 32 - Math.clz32(remainder)
  ) {
    remainder ^= generator << (bits - generatorBits);
  }
  return remainder;
}

interface FormatInfo {
  ecLevel: number;
  mask: number;
}

function readFormatCodeword(matrix: Matrix, copy: 0 | 1): number {
  const size = matrix.length;
  let raw = 0;
  for (let i = 0; i < 15; i++) {
    let row: number;
    let column: number;
    if (copy === 0) {
      if (i < 6) {
        [row, column] = [i, 8];
      } else if (i === 6) {
        [row, column] = [7, 8];
      } else if (i === 7) {
        [row, column] = [8, 8];
      } else if (i === 8) {
        [row, column] = [8, 7];
      } else {
        [row, column] = [8, 14 - i];
      }
    } else if (i < 8) {
      [row, column] = [8, size - 1 - i];
    } else {
      [row, column] = [size - 15 + i, 8];
    }
    if (moduleAt(matrix, row, column)) {
      raw |= 1 << i;
    }
  }
  return raw;
}

function readFormat(matrix: Matrix, copy: 0 | 1): FormatInfo {
  // Bits 14..10 carry the five data bits; bits 9..0 are the BCH remainder.
  const data = ((readFormatCodeword(matrix, copy) ^ 0x5412) >> 10) & 0b11111;
  return { ecLevel: (data >> 3) & 0b11, mask: data & 0b111 };
}

/** The 18-bit version information block, present from version 7 upwards. */
function readVersionCodeword(matrix: Matrix, mirrored: boolean): number {
  const size = matrix.length;
  let bits = 0;
  for (let i = 0; i < 18; i++) {
    const near = Math.floor(i / 3);
    const far = size - 11 + (i % 3);
    const [row, column] = mirrored ? [far, near] : [near, far];
    if (moduleAt(matrix, row, column)) {
      bits |= 1 << i;
    }
  }
  return bits;
}

/** Walks the zigzag, un-applies the mask and rebuilds the interleaved codewords. */
function readCodewords(matrix: Matrix, mask: number): number[] {
  const version = versionOf(matrix);
  const size = matrix.length;
  const bits: boolean[] = [];
  for (let column = size - 1; column > 0; column -= 2) {
    if (column === 6) {
      column = 5;
    }
    const upward = ((column + 1) & 2) === 0;
    for (let step = 0; step < size; step++) {
      const row = upward ? size - 1 - step : step;
      for (const x of [column, column - 1]) {
        if (isFunctionModule(version, row, x)) {
          continue;
        }
        bits.push(moduleAt(matrix, row, x) !== maskBit(mask, row, x));
      }
    }
  }

  const codewords: number[] = [];
  for (let i = 0; i + 8 <= bits.length; i += 8) {
    let codeword = 0;
    for (let j = 0; j < 8; j++) {
      codeword = (codeword << 1) | (flag(bits[i + j]) ? 1 : 0);
    }
    codewords.push(codeword);
  }
  return codewords;
}

interface Blocks {
  data: number[][];
  ec: number[][];
}

function deinterleave(codewords: readonly number[], version: number): Blocks {
  const blockCount = num(BLOCK_COUNT[version - 1]);
  const ecPerBlock = num(EC_PER_BLOCK[version - 1]);
  const dataCount = codewords.length - ecPerBlock * blockCount;
  const shortLength = Math.floor(dataCount / blockCount);
  const shortBlocks = blockCount - (dataCount % blockCount);
  const lengths = Array.from({ length: blockCount }, (_unused, i) =>
    i < shortBlocks ? shortLength : shortLength + 1,
  );

  const data: number[][] = lengths.map(() => []);
  const ec: number[][] = lengths.map(() => []);
  let cursor = 0;
  for (let i = 0; i <= shortLength; i++) {
    for (let b = 0; b < blockCount; b++) {
      if (i < num(lengths[b])) {
        blockAt(data, b).push(num(codewords[cursor]));
        cursor++;
      }
    }
  }
  for (let i = 0; i < ecPerBlock; i++) {
    for (let b = 0; b < blockCount; b++) {
      blockAt(ec, b).push(num(codewords[cursor]));
      cursor++;
    }
  }
  return { data, ec };
}

function decodePayload(dataBlocks: readonly number[][], version: number): string {
  const stream = dataBlocks.flat();
  let cursor = 0;
  const take = (width: number): number => {
    let value = 0;
    for (let i = 0; i < width; i++) {
      const byte = num(stream[cursor >> 3]);
      value = (value << 1) | ((byte >> (7 - (cursor & 7))) & 1);
      cursor++;
    }
    return value;
  };

  const mode = take(4);
  if (mode !== 0b0100) {
    throw new Error(`expected byte mode, read mode ${mode}`);
  }
  const length = take(version < 10 ? 8 : 16);
  const bytes = new Uint8Array(length);
  for (let i = 0; i < length; i++) {
    bytes[i] = take(8);
  }
  return new TextDecoder().decode(bytes);
}

interface Decoded {
  version: number;
  mask: number;
  ecLevel: number;
  text: string;
  blocks: Blocks;
}

function decode(matrix: Matrix): Decoded {
  const version = versionOf(matrix);
  const format = readFormat(matrix, 0);
  const second = readFormat(matrix, 1);
  expect(second).toEqual(format);
  const codewords = readCodewords(matrix, format.mask);
  const blocks = deinterleave(codewords, version);
  return {
    version,
    mask: format.mask,
    ecLevel: format.ecLevel,
    text: decodePayload(blocks.data, version),
    blocks,
  };
}

const ASCII_200 = 'Typvia pairing payload '.repeat(9).slice(0, 200);
const BASE64_LIKE =
  `${'QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVphYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejAxMjM0'.repeat(
    6,
  )}NTY3ODkrLw==`.slice(0, 420);
const CJK = '设备配对码 · Save once. Type anywhere. 键盘扩展需要完全访问权限';

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=';

/** Deterministic pseudo-random payload, so the mask survey is reproducible. */
function syntheticPayload(seed: number): string {
  let state = (seed * 2654435761) >>> 0;
  const next = (): number => {
    state = (Math.imul(state, 1103515245) + 12345) >>> 0;
    return state >>> 16;
  };
  const length = 1 + (next() % 90);
  let value = '';
  for (let i = 0; i < length; i++) {
    value += ALPHABET.charAt(next() % ALPHABET.length);
  }
  return value;
}

const PAYLOADS: readonly { name: string; value: string }[] = [
  { name: 'single character', value: 'a' },
  { name: '200-char ascii', value: ASCII_200 },
  { name: '420-char base64-ish pairing code', value: BASE64_LIKE },
  { name: 'mixed CJK and ascii', value: CJK },
];

describe('qrMatrix', () => {
  it('encodes a short ascii payload into a version 1 matrix', () => {
    const matrix = qrMatrix('a');
    expect(matrix).toHaveLength(21);
    expect(matrix.every((row) => row.length === 21)).toBe(true);
  });

  it('grows the symbol as the payload grows', () => {
    const sizes = PAYLOADS.map(({ value }) => qrMatrix(value).length);
    expect(sizes[0]).toBe(21);
    expect(num(sizes[1])).toBeGreaterThan(num(sizes[0]));
    expect(num(sizes[2])).toBeGreaterThan(num(sizes[1]));
  });

  it('picks the smallest version at the published level-M byte capacities', () => {
    const boundaries: readonly [number, number][] = [
      [14, 21],
      [15, 25],
      [26, 25],
      [27, 29],
      [42, 29],
      [43, 33],
      [213, 57],
      [214, 61],
    ];
    for (const [byteLength, expectedSize] of boundaries) {
      expect(qrMatrix('x'.repeat(byteLength))).toHaveLength(expectedSize);
    }
  });

  it('reports level M and a mask in range in both format information copies', () => {
    for (const { name, value } of PAYLOADS) {
      const { ecLevel, mask } = decode(qrMatrix(value));
      expect({ name, ecLevel }).toEqual({ name, ecLevel: 0b00 });
      expect(mask).toBeGreaterThanOrEqual(0);
      expect(mask).toBeLessThanOrEqual(7);
    }
  });

  it('protects the format and version information with valid BCH codes', () => {
    for (const { name, value } of PAYLOADS) {
      const matrix = qrMatrix(value);
      for (const copy of [0, 1] as const) {
        const codeword = readFormatCodeword(matrix, copy) ^ 0x5412;
        expect({ name, copy, remainder: polyMod(codeword, 0x537) }).toEqual({
          name,
          copy,
          remainder: 0,
        });
      }

      const version = versionOf(matrix);
      if (version >= 7) {
        for (const mirrored of [false, true]) {
          const codeword = readVersionCodeword(matrix, mirrored);
          expect({ name, mirrored, remainder: polyMod(codeword, 0x1f25) }).toEqual({
            name,
            mirrored,
            remainder: 0,
          });
          expect(codeword >> 12).toBe(version);
        }
      }
    }
  });

  it('produces blocks whose syndromes are all zero', () => {
    for (const { name, value } of PAYLOADS) {
      const matrix = qrMatrix(value);
      const { version, blocks } = decode(matrix);
      const ecPerBlock = num(EC_PER_BLOCK[version - 1]);
      expect(blocks.data).toHaveLength(num(BLOCK_COUNT[version - 1]));

      for (let b = 0; b < blocks.data.length; b++) {
        const codeword = [...blockAt(blocks.data, b), ...blockAt(blocks.ec, b)];
        const syndromes = Array.from({ length: ecPerBlock }, (_unused, i) =>
          evaluatePolynomial(codeword, gfPow(i)),
        );
        expect({ name, block: b, syndromes }).toEqual({
          name,
          block: b,
          syndromes: new Array<number>(ecPerBlock).fill(0),
        });
      }
    }
  });

  it('round-trips every payload back through a spec decoder', () => {
    for (const { name, value } of PAYLOADS) {
      const { text } = decode(qrMatrix(value));
      expect({ name, text }).toEqual({ name, text: value });
    }
  });

  it('round-trips payloads that exercise all eight data masks', () => {
    const masks = new Set<number>();
    for (let seed = 0; seed < 160; seed++) {
      const value = syntheticPayload(seed);
      const { text, mask } = decode(qrMatrix(value));
      expect({ seed, text }).toEqual({ seed, text: value });
      masks.add(mask);
    }
    expect([...masks].sort()).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
  });

  it('places the finder patterns in the three corners', () => {
    const matrix = qrMatrix(ASCII_200);
    const size = matrix.length;
    const finder = [
      [true, true, true, true, true, true, true],
      [true, false, false, false, false, false, true],
      [true, false, true, true, true, false, true],
      [true, false, true, true, true, false, true],
      [true, false, true, true, true, false, true],
      [true, false, false, false, false, false, true],
      [true, true, true, true, true, true, true],
    ];
    const corners: readonly [number, number][] = [
      [0, 0],
      [0, size - 7],
      [size - 7, 0],
    ];
    for (const [top, left] of corners) {
      const block = Array.from({ length: 7 }, (_unused, r) =>
        Array.from({ length: 7 }, (_unused2, c) => moduleAt(matrix, top + r, left + c)),
      );
      expect(block).toEqual(finder);
    }
  });

  it('alternates the timing patterns and sets the dark module', () => {
    const matrix = qrMatrix(ASCII_200);
    const size = matrix.length;
    for (let i = 8; i < size - 8; i++) {
      expect({ i, row: moduleAt(matrix, 6, i), column: moduleAt(matrix, i, 6) }).toEqual({
        i,
        row: i % 2 === 0,
        column: i % 2 === 0,
      });
    }
    const version = versionOf(matrix);
    expect(moduleAt(matrix, 4 * version + 9, 8)).toBe(true);
  });

  it('rejects a payload larger than the version 40 capacity without echoing it', () => {
    expect(QR_MAX_BYTES).toBe(2331);
    const payload = `${'x'.repeat(QR_MAX_BYTES)}PAIRINGSECRET`;
    let message = '';
    try {
      qrMatrix(payload);
    } catch (error) {
      message = error instanceof Error ? error.message : String(error);
    }
    expect(message).toContain('2331');
    expect(message).not.toContain('PAIRINGSECRET');
    expect(message).not.toContain('xxxxxxxxxx');
  });
});
