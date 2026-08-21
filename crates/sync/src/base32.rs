// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! RFC 4648 base32 (upper-case alphabet, no padding). Encoding serves the
//! device-fingerprint display code and the SAS short code; decoding serves
//! only the recovery code, the one base32 value a user types back in.

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Encodes `bytes` as unpadded upper-case base32.
pub(crate) fn encode_nopad(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    // Bit accumulator: at most 12 bits are pending when 8 more arrive.
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 0x1F) as usize] as char);
        }
    }
    if bits > 0 {
        // Final partial group, left-aligned per RFC 4648 (padding omitted).
        out.push(ALPHABET[((buffer << (5 - bits)) & 0x1F) as usize] as char);
    }
    out
}

/// Decodes unpadded upper-case base32 into exactly `N` bytes. `None` for
/// characters outside the alphabet or a length that cannot encode `N` bytes.
pub(crate) fn decode_nopad<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != (N * 8).div_ceil(5) {
        return None;
    }
    let mut out = [0u8; N];
    let mut filled = 0usize;
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for ch in text.bytes() {
        let value = ALPHABET.iter().position(|&a| a == ch)? as u32;
        buffer = (buffer << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            if filled == N {
                return None;
            }
            out[filled] = ((buffer >> bits) & 0xFF) as u8;
            filled += 1;
        }
    }
    // Trailing bits must be zero padding (canonical encoding only).
    if filled != N || (buffer & ((1 << bits) - 1)) != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_rfc_4648_test_vectors() {
        assert_eq!(encode_nopad(b""), "");
        assert_eq!(encode_nopad(b"f"), "MY");
        assert_eq!(encode_nopad(b"fo"), "MZXQ");
        assert_eq!(encode_nopad(b"foo"), "MZXW6");
        assert_eq!(encode_nopad(b"foob"), "MZXW6YQ");
        assert_eq!(encode_nopad(b"fooba"), "MZXW6YTB");
        assert_eq!(encode_nopad(b"foobar"), "MZXW6YTBOI");
    }

    #[test]
    fn a_32_byte_digest_encodes_to_52_characters() {
        assert_eq!(encode_nopad(&[0u8; 32]).len(), 52);
    }

    #[test]
    fn decode_round_trips_encode_for_16_bytes() {
        let bytes: [u8; 16] = *b"0123456789ABCDEF";
        let encoded = encode_nopad(&bytes);
        assert_eq!(decode_nopad::<16>(&encoded), Some(bytes));
    }

    #[test]
    fn decode_rejects_wrong_length_bad_characters_and_dirty_padding() {
        assert_eq!(decode_nopad::<16>("SHORT"), None);
        // Right length, character outside the alphabet ('1' is excluded).
        let bad = "1".repeat(26);
        assert_eq!(decode_nopad::<16>(&bad), None);
        // Right length and alphabet, but non-zero trailing padding bits:
        // flip the last character of a valid encoding to a value with low
        // bits set ('7' = 31).
        let mut dirty = encode_nopad(&[0u8; 16]);
        dirty.pop();
        dirty.push('7');
        assert_eq!(decode_nopad::<16>(&dirty), None);
    }
}
