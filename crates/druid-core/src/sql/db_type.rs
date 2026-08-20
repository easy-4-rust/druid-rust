#![allow(clippy::match_same_arms)]
/// Druid 支持的数据库类型。
///
/// 对应 Java：`com.alibaba.druid.DbType`。Rust variant 使用 `PascalCase`，
/// [`Self::as_str`] 保留 Java enum 的精确外部名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DbType {
    Other,
    Jtds,
    Hsql,
    Db2,
    PostgreSql,
    SqlServer,
    Oracle,
    MySql,
    MariaDb,
    Derby,
    Hive,
    H2,
    Dm,
    Kingbase,
    Gbase,
    OceanBase,
    Informix,
    Odps,
    Teradata,
    Phoenix,
    Edb,
    Kylin,
    SQLite,
    Ads,
    Presto,
    ElasticSearch,
    Hbase,
    Drds,
    ClickHouse,
    Blink,
    /// 对应 Java 已废弃的 `antspark`；与 `spark` 共享 mask。
    AntSpark,
    Spark,
    OceanBaseOracle,
    PolarDb,
    AliOracle,
    Mock,
    Sybase,
    HighGo,
    Greenplum,
    GaussDb,
    Trino,
    Oscar,
    TiDb,
    TyDb,
    StarRocks,
    GoldenDb,
    Snowflake,
    Redshift,
    Hologres,
    BigQuery,
    Impala,
    Doris,
    Lealone,
    Athena,
    PolarDbX,
    SuperSql,
    Databricks,
    AdbMySql,
    PolarDb2,
    Synapse,
    Ingres,
    Cloudscape,
    TimesTen,
    As400,
    SapDb,
    Kdb,
    Xugu,
    FirebirdSql,
    JsqlConnect,
    JTurbo,
    Interbase,
    Pointbase,
    Edbc,
    Mimer,
    TaosData,
    SunDb,
}

impl DbType {
    /// 按 Java `DbType.of(String)` 的规则解析。
    ///
    /// 对应 Java：`DbType#of(String)`。除 `aliyun_ads` 和 `maxcompute`
    /// 两个兼容别名忽略大小写外，枚举名称严格区分大小写，也不会修剪空白。
    /// Java 专属的 log4rdbc 包装驱动不形成 Rust 数据库类型，因此
    /// `log4rdbc` 返回 `None`；旧 RDBC URL 仍由 `RdbcUtils` 归一化到真实
    /// vendor。
    #[must_use]
    pub fn of(name: &str) -> Option<Self> {
        if name.is_empty() {
            return None;
        }
        if name.eq_ignore_ascii_case("aliyun_ads") {
            return Some(Self::Ads);
        }
        if name.eq_ignore_ascii_case("maxcompute") {
            return Some(Self::Odps);
        }
        Some(match name {
            "other" => Self::Other,
            "jtds" => Self::Jtds,
            "hsql" => Self::Hsql,
            "db2" => Self::Db2,
            "postgresql" => Self::PostgreSql,
            "sqlserver" => Self::SqlServer,
            "oracle" => Self::Oracle,
            "mysql" => Self::MySql,
            "mariadb" => Self::MariaDb,
            "derby" => Self::Derby,
            "hive" => Self::Hive,
            "h2" => Self::H2,
            "dm" => Self::Dm,
            "kingbase" => Self::Kingbase,
            "gbase" => Self::Gbase,
            "oceanbase" => Self::OceanBase,
            "informix" => Self::Informix,
            "odps" => Self::Odps,
            "teradata" => Self::Teradata,
            "phoenix" => Self::Phoenix,
            "edb" => Self::Edb,
            "kylin" => Self::Kylin,
            "sqlite" => Self::SQLite,
            "ads" => Self::Ads,
            "presto" => Self::Presto,
            "elastic_search" => Self::ElasticSearch,
            "hbase" => Self::Hbase,
            "drds" => Self::Drds,
            "clickhouse" => Self::ClickHouse,
            "blink" => Self::Blink,
            "antspark" => Self::AntSpark,
            "spark" => Self::Spark,
            "oceanbase_oracle" => Self::OceanBaseOracle,
            "polardb" => Self::PolarDb,
            "ali_oracle" => Self::AliOracle,
            "mock" => Self::Mock,
            "sybase" => Self::Sybase,
            "highgo" => Self::HighGo,
            "greenplum" => Self::Greenplum,
            "gaussdb" => Self::GaussDb,
            "trino" => Self::Trino,
            "oscar" => Self::Oscar,
            "tidb" => Self::TiDb,
            "tydb" => Self::TyDb,
            "starrocks" => Self::StarRocks,
            "goldendb" => Self::GoldenDb,
            "snowflake" => Self::Snowflake,
            "redshift" => Self::Redshift,
            "hologres" => Self::Hologres,
            "bigquery" => Self::BigQuery,
            "impala" => Self::Impala,
            "doris" => Self::Doris,
            "lealone" => Self::Lealone,
            "athena" => Self::Athena,
            "polardbx" => Self::PolarDbX,
            "supersql" => Self::SuperSql,
            "databricks" => Self::Databricks,
            "adb_mysql" => Self::AdbMySql,
            "polardb2" => Self::PolarDb2,
            "synapse" => Self::Synapse,
            "ingres" => Self::Ingres,
            "cloudscape" => Self::Cloudscape,
            "timesten" => Self::TimesTen,
            "as400" => Self::As400,
            "sapdb" => Self::SapDb,
            "kdb" => Self::Kdb,
            "xugu" => Self::Xugu,
            "firebirdsql" => Self::FirebirdSql,
            "JSQLConnect" => Self::JsqlConnect,
            "JTurbo" => Self::JTurbo,
            "interbase" => Self::Interbase,
            "pointbase" => Self::Pointbase,
            "edbc" => Self::Edbc,
            "mimer" => Self::Mimer,
            "taosdata" => Self::TaosData,
            "sundb" => Self::SunDb,
            _ => return None,
        })
    }

    /// 返回 Java enum 的精确名称。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Other => "other",
            Self::Jtds => "jtds",
            Self::Hsql => "hsql",
            Self::Db2 => "db2",
            Self::PostgreSql => "postgresql",
            Self::SqlServer => "sqlserver",
            Self::Oracle => "oracle",
            Self::MySql => "mysql",
            Self::MariaDb => "mariadb",
            Self::Derby => "derby",
            Self::Hive => "hive",
            Self::H2 => "h2",
            Self::Dm => "dm",
            Self::Kingbase => "kingbase",
            Self::Gbase => "gbase",
            Self::OceanBase => "oceanbase",
            Self::Informix => "informix",
            Self::Odps => "odps",
            Self::Teradata => "teradata",
            Self::Phoenix => "phoenix",
            Self::Edb => "edb",
            Self::Kylin => "kylin",
            Self::SQLite => "sqlite",
            Self::Ads => "ads",
            Self::Presto => "presto",
            Self::ElasticSearch => "elastic_search",
            Self::Hbase => "hbase",
            Self::Drds => "drds",
            Self::ClickHouse => "clickhouse",
            Self::Blink => "blink",
            Self::AntSpark => "antspark",
            Self::Spark => "spark",
            Self::OceanBaseOracle => "oceanbase_oracle",
            Self::PolarDb => "polardb",
            Self::AliOracle => "ali_oracle",
            Self::Mock => "mock",
            Self::Sybase => "sybase",
            Self::HighGo => "highgo",
            Self::Greenplum => "greenplum",
            Self::GaussDb => "gaussdb",
            Self::Trino => "trino",
            Self::Oscar => "oscar",
            Self::TiDb => "tidb",
            Self::TyDb => "tydb",
            Self::StarRocks => "starrocks",
            Self::GoldenDb => "goldendb",
            Self::Snowflake => "snowflake",
            Self::Redshift => "redshift",
            Self::Hologres => "hologres",
            Self::BigQuery => "bigquery",
            Self::Impala => "impala",
            Self::Doris => "doris",
            Self::Lealone => "lealone",
            Self::Athena => "athena",
            Self::PolarDbX => "polardbx",
            Self::SuperSql => "supersql",
            Self::Databricks => "databricks",
            Self::AdbMySql => "adb_mysql",
            Self::PolarDb2 => "polardb2",
            Self::Synapse => "synapse",
            Self::Ingres => "ingres",
            Self::Cloudscape => "cloudscape",
            Self::TimesTen => "timesten",
            Self::As400 => "as400",
            Self::SapDb => "sapdb",
            Self::Kdb => "kdb",
            Self::Xugu => "xugu",
            Self::FirebirdSql => "firebirdsql",
            Self::JsqlConnect => "JSQLConnect",
            Self::JTurbo => "JTurbo",
            Self::Interbase => "interbase",
            Self::Pointbase => "pointbase",
            Self::Edbc => "edbc",
            Self::Mimer => "mimer",
            Self::TaosData => "taosdata",
            Self::SunDb => "sundb",
        }
    }

    /// 返回 Java `mask`。
    #[must_use]
    pub const fn mask(self) -> u64 {
        let bit = match self {
            Self::Other => Some(0),
            Self::Jtds => Some(1),
            Self::Hsql => Some(2),
            Self::Db2 => Some(3),
            Self::PostgreSql => Some(4),
            Self::SqlServer => Some(5),
            Self::Oracle => Some(6),
            Self::MySql => Some(7),
            Self::MariaDb => Some(8),
            Self::Derby => Some(9),
            Self::Hive => Some(10),
            Self::H2 => Some(11),
            Self::Dm => Some(12),
            Self::Kingbase => Some(13),
            Self::Gbase => Some(14),
            Self::OceanBase => Some(15),
            Self::Informix => Some(16),
            Self::Odps => Some(17),
            Self::Teradata => Some(18),
            Self::Phoenix => Some(19),
            Self::Edb => Some(20),
            Self::Kylin => Some(21),
            Self::SQLite => Some(22),
            Self::Ads => Some(23),
            Self::Presto => Some(24),
            Self::ElasticSearch => Some(25),
            Self::Hbase => Some(26),
            Self::Drds => Some(27),
            Self::ClickHouse => Some(28),
            Self::Blink => Some(29),
            Self::AntSpark => Some(30),
            Self::Spark => Some(30),
            Self::OceanBaseOracle => Some(31),
            Self::PolarDb => Some(32),
            Self::AliOracle => Some(33),
            Self::Mock => Some(34),
            Self::Sybase => Some(35),
            Self::HighGo => Some(36),
            Self::Greenplum => Some(37),
            Self::GaussDb => Some(38),
            Self::Trino => Some(39),
            Self::Oscar => Some(40),
            Self::TiDb => Some(41),
            Self::TyDb => Some(42),
            Self::StarRocks => Some(43),
            Self::GoldenDb => Some(44),
            Self::Snowflake => Some(45),
            Self::Redshift => Some(46),
            Self::Hologres => Some(47),
            Self::BigQuery => Some(48),
            Self::Impala => Some(49),
            Self::Doris => Some(50),
            Self::Lealone => Some(51),
            Self::Athena => Some(52),
            Self::PolarDbX => Some(53),
            Self::SuperSql => Some(54),
            Self::Databricks => Some(55),
            Self::AdbMySql => Some(56),
            Self::PolarDb2 => Some(57),
            Self::Synapse => Some(58),
            _ => None,
        };
        match bit {
            Some(bit) => 1_u64 << bit,
            None => 0,
        }
    }

    /// 返回是否属于 `PostgreSQL` 风格。
    #[must_use]
    pub const fn is_postgresql_style(self) -> bool {
        matches!(
            self,
            Self::PostgreSql | Self::Edb | Self::Greenplum | Self::Hologres
        )
    }

    /// 返回 Java 构造器保存的 `hashCode64`。
    ///
    /// 对应 Java：`new DbType(mask)` 中的
    /// `FnvHash.hashCode64(name())`，按 UTF-16 code unit 对 ASCII 大写
    /// 归一化后执行 FNV-1a 64 位运算。当前枚举外部名称均为 ASCII。
    #[must_use]
    pub fn hash_code_64(self) -> u64 {
        const BASIC: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        self.as_str().bytes().fold(BASIC, |hash, byte| {
            let normalized = if byte.is_ascii_uppercase() {
                byte.to_ascii_lowercase()
            } else {
                byte
            };
            (hash ^ u64::from(normalized)).wrapping_mul(PRIME)
        })
    }

    /// 合并多个数据库类型的 Java mask。
    ///
    /// 对应 Java：`DbType.of(DbType...)`。
    #[must_use]
    pub fn mask_of(types: impl IntoIterator<Item = Self>) -> u64 {
        types
            .into_iter()
            .fold(0_u64, |mask, db_type| mask | db_type.mask())
    }

    /// 判断字符串是否按 Java `equals(String)` 解析为当前类型。
    #[must_use]
    pub fn equals_name(self, other: &str) -> bool {
        Self::of(other) == Some(self)
    }
}

impl std::fmt::Display for DbType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
