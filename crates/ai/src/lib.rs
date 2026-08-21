// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! AI provider adapter layer.
//!
//! This crate defines the [`AiProvider`] trait boundary that business code
//! programs against, plus the single OpenAI-compatible wire client behind
//! it — Ollama, LM Studio, OpenAI-compatible services and custom base URLs
//! all speak that wire format and differ only in configuration
//! ([`ProviderKind`]). HTTP goes through the [`AiTransport`] seam so every
//! test runs against a double instead of the network.
//!
//! Red lines honored here: API keys exist only in the platform secure store
//! and in [`ApiKey`] buffers that wipe on drop; they travel exclusively in
//! the `Authorization` header over TLS or loopback; nothing in this crate
//! logs free-form text, and no error carries key material or
//! request/response content. Every request passes the mandatory egress gate
//! ([`egress`]): sensitive-looking content
//! is refused before serialization, and the metadata-only egress log is
//! written before the transport is touched — fail-closed, with no switch
//! that disables either. No concrete AI feature (organizing, extraction,
//! actions) lives in this crate.

pub mod config;
pub mod credentials;
pub mod egress;
pub mod error;
pub mod http;
pub mod provider;
pub mod transport;

pub use config::{ProviderConfig, ProviderKind, ProviderKindExt};
pub use credentials::{ApiKey, CredentialError};
pub use egress::{EgressEntry, EgressLog, EgressLogError, vetted_snippet_text};
pub use error::{AiError, ErrorClass};
pub use http::HttpAiTransport;
pub use provider::{
    AiProvider, ChatMessage, ChatOutcome, ChatRequest, Connectivity, OpenAiCompatProvider, Role,
};
pub use transport::{
    AiTransport, CancelToken, TransportFailure, TransportRequest, TransportResponse,
};
pub use typvia_core::model::AiRequestClass;
