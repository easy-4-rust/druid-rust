use std::borrow::Cow;

use super::DbType;

/// JDBC 平台工具门面。
///
/// 对应 Java：`com.alibaba.druid.util.JdbcUtils`。Rust 不提供 JDBC close
/// helpers（RAII/Drop 承担），但完整保留 Druid 依赖的 URL/driver 数据库识别。
#[derive(Debug, Default, Clone, Copy)]
pub struct JdbcUtils;

impl JdbcUtils {
    /// 根据 URL 和驱动类名推断数据库类型。
    #[must_use]
    pub fn get_db_type(raw_url: Option<&str>, driver_class_name: Option<&str>) -> Option<DbType> {
        if let Some(url) = raw_url {
            let lower = url.to_ascii_lowercase();
            for (prefix, db_type) in JDBC_URL_PREFIXES {
                if lower.starts_with(prefix) {
                    return Some(*db_type);
                }
            }
            for (prefix, db_type) in RUST_URL_PREFIXES {
                if lower.starts_with(prefix) {
                    return Some(*db_type);
                }
            }
        }
        let driver = driver_class_name?.to_ascii_lowercase();
        DRIVER_IDENTITIES
            .iter()
            .find_map(|(identity, db_type)| driver.contains(identity).then_some(*db_type))
    }

    /// 将 Toasty/SQLx 能表达的 JDBC URL 转为 Rust 原生 DSN。
    ///
    /// 非 JDBC DSN 原样借用；SQLite、PostgreSQL、MySQL/MariaDB 的 JDBC
    /// 前缀被转换。其余 JDBC 驱动返回 `None`，调用方必须选择
    /// `druid-wrapper` 中相应扩展 Adapter，不能把 Java URL 直接传给 Rust
    /// driver 后等待不透明解析错误。
    #[must_use]
    pub fn to_rust_url(raw_url: &str) -> Option<Cow<'_, str>> {
        let lower = raw_url.to_ascii_lowercase();
        let mapping = [
            ("jdbc:sqlite:", "sqlite:"),
            ("jdbc:postgresql:", "postgresql:"),
            ("jdbc:mysql:", "mysql:"),
            ("jdbc:mariadb:", "mysql:"),
        ];
        if let Some((jdbc_prefix, rust_prefix)) = mapping
            .iter()
            .find(|(jdbc_prefix, _)| lower.starts_with(jdbc_prefix))
        {
            return Some(Cow::Owned(format!(
                "{rust_prefix}{}",
                &raw_url[jdbc_prefix.len()..]
            )));
        }
        (!lower.starts_with("jdbc:")).then_some(Cow::Borrowed(raw_url))
    }

    /// 返回是否为 MySQL 协议族。
    #[must_use]
    pub const fn is_mysql_db_type(db_type: DbType) -> bool {
        matches!(
            db_type,
            DbType::MySql
                | DbType::MariaDb
                | DbType::OceanBase
                | DbType::Drds
                | DbType::TiDb
                | DbType::GoldenDb
                | DbType::PolarDbX
                | DbType::AdbMySql
        )
    }

    /// 返回是否为 Oracle 协议族。
    #[must_use]
    pub const fn is_oracle_db_type(db_type: DbType) -> bool {
        matches!(
            db_type,
            DbType::Oracle
                | DbType::AliOracle
                | DbType::OceanBaseOracle
                | DbType::PolarDb
                | DbType::PolarDb2
        )
    }
}

/// Java JDBC 兼容输入；仅用于识别和配置迁移，不直接传给 Rust driver。
const JDBC_URL_PREFIXES: &[(&str, DbType)] = &[
    ("jdbc:log4jdbc:mysql:", DbType::MySql),
    ("jdbc:mysql:", DbType::MySql),
    ("jdbc:cobar:", DbType::MySql),
    ("jdbc:goldendb:", DbType::GoldenDb),
    ("jdbc:mariadb:", DbType::MariaDb),
    ("jdbc:tidb:", DbType::TiDb),
    ("jdbc:log4jdbc:oracle:", DbType::Oracle),
    ("jdbc:oracle:", DbType::Oracle),
    ("jdbc:alibaba:oracle:", DbType::AliOracle),
    ("jdbc:oceanbase:oracle:", DbType::OceanBaseOracle),
    ("jdbc:oceanbase:", DbType::OceanBase),
    ("jdbc:log4jdbc:sqlserver:", DbType::SqlServer),
    ("jdbc:sqlserver:", DbType::SqlServer),
    ("jdbc:microsoft:", DbType::SqlServer),
    ("jdbc:jtds:", DbType::Jtds),
    ("jdbc:postgresql:", DbType::PostgreSql),
    ("jdbc:edb:", DbType::Edb),
    ("jdbc:pivotal:greenplum:", DbType::Greenplum),
    ("jdbc:datadirect:greenplum:", DbType::Greenplum),
    ("jdbc:opengauss:", DbType::GaussDb),
    ("jdbc:gaussdb:", DbType::GaussDb),
    ("jdbc:db2:", DbType::Db2),
    ("jdbc:sqlite:", DbType::SQLite),
    ("jdbc:h2:", DbType::H2),
    ("jdbc:hsqldb:", DbType::Hsql),
    ("jdbc:derby:", DbType::Derby),
    ("jdbc:dm:", DbType::Dm),
    ("jdbc:kingbase8:", DbType::Kingbase),
    ("jdbc:kingbase:", DbType::Kingbase),
    ("jdbc:gbase:", DbType::Gbase),
    ("jdbc:informix-sqli:", DbType::Informix),
    ("jdbc:odps:", DbType::Odps),
    ("jdbc:hive2:", DbType::Hive),
    ("jdbc:hive:", DbType::Hive),
    ("jdbc:phoenix:", DbType::Phoenix),
    ("jdbc:kylin:", DbType::Kylin),
    ("jdbc:elastic:", DbType::ElasticSearch),
    ("jdbc:clickhouse:", DbType::ClickHouse),
    ("jdbc:presto:", DbType::Presto),
    ("jdbc:trino:", DbType::Trino),
    ("jdbc:polardb2:", DbType::PolarDb2),
    ("jdbc:polardbx:", DbType::PolarDbX),
    ("jdbc:polardb:", DbType::PolarDb),
    ("jdbc:highgo:", DbType::HighGo),
    ("jdbc:oscar:", DbType::Oscar),
    ("jdbc:xugu:", DbType::Xugu),
    ("jdbc:firebirdsql:", DbType::FirebirdSql),
    ("jdbc:taos-rs:", DbType::TaosData),
    ("jdbc:taos:", DbType::TaosData),
    ("jdbc:sundb:", DbType::SunDb),
    ("jdbc:fake:", DbType::Mock),
    ("jdbc:mock:", DbType::Mock),
];

/// Rust driver/ORM 常用原生 DSN。
///
/// Toasty 0.9 接受 `sqlite`、`postgresql`/`postgres`、`mysql`、`turso`
/// scheme；SQLx 使用相同的前三类主流 scheme。Turso/libSQL 在 Druid SQL
/// 方言与 Wall 层按 SQLite 族处理。
const RUST_URL_PREFIXES: &[(&str, DbType)] = &[
    ("sqlite:", DbType::SQLite),
    ("libsql:", DbType::SQLite),
    ("turso:", DbType::SQLite),
    ("postgresql:", DbType::PostgreSql),
    ("postgres:", DbType::PostgreSql),
    ("mysql:", DbType::MySql),
];

const DRIVER_IDENTITIES: &[(&str, DbType)] = &[
    ("mysql", DbType::MySql),
    ("mariadb", DbType::MariaDb),
    ("postgres", DbType::PostgreSql),
    ("oracle", DbType::Oracle),
    ("sqlserver", DbType::SqlServer),
    ("sqlite", DbType::SQLite),
    ("clickhouse", DbType::ClickHouse),
    ("db2", DbType::Db2),
    ("h2", DbType::H2),
    ("rbdc", DbType::Other),
    ("toasty", DbType::Other),
];
