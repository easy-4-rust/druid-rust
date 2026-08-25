//! Toasty 数据库元数据 Adapter。

use druid_rdbc::error::RdbcError;
use druid_rdbc::spi::PhysicalDatabaseMetaData;
use std::borrow::Cow;

/// Toasty driver capability 的数据库元数据视图。
///
/// Toasty 0.9 没有 RDBC 风格的 metadata 对象。本 Adapter 只报告
/// `Capability`、连接状态和 URL 可以证明的合同；其余方法保持明确 unsupported。
pub struct ToastyDatabaseMetaData<'connection> {
    url: Cow<'connection, str>,
    driver_name: Cow<'connection, str>,
    read_only: bool,
}

impl<'connection> ToastyDatabaseMetaData<'connection> {
    /// 创建借用当前 Toasty 连接配置的 metadata。
    pub(crate) fn new(
        url: &'connection str,
        driver_name: &'connection str,
        read_only: bool,
    ) -> Self {
        Self {
            url: Cow::Borrowed(url),
            driver_name: Cow::Borrowed(driver_name),
            read_only,
        }
    }

    fn is_sqlite(&self) -> bool {
        self.driver_name.eq_ignore_ascii_case("sqlite")
            || self.driver_name.eq_ignore_ascii_case("turso")
    }

    fn is_mysql(&self) -> bool {
        self.driver_name.eq_ignore_ascii_case("mysql")
    }

    fn product_name_core(&self) -> Option<&'static str> {
        if self.is_sqlite() {
            Some("SQLite")
        } else if self.is_mysql() {
            Some("MySQL")
        } else if self.driver_name.eq_ignore_ascii_case("postgresql")
            || self.driver_name.eq_ignore_ascii_case("postgres")
        {
            Some("PostgreSQL")
        } else {
            None
        }
    }
}

impl ToastyDatabaseMetaData<'static> {
    pub(crate) fn owned(url: String, driver_name: String, read_only: bool) -> Self {
        Self {
            url: Cow::Owned(url),
            driver_name: Cow::Owned(driver_name),
            read_only,
        }
    }
}

#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for ToastyDatabaseMetaData<'_> {
    async fn get_url(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.url.as_ref().to_owned()))
    }

    async fn get_user_name(&mut self) -> Result<Option<String>, RdbcError> {
        // Toasty Connection SPI 不暴露认证后的用户名；不从 URL 重建敏感信息。
        Ok(None)
    }

    async fn is_read_only(&mut self) -> Result<bool, RdbcError> {
        Ok(self.read_only)
    }

    async fn get_database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        self.product_name_core().map(str::to_owned).map(Some).ok_or(
            RdbcError::UnsupportedOperation {
                operation: "toasty_database_metadata_product_name",
            },
        )
    }

    async fn get_driver_name(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.driver_name.as_ref().to_owned()))
    }

    async fn get_driver_version(&mut self) -> Result<Option<String>, RdbcError> {
        // Toasty Capability 未公开 driver version。
        Ok(None)
    }

    async fn get_identifier_quote_string(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(if self.is_mysql() { "`" } else { "\"" }.to_owned()))
    }

    async fn get_search_string_escape(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some("\\".to_owned()))
    }

    async fn get_catalog_separator(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(".".to_owned()))
    }

    async fn get_max_connections(&mut self) -> Result<i32, RdbcError> {
        // Toasty Connection 不携带工厂级 max_connections；RDBC 0 表示未知。
        Ok(0)
    }

    async fn supports_transactions(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }

    async fn supports_transaction_isolation_level(
        &mut self,
        level: i32,
    ) -> Result<bool, RdbcError> {
        if self.is_sqlite() {
            Ok(level == 8)
        } else {
            Ok(matches!(level, 1 | 2 | 4 | 8))
        }
    }

    async fn supports_batch_updates(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }

    async fn supports_savepoints(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }

    async fn supports_named_parameters(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

druid_rdbc::impl_standard_wrapper!(ToastyDatabaseMetaData<'static> => DatabaseMetaData);

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for ToastyDatabaseMetaData<'static> {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.product_name_core()
            .map(str::to_owned)
            .ok_or(RdbcError::UnsupportedOperation {
                operation: "toasty_database_metadata_product_name",
            })
    }
}
