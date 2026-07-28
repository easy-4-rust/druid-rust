//! 对应 Java 类：com.alibaba.druid.pool.WrapperAdapter + PoolableWrapper

/// Wrapper trait，替代 Java 的 javax.sql.Wrapper。
pub trait Wrapper: Send + Sync {
    fn is_wrapper_for(&self, type_name: &str) -> bool {
        let _ = type_name;
        false
    }
}
