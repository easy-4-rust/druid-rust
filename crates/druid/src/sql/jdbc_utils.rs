use std::borrow::Cow;

use super::DbType;

/// JDBC 平台工具门面。
///
/// 对应 Java：`com.alibaba.druid.util.JdbcUtils`。Rust 不提供 JDBC close
/// helpers（RAII/Drop 承担），但完整保留 Druid 依赖的 URL/driver 数据库识别。
#[derive(Debug, Default, Clone, Copy)]
pub struct JdbcUtils;

impl JdbcUtils {
    /// 按 Java `JdbcUtils#getDbTypeRaw` 识别 JDBC URL。
    ///
    /// Java 源实现虽然接收 `driverClassName`，但当前版本并未读取它；Rust
    /// 保留该参数和精确的大小写、前缀顺序。JVM 专属 log4jdbc 包装驱动的
    /// catch-all 类型不进入 Rust `DbType`，已知 vendor 包装前缀仍返回真实
    /// 数据库类型。
    #[must_use]
    pub fn get_db_type_raw(
        raw_url: Option<&str>,
        _driver_class_name: Option<&str>,
    ) -> Option<DbType> {
        let raw_url = raw_url?;
        JDBC_URL_PREFIXES
            .iter()
            .find_map(|(prefix, db_type)| raw_url.starts_with(prefix).then_some(*db_type))
    }

    /// 按 Java `JdbcUtils#getDbType` 返回精确枚举名称。
    #[must_use]
    pub fn get_db_type(
        raw_url: Option<&str>,
        driver_class_name: Option<&str>,
    ) -> Option<&'static str> {
        Self::get_db_type_raw(raw_url, driver_class_name).map(DbType::as_str)
    }

    /// 识别 JDBC 迁移输入或 Rust 原生 DSN/驱动身份。
    ///
    /// 这是 Rust 扩展，不冒充 Java `getDbTypeRaw`：先应用 Java JDBC URL
    /// 规则，再识别 Toasty/SQLx 等原生 scheme，最后才使用驱动身份提示。
    #[must_use]
    pub fn infer_db_type(raw_url: Option<&str>, driver_identity: Option<&str>) -> Option<DbType> {
        if let Some(db_type) = Self::get_db_type_raw(raw_url, driver_identity) {
            return Some(db_type);
        }
        if let Some(raw_url) = raw_url {
            let lower = raw_url.to_ascii_lowercase();
            if let Some(db_type) = RUST_URL_PREFIXES
                .iter()
                .find_map(|(prefix, db_type)| lower.starts_with(prefix).then_some(*db_type))
            {
                return Some(db_type);
            }
        }
        let driver = driver_identity?.to_ascii_lowercase();
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
            ("jdbc:log4jdbc:mysql:", "mysql:"),
            ("jdbc:log4jdbc:postgresql:", "postgresql:"),
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

    /// 返回 Java `java.sql.Types` 的显示名称。
    ///
    /// 对应 Java：`JdbcUtils#getTypeName(int)`。未列入源 switch 的值即使
    /// 在新版 JDBC 中有名称，也保持返回 `OTHER`。
    #[must_use]
    pub const fn get_type_name(sql_type: i32) -> &'static str {
        match sql_type {
            2003 => "ARRAY",
            -5 => "BIGINT",
            -2 => "BINARY",
            -7 => "BIT",
            2004 => "BLOB",
            16 => "BOOLEAN",
            1 => "CHAR",
            2005 => "CLOB",
            70 => "DATALINK",
            91 => "DATE",
            3 => "DECIMAL",
            2001 => "DISTINCT",
            8 => "DOUBLE",
            6 => "FLOAT",
            4 => "INTEGER",
            2000 => "JAVA_OBJECT",
            -16 => "LONGNVARCHAR",
            -4 => "LONGVARBINARY",
            -15 => "NCHAR",
            2011 => "NCLOB",
            0 => "NULL",
            2 => "NUMERIC",
            -9 => "NVARCHAR",
            7 => "REAL",
            2006 => "REF",
            -8 => "ROWID",
            5 => "SMALLINT",
            2009 => "SQLXML",
            2002 => "STRUCT",
            92 => "TIME",
            93 => "TIMESTAMP",
            2014 => "TIMESTAMP_WITH_TIMEZONE",
            -6 => "TINYINT",
            -3 => "VARBINARY",
            12 => "VARCHAR",
            _ => "OTHER",
        }
    }

    /// 返回是否为 MySQL 协议族。
    #[must_use]
    pub const fn is_mysql_db_type(db_type: DbType) -> bool {
        matches!(
            db_type,
            DbType::MySql
                | DbType::OceanBase
                | DbType::Ads
                | DbType::Drds
                | DbType::MariaDb
                | DbType::TiDb
                | DbType::H2
                | DbType::Lealone
                | DbType::GoldenDb
                | DbType::PolarDbX
        )
    }

    /// 返回是否为 Oracle 协议族。
    #[must_use]
    pub const fn is_oracle_db_type(db_type: DbType) -> bool {
        matches!(
            db_type,
            DbType::Oracle | DbType::OceanBaseOracle | DbType::AliOracle
        )
    }

    /// 按 Java 字符串重载判断 Oracle 协议族。
    #[must_use]
    pub fn is_oracle_db_type_name(db_type: &str) -> bool {
        db_type == DbType::Oracle.as_str()
            || db_type == DbType::OceanBaseOracle.as_str()
            || db_type.eq_ignore_ascii_case(DbType::AliOracle.as_str())
    }

    /// 按 Java 字符串重载判断 MySQL 协议族。
    #[must_use]
    pub fn is_mysql_db_type_name(db_type_name: &str) -> bool {
        DbType::of(db_type_name).is_some_and(Self::is_mysql_db_type)
    }

    /// 返回是否属于 Java JdbcUtils 的 PostgreSQL 协议族。
    #[must_use]
    pub const fn is_pgsql_db_type(db_type: DbType) -> bool {
        matches!(
            db_type,
            DbType::PostgreSql
                | DbType::Edb
                | DbType::PolarDb
                | DbType::Greenplum
                | DbType::GaussDb
                | DbType::Hologres
        )
    }

    /// 按 Java 字符串重载判断 PostgreSQL 协议族。
    #[must_use]
    pub fn is_pgsql_db_type_name(db_type_name: &str) -> bool {
        DbType::of(db_type_name).is_some_and(Self::is_pgsql_db_type)
    }

    /// 返回是否属于 Java JdbcUtils 的 SQL Server 协议族。
    #[must_use]
    pub const fn is_sqlserver_db_type(db_type: DbType) -> bool {
        matches!(db_type, DbType::SqlServer | DbType::Jtds)
    }

    /// 按 Java 字符串重载判断 SQL Server 协议族。
    #[must_use]
    pub fn is_sqlserver_db_type_name(db_type_name: &str) -> bool {
        DbType::of(db_type_name).is_some_and(Self::is_sqlserver_db_type)
    }

    /// 判断是否为 Java MySQL Connector/J 的四个标准 driver class name。
    #[must_use]
    pub fn is_my_sql_driver(driver_class_name: &str) -> bool {
        matches!(
            driver_class_name,
            "com.mysql.jdbc.Driver"
                | "com.mysql.cj.jdbc.Driver"
                | "com.mysql.cj.api.MysqlConnection"
                | "com.mysql.jdbc."
        )
    }
}

/// Java JDBC 兼容输入；仅用于识别和配置迁移，不直接传给 Rust driver。
const JDBC_URL_PREFIXES: &[(&str, DbType)] = &[
    ("jdbc:derby:", DbType::Derby),
    ("jdbc:log4jdbc:derby:", DbType::Derby),
    ("jdbc:log4jdbc:mysql:", DbType::MySql),
    ("jdbc:mysql:", DbType::MySql),
    ("jdbc:cobar:", DbType::MySql),
    // Java 1.2.28 历史行为：goldendb URL 返回 mysql，而非 goldendb。
    ("jdbc:goldendb:", DbType::MySql),
    ("jdbc:mariadb:", DbType::MariaDb),
    ("jdbc:tidb:", DbType::TiDb),
    ("jdbc:log4jdbc:oracle:", DbType::Oracle),
    ("jdbc:oracle:", DbType::Oracle),
    ("jdbc:alibaba:oracle:", DbType::AliOracle),
    ("jdbc:oceanbase:oracle:", DbType::OceanBaseOracle),
    ("jdbc:oceanbase:", DbType::OceanBase),
    ("jdbc:log4jdbc:microsoft:", DbType::SqlServer),
    ("jdbc:log4jdbc:sqlserver:", DbType::SqlServer),
    ("jdbc:sqlserver:", DbType::SqlServer),
    ("jdbc:microsoft:", DbType::SqlServer),
    ("jdbc:sybase:Tds:", DbType::Sybase),
    ("jdbc:log4jdbc:sybase:", DbType::Sybase),
    ("jdbc:jtds:", DbType::Jtds),
    ("jdbc:log4jdbc:jtds:", DbType::Jtds),
    ("jdbc:fake:", DbType::Mock),
    ("jdbc:mock:", DbType::Mock),
    ("jdbc:postgresql:", DbType::PostgreSql),
    ("jdbc:log4jdbc:postgresql:", DbType::PostgreSql),
    ("jdbc:edb:", DbType::Edb),
    ("jdbc:hsqldb:", DbType::Hsql),
    ("jdbc:log4jdbc:hsqldb:", DbType::Hsql),
    ("jdbc:odps:", DbType::Odps),
    ("jdbc:db2:", DbType::Db2),
    ("jdbc:sqlite:", DbType::SQLite),
    ("jdbc:ingres:", DbType::Ingres),
    ("jdbc:h2:", DbType::H2),
    ("jdbc:log4jdbc:h2:", DbType::H2),
    ("jdbc:lealone:", DbType::Lealone),
    ("jdbc:mckoi:", DbType::Mock),
    ("jdbc:cloudscape:", DbType::Cloudscape),
    ("jdbc:informix-sqli:", DbType::Informix),
    ("jdbc:log4jdbc:informix-sqli:", DbType::Informix),
    ("jdbc:timesten:", DbType::TimesTen),
    ("jdbc:as400:", DbType::As400),
    ("jdbc:sapdb:", DbType::SapDb),
    ("jdbc:JSQLConnect:", DbType::JsqlConnect),
    ("jdbc:JTurbo:", DbType::JTurbo),
    ("jdbc:firebirdsql:", DbType::FirebirdSql),
    ("jdbc:interbase:", DbType::Interbase),
    ("jdbc:pointbase:", DbType::Pointbase),
    ("jdbc:edbc:", DbType::Edbc),
    ("jdbc:mimer:multi1:", DbType::Mimer),
    ("jdbc:dm:", DbType::Dm),
    ("jdbc:kingbase:", DbType::Kingbase),
    ("jdbc:kingbase8:", DbType::Kingbase),
    ("jdbc:gbase:", DbType::Gbase),
    ("jdbc:xugu:", DbType::Xugu),
    // Java 对未知 log4jdbc vendor 返回 JVM 专属 DbType.log4jdbc；Rust
    // 不制造该伪数据库类型，因此 catch-all 有意不进入本表。
    ("jdbc:hive:", DbType::Hive),
    ("jdbc:hive2:", DbType::Hive),
    ("jdbc:phoenix:", DbType::Phoenix),
    ("jdbc:kylin:", DbType::Kylin),
    ("jdbc:elastic:", DbType::ElasticSearch),
    ("jdbc:clickhouse:", DbType::ClickHouse),
    ("jdbc:presto:", DbType::Presto),
    ("jdbc:trino:", DbType::Trino),
    ("jdbc:inspur:", DbType::Kdb),
    ("jdbc:polardb2:", DbType::PolarDb2),
    ("jdbc:polardbx:", DbType::PolarDbX),
    ("jdbc:polardb", DbType::PolarDb),
    ("jdbc:highgo:", DbType::HighGo),
    ("jdbc:pivotal:greenplum:", DbType::Greenplum),
    ("jdbc:datadirect:greenplum:", DbType::Greenplum),
    ("jdbc:opengauss:", DbType::GaussDb),
    ("jdbc:gaussdb:", DbType::GaussDb),
    ("jdbc:dws:iam:", DbType::GaussDb),
    ("jdbc:TAOS:", DbType::TaosData),
    ("jdbc:TAOS-RS:", DbType::TaosData),
    ("jdbc:oscar:", DbType::Oscar),
    ("jdbc:sundb:", DbType::SunDb),
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
    // rsfbclient native 与 pure-Rust builder 使用同一 DSN。
    ("firebird:", DbType::FirebirdSql),
];

const DRIVER_IDENTITIES: &[(&str, DbType)] = &[
    ("mysql", DbType::MySql),
    ("mariadb", DbType::MariaDb),
    ("postgres", DbType::PostgreSql),
    ("tiberius", DbType::SqlServer),
    ("sibyl", DbType::Oracle),
    ("oracle", DbType::Oracle),
    ("sqlserver", DbType::SqlServer),
    ("sqlite", DbType::SQLite),
    ("turso", DbType::SQLite),
    ("libsql", DbType::SQLite),
    ("rsfbclient", DbType::FirebirdSql),
    ("firebird", DbType::FirebirdSql),
    ("clickhouse", DbType::ClickHouse),
    ("db2", DbType::Db2),
    // DuckDB、ODBC/ADBC 在 Java 1.2.28 DbType 中没有独立枚举；识别为
    // Other，避免伪造 Java enum variant 或错误套用其他 SQL 方言。
    ("duckdb", DbType::Other),
    ("odbc", DbType::Other),
    ("adbc", DbType::Other),
    ("h2", DbType::H2),
    ("rbdc", DbType::Other),
    ("toasty", DbType::Other),
];
