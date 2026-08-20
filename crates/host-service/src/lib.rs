//! Shared desktop/mobile IPC orchestration layer: pure-Connection use-case
//! orchestration over the core traits, the wire DTOs, and the stable IPC
//! error codes. This crate knows nothing of Tauri types or platform
//! capabilities — injector- and Espanso-coupled use cases stay in the
//! desktop host.

pub mod ai;
pub mod dto;
pub mod error;
pub mod service;
pub mod snapshot;
