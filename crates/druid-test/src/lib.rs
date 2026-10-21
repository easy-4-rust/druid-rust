//! 整仓验收测试包。
//!
//! 该 crate 只承载跨 crate 的合同/差分/宿主测试，不发布、不包含生产逻辑；
//! 具体断言位于 `tests/`，共享夹具与 SQL 语料位于 `tests/fixtures/`。

/// 验收包版本标识，便于证据文件记录来源。
pub const ACCEPTANCE_PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
