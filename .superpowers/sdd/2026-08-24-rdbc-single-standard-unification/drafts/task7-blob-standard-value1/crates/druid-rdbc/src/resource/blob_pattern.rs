use crate::resource::Blob;

/// Blob模式参数的安全别名表示（Rust平台映射）。
/// 对应 Java: java.sql.Blob#position(java.sql.Blob,long) 的第一个参数。
/// 同一对象使用SameResource，避免为同一驱动对象构造重叠的两个可变引用；
/// Other保留另一个真实标准对象，不预先读取或替换其内容。
#[derive(Debug)]
pub enum BlobPattern<'a> {
    /// 模式就是当前接收搜索操作的Blob；由驱动按自身内容与原start处理。
    SameResource,
    /// 独占借用另一个Blob；驱动决定如何读取内容或使用原生定位器。
    Other(&'a mut dyn Blob),
}
