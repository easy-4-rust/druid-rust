use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::resource::SqlXml;

/// 在独占驱动对象旁保存 SQLXML 的一次性读写机会，取消也不恢复已进入驱动的操作。
/// 对应 Java: java.sql.SQLXML 的 readable/writable 概念状态。
pub(crate) struct DruidSqlXmlPayload {
    pub(crate) driver: Box<dyn SqlXml>,
    readable: bool,
    writable: bool,
}
impl DruidSqlXmlPayload {
    /// 接管驱动对象，返回读写机会尚未消费的状态；驱动自身仍可拒绝不可用方向。
    pub(crate) fn new(driver: Box<dyn SqlXml>) -> Self {
        Self {
            driver,
            readable: true,
            writable: true,
        }
    }
    /// 根据 read 选择读取或写入机会；首次调用消费机会，再次调用返回标准 SQL 异常。
    pub(crate) fn admit(&mut self, read: bool) -> Result<(), RdbcError> {
        let available = if read {
            &mut self.readable
        } else {
            &mut self.writable
        };
        if !*available {
            return Err(RdbcError::SqlException(Box::new(SqlException::new(
                0,
                Some("HY010".to_owned()),
                Some(format!(
                    "SQLXML is no longer {}",
                    if read { "readable" } else { "writable" }
                )),
            ))));
        }
        // 必须在任何驱动 await 前消费；future 被取消不允许再次执行同方向操作。
        *available = false;
        Ok(())
    }
}
