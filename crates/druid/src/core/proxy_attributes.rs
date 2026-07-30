//! Druid Proxy attributes 的 Rust 值域与共享存储。
//!
//! 对应 Java：
//! `com.alibaba.druid.proxy.jdbc.WrapperProxy#getAttributes/putAttribute`。

use parking_lot::RwLock;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// Java nullable `Object` attribute 的 Rust 表示。
#[derive(Clone)]
pub enum ProxyAttributeValue {
    /// Java map 中显式保存的 null。
    Null,
    /// 可在线程间安全共享的具体 Rust 值。
    Value(Arc<dyn Any + Send + Sync>),
}

impl ProxyAttributeValue {
    /// 创建非 null attribute。
    #[must_use]
    pub fn new<T>(value: T) -> Self
    where
        T: Any + Send + Sync,
    {
        Self::Value(Arc::new(value))
    }

    /// 尝试恢复具体值的共享身份。
    #[must_use]
    pub fn downcast<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync,
    {
        let Self::Value(value) = self else {
            return None;
        };
        Arc::clone(value).downcast::<T>().ok()
    }

    /// 返回是否为显式 Java null。
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl std::fmt::Debug for ProxyAttributeValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => formatter.write_str("Null"),
            Self::Value(value) => formatter
                .debug_tuple("Value")
                .field(&value.as_ref().type_id())
                .finish(),
        }
    }
}

/// 一个逻辑 Connection/Statement/ResultSet 自己的 attribute map。
///
/// Java 注释声明该 map 不要求线程安全；Rust pooled handle 可以跨线程移动，
/// 因此使用短临界区 `RwLock`，但仍保留惰性空 map、覆盖、显式 null 和清空语义。
#[derive(Default)]
pub struct ProxyAttributes {
    values: RwLock<Option<HashMap<String, ProxyAttributeValue>>>,
}

impl ProxyAttributes {
    /// 返回 attribute 数量；尚未创建 map 时返回 0。
    #[must_use]
    pub fn len(&self) -> usize {
        self.values
            .read()
            .as_ref()
            .map_or(0, std::collections::HashMap::len)
    }

    /// 返回是否没有 attribute。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 清空 attribute；尚未创建 map 时不分配。
    pub fn clear(&self) {
        if let Some(values) = self.values.write().as_mut() {
            values.clear();
        }
    }

    /// 返回 attribute map 快照。
    ///
    /// 对应 Java `getAttributes()` 的可观察键值；Rust 不返回锁内可变引用。
    #[must_use]
    pub fn snapshot(&self) -> HashMap<String, ProxyAttributeValue> {
        self.values.read().as_ref().cloned().unwrap_or_default()
    }

    /// 返回指定 attribute；缺失与显式 null 通过外层 `Option` 区分。
    #[must_use]
    pub fn get(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.values
            .read()
            .as_ref()
            .and_then(|values| values.get(key))
            .cloned()
    }

    /// 保存或覆盖指定 attribute。
    pub fn put(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.values
            .write()
            .get_or_insert_with(|| HashMap::with_capacity(4))
            .insert(key.into(), value)
    }
}
