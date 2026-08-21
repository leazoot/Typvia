// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Native-messaging wire protocol: 4-byte little-endian length prefix per
//! frame (the browser side of the pipe), JSON payloads, and the typed
//! request/response set the extension speaks. Every
//! inbound payload is untrusted: size-capped before it is read, parsed with
//! unknown message types answered as stable error codes, never panicking.

use std::collections::HashMap;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

/// Inbound frame cap. The extension sends small queries; anything larger is
/// discarded (streamed to a sink, never buffered) and answered with an
/// in-band error so the stream stays aligned.
pub const MAX_INBOUND_BYTES: u32 = 1024 * 1024;
/// Outbound frame cap (browser-imposed 1MB host→extension limit); results
/// are limit-bounded so a violation is a host bug surfaced as an error.
pub const MAX_OUTBOUND_BYTES: usize = 1024 * 1024;

/// One parsed inbound frame, or the ways reading it can end.
pub enum Frame {
    /// A payload within the size cap.
    Payload(Vec<u8>),
    /// A frame that declared more than [`MAX_INBOUND_BYTES`]; its bytes were
    /// consumed and discarded, the stream is aligned on the next frame.
    Oversized,
    /// Clean end of stream (the browser closed the port).
    Eof,
}

/// Reads one length-prefixed frame. IO errors are terminal for the process
/// (the pipe is gone); oversized frames are survivable and reported in-band.
pub fn read_frame<R: Read>(reader: &mut R) -> std::io::Result<Frame> {
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(Frame::Eof),
        Err(error) => return Err(error),
    }
    let len = u32::from_le_bytes(len_bytes);
    if len > MAX_INBOUND_BYTES {
        // Drain without buffering so a hostile length cannot balloon memory.
        std::io::copy(&mut reader.take(u64::from(len)), &mut std::io::sink())?;
        return Ok(Frame::Oversized);
    }
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;
    Ok(Frame::Payload(payload))
}

/// Writes one length-prefixed frame and flushes (the browser waits on it).
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> std::io::Result<()> {
    debug_assert!(payload.len() <= MAX_OUTBOUND_BYTES);
    let len =
        u32::try_from(payload.len().min(MAX_OUTBOUND_BYTES)).unwrap_or(MAX_OUTBOUND_BYTES as u32);
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload[..len as usize])?;
    writer.flush()
}

/// The read-only message surface.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Hello,
    Search {
        query: String,
        limit: Option<u32>,
    },
    ListRecent {
        limit: Option<u32>,
    },
    ListFavorites {
        limit: Option<u32>,
    },
    Render {
        snippet_id: String,
        #[serde(default)]
        variables: HashMap<String, String>,
    },
}

/// Version negotiation + snapshot presence for the popup's first paint.
#[derive(Debug, Serialize)]
pub struct HelloResponse {
    pub ok: bool,
    pub protocol_version: u32,
    pub snapshot_present: bool,
    pub generated_at: Option<i64>,
    pub snippet_count: Option<usize>,
}

/// One search/list hit. Carries no body — insertion always goes through
/// `render`, so the full text crosses the wire exactly once.
#[derive(Debug, Serialize)]
pub struct ResultEntry {
    pub id: String,
    pub title: String,
    pub snippet_type: String,
    pub trigger: Option<String>,
    pub is_favorite: bool,
    /// First line of the body, capped for list display.
    pub preview: String,
    /// Template variable names the popup must collect before rendering.
    pub variables: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ResultsResponse {
    pub ok: bool,
    pub results: Vec<ResultEntry>,
}

#[derive(Debug, Serialize)]
pub struct RenderResponse {
    pub ok: bool,
    pub text: String,
}

/// Stable error codes only — never message content (log/error red line).
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub ok: bool,
    pub code: &'static str,
    /// Variable name for `missing_variable`; never snippet content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl ErrorResponse {
    pub fn code(code: &'static str) -> Self {
        Self {
            ok: false,
            code,
            field: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_through_a_buffer() {
        let mut wire = Vec::new();
        write_frame(&mut wire, br#"{"type":"hello"}"#).unwrap();
        let mut reader = wire.as_slice();
        match read_frame(&mut reader).unwrap() {
            Frame::Payload(payload) => assert_eq!(payload, br#"{"type":"hello"}"#),
            _ => panic!("expected a payload frame"),
        }
        assert!(matches!(read_frame(&mut reader).unwrap(), Frame::Eof));
    }

    #[test]
    fn an_oversized_frame_is_drained_and_the_stream_stays_aligned() {
        let mut wire = Vec::new();
        let huge = MAX_INBOUND_BYTES + 1;
        wire.extend_from_slice(&huge.to_le_bytes());
        wire.extend(std::iter::repeat_n(b'x', huge as usize));
        write_frame(&mut wire, b"{}").unwrap();

        let mut reader = wire.as_slice();
        assert!(matches!(read_frame(&mut reader).unwrap(), Frame::Oversized));
        match read_frame(&mut reader).unwrap() {
            Frame::Payload(payload) => assert_eq!(payload, b"{}"),
            _ => panic!("expected the next frame to parse"),
        }
    }

    #[test]
    fn a_truncated_length_prefix_reads_as_eof() {
        let mut reader: &[u8] = &[0x01, 0x02];
        assert!(matches!(read_frame(&mut reader).unwrap(), Frame::Eof));
    }

    #[test]
    fn requests_parse_by_type_tag_and_reject_unknown_types() {
        let parsed: Request = serde_json::from_slice(br#"{"type":"search","query":"do"}"#).unwrap();
        assert!(matches!(parsed, Request::Search { .. }));
        assert!(serde_json::from_slice::<Request>(br#"{"type":"write_db"}"#).is_err());
    }
}
