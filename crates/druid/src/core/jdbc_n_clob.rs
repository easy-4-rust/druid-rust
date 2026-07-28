//! JDBC `NClob` 平台对象。
//!
//! 对应 Java 平台对象：`java.sql.NClob`。`NClob` 继承 `Clob` 的全部资源操作，
//! 并以类型身份声明内容使用 SQL national character set。

use super::{JdbcClob, PhysicalClob};
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// 物理 `NClob` marker SPI。
///
/// 对应 Java：`java.sql.NClob extends Clob`。
pub trait PhysicalNClob: PhysicalClob {}

/// 对外 JDBC `NClob` 句柄。
///
/// 通过 `Deref` 暴露完整 `JdbcClob` 操作，同时保持独立 `NClob` 类型身份。
#[derive(Clone)]
pub struct JdbcNClob {
    clob: JdbcClob,
    physical: Arc<dyn PhysicalNClob>,
}

impl JdbcNClob {
    /// 包装物理 `NClob` Adapter。
    pub fn new(physical: Arc<dyn PhysicalNClob>) -> Self {
        let clob_physical: Arc<dyn PhysicalClob> = physical.clone();
        Self {
            clob: JdbcClob::new(clob_physical),
            physical,
        }
    }

    /// 返回物理 `NClob` SPI。
    pub fn physical_n_clob(&self) -> &dyn PhysicalNClob {
        self.physical.as_ref()
    }

    /// 返回继承的 `Clob` 句柄。
    pub fn as_clob(&self) -> &JdbcClob {
        &self.clob
    }
}

impl Deref for JdbcNClob {
    type Target = JdbcClob;

    fn deref(&self) -> &Self::Target {
        &self.clob
    }
}

impl fmt::Debug for JdbcNClob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcNClob")
            .field("clob", &self.clob)
            .field("physical", &self.physical)
            .field("freed", &self.is_freed())
            .finish()
    }
}

impl PartialEq for JdbcNClob {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcNClob {}
