//! gRPC protocol types for Druid metrics delivery.
//!
//! Generated from `proto/druid_metrics_v1.proto` via `tonic-build`.
//! This module provides the wire-format types and convenience
//! encode/decode helpers for client and server frames.

// Include the generated protobuf + gRPC code.
// tonic-build places the file at OUT_DIR/<package_path>.rs
// where package "druid_metrics.v1" becomes "druid_metrics/v1.rs".
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/druid_metrics.v1.rs"));
}

// Re-export all generated types at the `protocol` level so callers can do
// `use druid_metrics::protocol::*`.
pub use v1::*;

use prost::Message;

// ─── Client frame encode / decode ──────────────────────────────────────────

/// Encode a [`ClientFrame`] into a length-delimited byte vector.
///
/// # Errors
/// Returns a [`prost::EncodeError`] if the buffer is too small (should never
/// happen with a `Vec`).
pub fn encode_client_frame(frame: &ClientFrame) -> Vec<u8> {
    let mut buf = Vec::with_capacity(frame.encoded_len());
    frame.encode(&mut buf).expect("Vec::encode never fails");
    buf
}

/// Decode a [`ClientFrame`] from raw bytes.
///
/// # Errors
/// Returns a [`prost::DecodeError`] if the bytes are not a valid protobuf.
pub fn decode_client_frame(bytes: &[u8]) -> Result<ClientFrame, prost::DecodeError> {
    ClientFrame::decode(bytes)
}

// ─── Server frame encode / decode ──────────────────────────────────────────

/// Encode a [`ServerFrame`] into a length-delimited byte vector.
pub fn encode_server_frame(frame: &ServerFrame) -> Vec<u8> {
    let mut buf = Vec::with_capacity(frame.encoded_len());
    frame.encode(&mut buf).expect("Vec::encode never fails");
    buf
}

/// Decode a [`ServerFrame`] from raw bytes.
pub fn decode_server_frame(bytes: &[u8]) -> Result<ServerFrame, prost::DecodeError> {
    ServerFrame::decode(bytes)
}
