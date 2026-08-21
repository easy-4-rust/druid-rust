//! Druid 指标传输使用的 gRPC 协议类型。
//!
//! 所有类型由唯一的 `proto/druid_metrics_v1.proto` 生成。Cargo 的 `client`
//! 与 `server` feature 分别启用对应 Tonic stub；未启用任一 feature 时仍生成消息类型。
//! 本地遥测快照通过 `snapshot_encoder` 转换为这里的 [`SnapshotEntry`]，
//! 聚合阶段不再使用无类型 JSON payload。

// 包含生成的 protobuf 与 gRPC 代码。
// tonic-prost-build 将 package `druid_metrics.v1` 输出到 OUT_DIR/druid_metrics.v1.rs。
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/druid_metrics.v1.rs"));
}

// 在 protocol 层重导出全部生成类型，供调用方统一引用。
pub use v1::*;

use prost::Message;

// ─── Client frame encode / decode ──────────────────────────────────────────

/// 将 [`ClientFrame`] 编码为 protobuf 字节数组。
///
/// `Vec` 动态扩容，因此正常编码不会失败。
pub fn encode_client_frame(frame: &ClientFrame) -> Vec<u8> {
    let mut buf = Vec::with_capacity(frame.encoded_len());
    frame.encode(&mut buf).expect("Vec::encode never fails");
    buf
}

/// 从 protobuf 字节解码 [`ClientFrame`]。
///
/// # Errors
/// 输入不是合法 protobuf 时返回 [`prost::DecodeError`]。
pub fn decode_client_frame(bytes: &[u8]) -> Result<ClientFrame, prost::DecodeError> {
    ClientFrame::decode(bytes)
}

// ─── Server frame encode / decode ──────────────────────────────────────────

/// 将 [`ServerFrame`] 编码为 protobuf 字节数组。
pub fn encode_server_frame(frame: &ServerFrame) -> Vec<u8> {
    let mut buf = Vec::with_capacity(frame.encoded_len());
    frame.encode(&mut buf).expect("Vec::encode never fails");
    buf
}

/// 从 protobuf 字节解码 [`ServerFrame`]。
pub fn decode_server_frame(bytes: &[u8]) -> Result<ServerFrame, prost::DecodeError> {
    ServerFrame::decode(bytes)
}
