// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Self-contained QR Code encoder (ISO/IEC 18004): byte mode, error-correction
// level M, versions 1..40. Device pairing renders offline on every platform, so
// the symbol is generated locally instead of pulling in an encoder dependency.

/**
 * Error-correction codewords per block, level M, versions 1..40
 * (ISO/IEC 18004 table 9). Index 0 holds version 1.
 */
const EC_CODEWORDS_PER_BLOCK: readonly number[] = [
  10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28,
  28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
];

/**
 * Error-correction block count, level M, versions 1..40 (ISO/IEC 18004 table 9).
 * Index 0 holds version 1.
 */
const EC_BLOCKS: readonly number[] = [
  1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26,
  28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
];

const MIN_VERSION = 1;
const MAX_VERSION = 40;

/** Byte mode indicator (ISO/IEC 18004 table 2). */
const MODE_BYTE = 0b0100;

/** Level M is 0b00 in the format information (ISO/IEC 18004 table 12). */
const FORMAT_EC_BITS = 0b00;

/** Format information is XOR-masked with this value so it is never all-zero. */
const FORMAT_MASK = 0x5412;

/** Generator polynomials for the format (BCH 15,5) and version (BCH 18,6) codes. */
const FORMAT_GENERATOR = 0x537;
const VERSION_GENERATOR = 0x1f25;

/** GF(256) primitive modulus x^8 + x^4 + x^3 + x^2 + 1 (ISO/IEC 18004 §7.5.2). */
const GF_MODULUS = 0x11d;

/** Padding codewords alternated after the terminator (ISO/IEC 18004 §7.4.10). */
const PAD_CODEWORDS: readonly number[] = [0xec, 0x11];

/** Penalty weights for mask evaluation (ISO/IEC 18004 table 11). */
const PENALTY_N1 = 3;
const PENALTY_N2 = 3;
const PENALTY_N3 = 40;
const PENALTY_N4 = 10;

/** Finder-like sequence scored by penalty rule 3, and its mirror image. */
const FINDER_RUN: readonly boolean[] = [
  true,
  false,
  true,
  true,
  true,
  false,
  true,
  false,
  false,
  false,
  false,
];

/**
 * Bounds-checked read. Every index the encoder computes is derived from the
 * version tables, so a miss means the tables and the code disagree.
 */
function at(values: readonly number[], index: number): number {
  const value = values[index];
  if (value === undefined) {
    throw new Error('QR encoder read outside its codeword buffer');
  }
  return value;
}

function bitAt(value: number, index: number): boolean {
  return ((value >>> index) & 1) !== 0;
}

/**
 * Modules available to data and error correction, before the 8-bit split.
 * Derived from the symbol area minus the function patterns, which are fully
 * determined by the version (ISO/IEC 18004 §7.7.3).
 */
function rawDataModules(version: number): number {
  let modules = (16 * version + 128) * version + 64;
  if (version >= 2) {
    const alignCount = Math.floor(version / 7) + 2;
    modules -= (25 * alignCount - 10) * alignCount - 55;
    if (version >= 7) {
      // Two 6x3 version information blocks.
      modules -= 36;
    }
  }
  return modules;
}

function totalCodewords(version: number): number {
  return Math.floor(rawDataModules(version) / 8);
}

function dataCodewords(version: number): number {
  return (
    totalCodewords(version) - at(EC_CODEWORDS_PER_BLOCK, version - 1) * at(EC_BLOCKS, version - 1)
  );
}

/** Character count field width for byte mode (ISO/IEC 18004 table 3). */
function charCountBits(version: number): number {
  return version < 10 ? 8 : 16;
}

function byteCapacity(version: number): number {
  const available = dataCodewords(version) * 8 - 4 - charCountBits(version);
  return Math.floor(available / 8);
}

/** Largest UTF-8 payload this encoder can carry (version 40, level M, byte mode). */
export const QR_MAX_BYTES: number = byteCapacity(MAX_VERSION);

function smallestVersion(byteLength: number): number {
  for (let version = MIN_VERSION; version <= MAX_VERSION; version++) {
    if (byteLength <= byteCapacity(version)) {
      return version;
    }
  }
  throw new Error(
    `QR payload of ${byteLength} bytes exceeds the ${QR_MAX_BYTES}-byte capacity of version 40-M`,
  );
}

/** Header, payload, terminator and padding, as codewords for the chosen version. */
function buildDataCodewords(bytes: Uint8Array, version: number): number[] {
  const bits: boolean[] = [];
  const appendBits = (value: number, width: number): void => {
    for (let i = width - 1; i >= 0; i--) {
      bits.push(bitAt(value, i));
    }
  };

  appendBits(MODE_BYTE, 4);
  appendBits(bytes.length, charCountBits(version));
  for (const byte of bytes) {
    appendBits(byte, 8);
  }

  const capacityBits = dataCodewords(version) * 8;
  const terminator = Math.min(4, capacityBits - bits.length);
  appendBits(0, terminator);
  appendBits(0, (8 - (bits.length % 8)) % 8);

  const codewords: number[] = [];
  for (let i = 0; i < bits.length; i += 8) {
    let codeword = 0;
    for (let j = 0; j < 8; j++) {
      codeword = (codeword << 1) | (bits[i + j] === true ? 1 : 0);
    }
    codewords.push(codeword);
  }
  for (let i = 0; codewords.length < dataCodewords(version); i++) {
    codewords.push(at(PAD_CODEWORDS, i % PAD_CODEWORDS.length));
  }
  return codewords;
}

/** Carry-less multiply reduced by the GF(256) primitive modulus. */
function gfMultiply(x: number, y: number): number {
  let product = 0;
  for (let i = 7; i >= 0; i--) {
    product = (product << 1) ^ ((product >>> 7) * GF_MODULUS);
    product ^= ((y >>> i) & 1) * x;
  }
  return product & 0xff;
}

/** Reed–Solomon generator polynomial (x - α^0)(x - α^1)…, without its leading 1. */
function generatorPolynomial(degree: number): number[] {
  const coefficients: number[] = new Array<number>(degree).fill(0);
  coefficients[degree - 1] = 1;
  let root = 1;
  for (let i = 0; i < degree; i++) {
    for (let j = 0; j < degree; j++) {
      coefficients[j] = gfMultiply(at(coefficients, j), root);
      if (j + 1 < degree) {
        coefficients[j] = at(coefficients, j) ^ at(coefficients, j + 1);
      }
    }
    root = gfMultiply(root, 2);
  }
  return coefficients;
}

/** Remainder of data·x^degree divided by the generator — the EC codewords. */
function errorCorrectionCodewords(data: readonly number[], generator: readonly number[]): number[] {
  const remainder: number[] = new Array<number>(generator.length).fill(0);
  for (const codeword of data) {
    const factor = codeword ^ at(remainder, 0);
    remainder.shift();
    remainder.push(0);
    for (let i = 0; i < remainder.length; i++) {
      remainder[i] = at(remainder, i) ^ gfMultiply(at(generator, i), factor);
    }
  }
  return remainder;
}

/**
 * Splits the data codewords into blocks, appends error correction to each and
 * interleaves them in the order the symbol is filled (ISO/IEC 18004 §7.6).
 */
function interleaveBlocks(data: readonly number[], version: number): number[] {
  const blockCount = at(EC_BLOCKS, version - 1);
  const ecPerBlock = at(EC_CODEWORDS_PER_BLOCK, version - 1);
  const shortBlockLength = Math.floor(dataCodewords(version) / blockCount);
  const shortBlockCount = blockCount - (dataCodewords(version) % blockCount);
  const generator = generatorPolynomial(ecPerBlock);

  const dataBlocks: number[][] = [];
  const ecBlocks: number[][] = [];
  let offset = 0;
  for (let i = 0; i < blockCount; i++) {
    const length = shortBlockLength + (i < shortBlockCount ? 0 : 1);
    const block = data.slice(offset, offset + length);
    offset += length;
    dataBlocks.push(block);
    ecBlocks.push(errorCorrectionCodewords(block, generator));
  }

  const result: number[] = [];
  for (let i = 0; i <= shortBlockLength; i++) {
    for (const block of dataBlocks) {
      // The short blocks have no codeword at the final data index.
      if (i < block.length) {
        result.push(at(block, i));
      }
    }
  }
  for (let i = 0; i < ecPerBlock; i++) {
    for (const block of ecBlocks) {
      result.push(at(block, i));
    }
  }
  return result;
}

/**
 * Square module grid. Function modules are tracked alongside the dark bits
 * because masking must leave finders, timing and format areas untouched.
 */
class ModuleGrid {
  readonly size: number;
  private readonly dark: Uint8Array;
  private readonly functional: Uint8Array;

  constructor(version: number) {
    this.size = version * 4 + 17;
    this.dark = new Uint8Array(this.size * this.size);
    this.functional = new Uint8Array(this.size * this.size);
  }

  isDark(x: number, y: number): boolean {
    return this.dark[this.offset(x, y)] === 1;
  }

  isFunctional(x: number, y: number): boolean {
    return this.functional[this.offset(x, y)] === 1;
  }

  setModule(x: number, y: number, dark: boolean): void {
    this.dark[this.offset(x, y)] = dark ? 1 : 0;
  }

  setFunctionModule(x: number, y: number, dark: boolean): void {
    const offset = this.offset(x, y);
    this.dark[offset] = dark ? 1 : 0;
    this.functional[offset] = 1;
  }

  toMatrix(): boolean[][] {
    const matrix: boolean[][] = [];
    for (let y = 0; y < this.size; y++) {
      const row: boolean[] = [];
      for (let x = 0; x < this.size; x++) {
        row.push(this.isDark(x, y));
      }
      matrix.push(row);
    }
    return matrix;
  }

  private offset(x: number, y: number): number {
    if (x < 0 || y < 0 || x >= this.size || y >= this.size) {
      throw new Error('QR module coordinate outside the symbol');
    }
    return y * this.size + x;
  }
}

/** Alignment pattern centre coordinates for a version (ISO/IEC 18004 §7.3.5). */
function alignmentPositions(version: number): number[] {
  if (version === 1) {
    return [];
  }
  const count = Math.floor(version / 7) + 2;
  const size = version * 4 + 17;
  // Version 32 is the one case the general spacing formula does not produce.
  const step = version === 32 ? 26 : Math.ceil((version * 4 + 4) / (count * 2 - 2)) * 2;
  const positions: number[] = [6];
  for (let pos = size - 7; positions.length < count; pos -= step) {
    positions.splice(1, 0, pos);
  }
  return positions;
}

function drawFinderPattern(grid: ModuleGrid, centerX: number, centerY: number): void {
  for (let dy = -4; dy <= 4; dy++) {
    for (let dx = -4; dx <= 4; dx++) {
      const x = centerX + dx;
      const y = centerY + dy;
      if (x < 0 || y < 0 || x >= grid.size || y >= grid.size) {
        continue;
      }
      // Chebyshev distance: rings 0/1 and 3 are dark, ring 2 and the
      // separator at ring 4 are light.
      const ring = Math.max(Math.abs(dx), Math.abs(dy));
      grid.setFunctionModule(x, y, ring !== 2 && ring !== 4);
    }
  }
}

function drawAlignmentPattern(grid: ModuleGrid, centerX: number, centerY: number): void {
  for (let dy = -2; dy <= 2; dy++) {
    for (let dx = -2; dx <= 2; dx++) {
      const ring = Math.max(Math.abs(dx), Math.abs(dy));
      grid.setFunctionModule(centerX + dx, centerY + dy, ring !== 1);
    }
  }
}

/** BCH(15,5)-protected format information for level M and the given mask. */
function formatBits(mask: number): number {
  const data = (FORMAT_EC_BITS << 3) | mask;
  let remainder = data;
  for (let i = 0; i < 10; i++) {
    remainder = (remainder << 1) ^ ((remainder >>> 9) * FORMAT_GENERATOR);
  }
  return (((data << 10) | (remainder & 0x3ff)) ^ FORMAT_MASK) & 0x7fff;
}

/** BCH(18,6)-protected version information, used from version 7 upwards. */
function versionBits(version: number): number {
  let remainder = version;
  for (let i = 0; i < 12; i++) {
    remainder = (remainder << 1) ^ ((remainder >>> 11) * VERSION_GENERATOR);
  }
  return (version << 12) | (remainder & 0xfff);
}

function drawFormatBits(grid: ModuleGrid, mask: number): void {
  const bits = formatBits(mask);
  const size = grid.size;
  for (let i = 0; i <= 5; i++) {
    grid.setFunctionModule(8, i, bitAt(bits, i));
  }
  grid.setFunctionModule(8, 7, bitAt(bits, 6));
  grid.setFunctionModule(8, 8, bitAt(bits, 7));
  grid.setFunctionModule(7, 8, bitAt(bits, 8));
  for (let i = 9; i < 15; i++) {
    grid.setFunctionModule(14 - i, 8, bitAt(bits, i));
  }
  for (let i = 0; i < 8; i++) {
    grid.setFunctionModule(size - 1 - i, 8, bitAt(bits, i));
  }
  for (let i = 8; i < 15; i++) {
    grid.setFunctionModule(8, size - 15 + i, bitAt(bits, i));
  }
  // The module above the lower-left finder is always dark.
  grid.setFunctionModule(8, size - 8, true);
}

function drawFunctionPatterns(grid: ModuleGrid, version: number): void {
  const size = grid.size;
  for (let i = 0; i < size; i++) {
    // Timing patterns run between the finders on row and column 6.
    grid.setFunctionModule(6, i, i % 2 === 0);
    grid.setFunctionModule(i, 6, i % 2 === 0);
  }

  drawFinderPattern(grid, 3, 3);
  drawFinderPattern(grid, size - 4, 3);
  drawFinderPattern(grid, 3, size - 4);

  const positions = alignmentPositions(version);
  for (let i = 0; i < positions.length; i++) {
    for (let j = 0; j < positions.length; j++) {
      const isFinderCorner =
        (i === 0 && j === 0) ||
        (i === 0 && j === positions.length - 1) ||
        (i === positions.length - 1 && j === 0);
      if (!isFinderCorner) {
        drawAlignmentPattern(grid, at(positions, i), at(positions, j));
      }
    }
  }

  if (version >= 7) {
    const bits = versionBits(version);
    for (let i = 0; i < 18; i++) {
      const bit = bitAt(bits, i);
      const far = size - 11 + (i % 3);
      const near = Math.floor(i / 3);
      grid.setFunctionModule(far, near, bit);
      grid.setFunctionModule(near, far, bit);
    }
  }

  // Reserved with mask 0; the chosen mask is written once it is known.
  drawFormatBits(grid, 0);
}

/** Two-module-wide zigzag from the bottom-right corner (ISO/IEC 18004 §7.7.3). */
function drawCodewords(grid: ModuleGrid, codewords: readonly number[]): void {
  const size = grid.size;
  let bitIndex = 0;
  const totalBits = codewords.length * 8;
  for (let right = size - 1; right >= 1; right -= 2) {
    // Column 6 is the vertical timing pattern and carries no data.
    const rightColumn = right <= 6 ? right - 1 : right;
    for (let step = 0; step < size; step++) {
      const upward = ((rightColumn + 1) & 2) === 0;
      const y = upward ? size - 1 - step : step;
      for (let offset = 0; offset < 2; offset++) {
        const x = rightColumn - offset;
        if (grid.isFunctional(x, y)) {
          continue;
        }
        // Remainder bits beyond the codeword stream stay light.
        const dark =
          bitIndex < totalBits && bitAt(at(codewords, bitIndex >>> 3), 7 - (bitIndex & 7));
        grid.setModule(x, y, dark);
        bitIndex++;
      }
    }
  }
}

/** The eight data mask conditions (ISO/IEC 18004 table 10). */
function maskCondition(mask: number, x: number, y: number): boolean {
  switch (mask) {
    case 0:
      return (x + y) % 2 === 0;
    case 1:
      return y % 2 === 0;
    case 2:
      return x % 3 === 0;
    case 3:
      return (x + y) % 3 === 0;
    case 4:
      return (Math.floor(y / 2) + Math.floor(x / 3)) % 2 === 0;
    case 5:
      return ((x * y) % 2) + ((x * y) % 3) === 0;
    case 6:
      return (((x * y) % 2) + ((x * y) % 3)) % 2 === 0;
    case 7:
      return (((x + y) % 2) + ((x * y) % 3)) % 2 === 0;
    default:
      throw new Error('QR mask pattern out of range');
  }
}

/** XOR is its own inverse, so the same call applies and removes a mask. */
function toggleMask(grid: ModuleGrid, mask: number): void {
  for (let y = 0; y < grid.size; y++) {
    for (let x = 0; x < grid.size; x++) {
      if (!grid.isFunctional(x, y) && maskCondition(mask, x, y)) {
        grid.setModule(x, y, !grid.isDark(x, y));
      }
    }
  }
}

function runPenalty(line: readonly boolean[]): number {
  let penalty = 0;
  let runLength = 1;
  for (let i = 1; i < line.length; i++) {
    if (line[i] === line[i - 1]) {
      runLength++;
    } else {
      if (runLength >= 5) {
        penalty += PENALTY_N1 + (runLength - 5);
      }
      runLength = 1;
    }
  }
  if (runLength >= 5) {
    penalty += PENALTY_N1 + (runLength - 5);
  }
  return penalty;
}

function finderPenalty(line: readonly boolean[]): number {
  let penalty = 0;
  for (let i = 0; i + FINDER_RUN.length <= line.length; i++) {
    let forward = true;
    let backward = true;
    for (let j = 0; j < FINDER_RUN.length; j++) {
      const module = line[i + j];
      if (module !== FINDER_RUN[j]) {
        forward = false;
      }
      if (module !== FINDER_RUN[FINDER_RUN.length - 1 - j]) {
        backward = false;
      }
    }
    if (forward || backward) {
      penalty += PENALTY_N3;
    }
  }
  return penalty;
}

/** Total penalty score across the four mask evaluation rules. */
function maskPenalty(grid: ModuleGrid): number {
  const size = grid.size;
  let penalty = 0;
  let darkCount = 0;

  for (let i = 0; i < size; i++) {
    const row: boolean[] = [];
    const column: boolean[] = [];
    for (let j = 0; j < size; j++) {
      row.push(grid.isDark(j, i));
      column.push(grid.isDark(i, j));
    }
    penalty += runPenalty(row) + runPenalty(column);
    penalty += finderPenalty(row) + finderPenalty(column);
    darkCount += row.filter((module) => module).length;
  }

  for (let y = 0; y + 1 < size; y++) {
    for (let x = 0; x + 1 < size; x++) {
      const first = grid.isDark(x, y);
      if (
        first === grid.isDark(x + 1, y) &&
        first === grid.isDark(x, y + 1) &&
        first === grid.isDark(x + 1, y + 1)
      ) {
        penalty += PENALTY_N2;
      }
    }
  }

  const total = size * size;
  const deviation = Math.floor(Math.abs(darkCount * 20 - total * 10) / total);
  return penalty + deviation * PENALTY_N4;
}

/**
 * Encodes `text` as a QR Code symbol and returns its modules as `matrix[row][column]`,
 * `true` meaning a dark module. Byte mode over UTF-8, error-correction level M,
 * smallest fitting version, and the lowest-penalty of the eight data masks.
 *
 * @throws Error if the UTF-8 payload exceeds {@link QR_MAX_BYTES}. The message
 * never repeats the payload, which may carry pairing material.
 */
export function qrMatrix(text: string): boolean[][] {
  const bytes = new TextEncoder().encode(text);
  const version = smallestVersion(bytes.length);
  const codewords = interleaveBlocks(buildDataCodewords(bytes, version), version);

  const grid = new ModuleGrid(version);
  drawFunctionPatterns(grid, version);
  drawCodewords(grid, codewords);

  let bestMask = 0;
  let bestPenalty = Number.POSITIVE_INFINITY;
  for (let mask = 0; mask < 8; mask++) {
    toggleMask(grid, mask);
    drawFormatBits(grid, mask);
    const penalty = maskPenalty(grid);
    if (penalty < bestPenalty) {
      bestPenalty = penalty;
      bestMask = mask;
    }
    toggleMask(grid, mask);
  }

  toggleMask(grid, bestMask);
  drawFormatBits(grid, bestMask);
  return grid.toMatrix();
}
