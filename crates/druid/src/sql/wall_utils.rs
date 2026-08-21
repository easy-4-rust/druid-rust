//! 对应 Java：`com.alibaba.druid.wall.WallUtils`。

use super::{
    Db2WallProvider, MySqlWallProvider, OracleWallProvider, PgWallProvider, SqlServerWallProvider,
    WallConfig,
};
use crate::core::DruidError;

/// 五种 Java 公开方言的 Wall 快捷校验入口。
pub struct WallUtils;

impl WallUtils {
    /// 使用 DB2 默认规则校验 SQL。
    pub fn is_validate_db2(sql: &str) -> Result<bool, DruidError> {
        Db2WallProvider::new().check_valid(sql)
    }

    /// 使用 DB2 调用方规则校验 SQL。
    pub fn is_validate_db2_with_config(sql: &str, config: WallConfig) -> Result<bool, DruidError> {
        Db2WallProvider::with_config(config).check_valid(sql)
    }

    /// 使用 `PostgreSQL` 默认规则校验 SQL。
    pub fn is_validate_postgres(sql: &str) -> Result<bool, DruidError> {
        PgWallProvider::new().check_valid(sql)
    }

    /// 使用 `PostgreSQL` 调用方规则校验 SQL。
    pub fn is_validate_postgres_with_config(
        sql: &str,
        config: WallConfig,
    ) -> Result<bool, DruidError> {
        PgWallProvider::with_config(config).check_valid(sql)
    }

    /// 使用 `MySQL` 默认规则校验 SQL。
    pub fn is_validate_my_sql(sql: &str) -> Result<bool, DruidError> {
        MySqlWallProvider::new().check_valid(sql)
    }

    /// 使用 `MySQL` 调用方规则校验 SQL。
    pub fn is_validate_my_sql_with_config(
        sql: &str,
        config: WallConfig,
    ) -> Result<bool, DruidError> {
        MySqlWallProvider::with_config(config).check_valid(sql)
    }

    /// 使用 Oracle 默认规则校验 SQL。
    pub fn is_validate_oracle(sql: &str) -> Result<bool, DruidError> {
        OracleWallProvider::new().check_valid(sql)
    }

    /// 使用 Oracle 调用方规则校验 SQL。
    pub fn is_validate_oracle_with_config(
        sql: &str,
        config: WallConfig,
    ) -> Result<bool, DruidError> {
        OracleWallProvider::with_config(config).check_valid(sql)
    }

    /// 使用 SQL Server 默认规则校验 SQL。
    pub fn is_validate_sql_server(sql: &str) -> Result<bool, DruidError> {
        SqlServerWallProvider::new().check_valid(sql)
    }

    /// 使用 SQL Server 调用方规则校验 SQL。
    pub fn is_validate_sql_server_with_config(
        sql: &str,
        config: WallConfig,
    ) -> Result<bool, DruidError> {
        SqlServerWallProvider::with_config(config).check_valid(sql)
    }
}
