/// Filter 链公共契约的兼容名称。
///
/// 对应 Java: `com.alibaba.druid.filter.FilterChain`。Java 接口与
/// `FilterChainImpl` 的公开方法集合由 Rust 具体类型直接承载；保留此名称让
/// 既有调用点不需要动态 trait object，也不复制 384 个精确重载。
pub type FilterChain = super::filter_chain_impl::FilterChainImpl;
