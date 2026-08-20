//! 连接回收结果分类。
//!
//! 对应 Java：`DruidDataSource#recycle(DruidPooledConnection)` 中进入
//! `putLast`、`discardConnection` 或 recycle-error 分支的最终处置。

use super::DruidError;

/// 物理连接结束一次租约后的处置方式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionRecycleDisposition {
    /// 状态已复位且验证通过，可以重新进入空闲队列。
    Reusable,
    /// 连接不可复用，必须从池容量中移除。
    Discard {
        /// 回收过程中的错误；`None` 表示正常淘汰或连接已关闭。
        recycle_error: Option<DruidError>,
    },
}

impl ConnectionRecycleDisposition {
    /// 创建不带回收错误的丢弃结果。
    pub fn discard() -> Self {
        Self::Discard {
            recycle_error: None,
        }
    }

    /// 创建带回收错误的丢弃结果。
    pub fn recycle_error(error: DruidError) -> Self {
        Self::Discard {
            recycle_error: Some(error),
        }
    }

    /// 返回连接是否允许重新进入空闲队列。
    pub fn is_reusable(&self) -> bool {
        matches!(self, Self::Reusable)
    }

    /// 返回是否发生了回收错误。
    pub fn has_recycle_error(&self) -> bool {
        matches!(
            self,
            Self::Discard {
                recycle_error: Some(_)
            }
        )
    }
}
