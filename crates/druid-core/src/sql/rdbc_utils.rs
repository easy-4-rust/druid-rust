use std::borrow::Cow;

use super::DbType;

/// RDBC 平台工具门面。
///
/// 对应 Java：`com.alibaba.druid.util.RdbcUtils`。Rust 不提供 RDBC close
/// helpers（RAII/Drop 承担），但完整保留 Druid 依赖的 URL/driver 数据库识别。
#[derive(Debug, Default, Clone, Copy)]
pub struct RdbcUtils;

impl RdbcUtils {
    /// 按 Java `RdbcUtils#getDbTypeRaw` 识别 RDBC URL。
    ///
    /// Java 源实现虽然接收 `driverClassName`，但当前版本并未读取它；Rust
    /// 保留该参数和精确的大小写、前缀顺序。JVM 专属 log4rdbc 包装驱动的
    /// catch-all 类型不进入 Rust `DbType`，已知 vendor 包装前缀仍返回真实
    /// 数据库类型。
    #[must_use]
    pub fn get_db_type_raw(
        raw_url: Option<&str>,
        _driver_class_name: Option<&str>,
    ) -> Option<DbType> {
        let raw_url = raw_url?;
        RDBC_URL_PREFIXES
            .iter()
            .find_map(|(prefix, db_type)| raw_url.starts_with(prefix).then_some(*db_type))
    }

    /// 按 Java `RdbcUtils#getDbType` 返回精确枚举名称。
    #[must_use]
    pub fn get_db_type(
        raw_url: Option<&str>,
        driver_class_name: Option<&str>,
    ) -> Option<&'static str> {
        Self::get_db_type_raw(raw_url, driver_class_name).map(DbType::as_str)
    }

    /// 识别 RDBC 迁移输入或 Rust 原生 DSN/驱动身份。
    ///
    /// 这是 Rust 扩展，不冒充 Java `getDbTypeRaw`：先应用 Java RDBC URL
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

    /// 将 Toasty/SQLx 能表达的 RDBC URL 转为 Rust 原生 DSN。
    ///
    /// 非 RDBC DSN 原样借用；SQLite、PostgreSQL、MySQL/MariaDB 的 RDBC
    /// 前缀被转换。其余 RDBC 驱动返回 `None`，调用方必须选择
    /// `druid-wrapper` 中相应扩展 Adapter，不能把 Java URL 直接传给 Rust
    /// driver 后等待不透明解析错误。
    #[must_use]
    pub fn to_rust_url(raw_url: &str) -> Option<Cow<'_, str>> {
        let lower = raw_url.to_ascii_lowercase();
        let mapping = [
            ("rdbc:log4rdbc:mysql:", "mysql:"),
            ("rdbc:log4rdbc:postgresql:", "postgresql:"),
            ("rdbc:sqlite:", "sqlite:"),
            ("rdbc:postgresql:", "postgresql:"),
            ("rdbc:mysql:", "mysql:"),
            ("rdbc:mariadb:", "mysql:"),
        ];
        if let Some((rdbc_prefix, rust_prefix)) = mapping
            .iter()
            .find(|(rdbc_prefix, _)| lower.starts_with(rdbc_prefix))
        {
            return Some(Cow::Owned(format!(
                "{rust_prefix}{}",
                &raw_url[rdbc_prefix.len()..]
            )));
        }
        (!lower.starts_with("rdbc:")).then_some(Cow::Borrowed(raw_url))
    }

    /// 返回 Java `java.sql.Types` 的显示名称。
    ///
    /// 对应 Java：`RdbcUtils#getTypeName(int)`。未列入源 switch 的值即使
    /// 在新版 RDBC 中有名称，也保持返回 `OTHER`。
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

    /// 返回是否属于 Java RdbcUtils 的 PostgreSQL 协议族。
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

    /// 返回是否属于 Java RdbcUtils 的 SQL Server 协议族。
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
            "com.mysql.rdbc.Driver"
                | "com.mysql.cj.rdbc.Driver"
                | "com.mysql.cj.api.MysqlConnection"
                | "com.mysql.rdbc."
        )
    }
}

/// Java RDBC 兼容输入；仅用于识别和配置迁移，不直接传给 Rust driver。
const RDBC_URL_PREFIXES: &[(&str, DbType)] = &[
    ("rdbc:derby:", DbType::Derby),
    ("rdbc:log4rdbc:derby:", DbType::Derby),
    ("rdbc:log4rdbc:mysql:", DbType::MySql),
    ("rdbc:mysql:", DbType::MySql),
    ("rdbc:cobar:", DbType::MySql),
    // Java 1.2.28 历史行为：goldendb URL 返回 mysql，而非 goldendb。
    ("rdbc:goldendb:", DbType::MySql),
    ("rdbc:mariadb:", DbType::MariaDb),
    ("rdbc:tidb:", DbType::TiDb),
    ("rdbc:log4rdbc:oracle:", DbType::Oracle),
    ("rdbc:oracle:", DbType::Oracle),
    ("rdbc:alibaba:oracle:", DbType::AliOracle),
    ("rdbc:oceanbase:oracle:", DbType::OceanBaseOracle),
    ("rdbc:oceanbase:", DbType::OceanBase),
    ("rdbc:log4rdbc:microsoft:", DbType::SqlServer),
    ("rdbc:log4rdbc:sqlserver:", DbType::SqlServer),
    ("rdbc:sqlserver:", DbType::SqlServer),
    ("rdbc:microsoft:", DbType::SqlServer),
    ("rdbc:sybase:Tds:", DbType::Sybase),
    ("rdbc:log4rdbc:sybase:", DbType::Sybase),
    ("rdbc:jtds:", DbType::Jtds),
    ("rdbc:log4rdbc:jtds:", DbType::Jtds),
    ("rdbc:fake:", DbType::Mock),
    ("rdbc:mock:", DbType::Mock),
    ("rdbc:postgresql:", DbType::PostgreSql),
    ("rdbc:log4rdbc:postgresql:", DbType::PostgreSql),
    ("rdbc:edb:", DbType::Edb),
    ("rdbc:hsqldb:", DbType::Hsql),
    ("rdbc:log4rdbc:hsqldb:", DbType::Hsql),
    ("rdbc:odps:", DbType::Odps),
    ("rdbc:db2:", DbType::Db2),
    ("rdbc:sqlite:", DbType::SQLite),
    ("rdbc:ingres:", DbType::Ingres),
    ("rdbc:h2:", DbType::H2),
    ("rdbc:log4rdbc:h2:", DbType::H2),
    ("rdbc:lealone:", DbType::Lealone),
    ("rdbc:mckoi:", DbType::Mock),
    ("rdbc:cloudscape:", DbType::Cloudscape),
    ("rdbc:informix-sqli:", DbType::Informix),
    ("rdbc:log4rdbc:informix-sqli:", DbType::Informix),
    ("rdbc:timesten:", DbType::TimesTen),
    ("rdbc:as400:", DbType::As400),
    ("rdbc:sapdb:", DbType::SapDb),
    ("rdbc:JSQLConnect:", DbType::JsqlConnect),
    ("rdbc:JTurbo:", DbType::JTurbo),
    ("rdbc:firebirdsql:", DbType::FirebirdSql),
    ("rdbc:interbase:", DbType::Interbase),
    ("rdbc:pointbase:", DbType::Pointbase),
    ("rdbc:edbc:", DbType::Edbc),
    ("rdbc:mimer:multi1:", DbType::Mimer),
    ("rdbc:dm:", DbType::Dm),
    ("rdbc:kingbase:", DbType::Kingbase),
    ("rdbc:kingbase8:", DbType::Kingbase),
    ("rdbc:gbase:", DbType::Gbase),
    ("rdbc:xugu:", DbType::Xugu),
    // Java 对未知 log4rdbc vendor 返回 JVM 专属 DbType.log4rdbc；Rust
    // 不制造该伪数据库类型，因此 catch-all 有意不进入本表。
    ("rdbc:hive:", DbType::Hive),
    ("rdbc:hive2:", DbType::Hive),
    ("rdbc:phoenix:", DbType::Phoenix),
    ("rdbc:kylin:", DbType::Kylin),
    ("rdbc:elastic:", DbType::ElasticSearch),
    ("rdbc:clickhouse:", DbType::ClickHouse),
    ("rdbc:presto:", DbType::Presto),
    ("rdbc:trino:", DbType::Trino),
    ("rdbc:inspur:", DbType::Kdb),
    ("rdbc:polardb2:", DbType::PolarDb2),
    ("rdbc:polardbx:", DbType::PolarDbX),
    ("rdbc:polardb", DbType::PolarDb),
    ("rdbc:highgo:", DbType::HighGo),
    ("rdbc:pivotal:greenplum:", DbType::Greenplum),
    ("rdbc:datadirect:greenplum:", DbType::Greenplum),
    ("rdbc:opengauss:", DbType::GaussDb),
    ("rdbc:gaussdb:", DbType::GaussDb),
    ("rdbc:dws:iam:", DbType::GaussDb),
    ("rdbc:TAOS:", DbType::TaosData),
    ("rdbc:TAOS-RS:", DbType::TaosData),
    ("rdbc:oscar:", DbType::Oscar),
    ("rdbc:sundb:", DbType::SunDb),
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
