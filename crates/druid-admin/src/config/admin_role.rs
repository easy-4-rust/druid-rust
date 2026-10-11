/// Admin 请求经认证后获得的最小权限角色。
///
/// Rust-only 安全类型；Java MonitorViewServlet 未区分 gRPC 采集身份。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AdminRole {
    /// 读取管理信息并发出操作命令。
    Operator,
    /// 仅向 gRPC ingest 写入指标。
    Ingest,
}
