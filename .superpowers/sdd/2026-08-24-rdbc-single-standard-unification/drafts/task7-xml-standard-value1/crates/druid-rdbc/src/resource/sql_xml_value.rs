use crate::error::{RdbcError, SqlException};
use crate::resource::SqlXml;
/// 标准SQLXML独占对象与一次性读写机会（Rust平台映射）；对应 Java: java.sql.SQLXML。
/// 仅保留Send驱动及概念状态，不定义第二套驱动接口，不持有owner或后台任务。
pub(crate) struct SqlXmlValue {
    pub(crate) driver: Box<dyn SqlXml>,
    readable: bool,
    writable: bool,
}
impl SqlXmlValue {
    /// 接管参数driver，返回尚未消费读写机会的状态；驱动仍决定原始可用方向。
    pub(crate) fn new(driver: Box<dyn SqlXml>) -> Self {
        Self {
            driver,
            readable: true,
            writable: true,
        }
    }
    /// 根据参数read选择方向，首次消费返回成功；再次返回HY010，失败或取消不恢复机会。
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
        // 在任何驱动await之前消费；调用者取消不能重放同一方向的操作。
        *available = false;
        Ok(())
    }
}
