use super::{
    database_profile_record::DatabaseProfileRecord, DatabaseProfileId, DriverCapabilities,
    DriverRegistryError, DriverRuntimeMode, DriverSupportStatus, DriverVerificationEvidence,
    ProtocolFamily, WallMode,
};
use druid_core::sql::DbType;

/// 经过清单校验的数据库产品档案。
#[derive(Debug, Clone)]
pub struct DatabaseProfile {
    id: DatabaseProfileId,
    display_name: String,
    db_type: DbType,
    protocol_family: ProtocolFamily,
    runtime_mode: DriverRuntimeMode,
    provider_id: String,
    artifact_id: String,
    artifact_version: Option<String>,
    driver_class: Option<String>,
    default_port: Option<u16>,
    support_status: DriverSupportStatus,
    wall_mode: WallMode,
    delivery_phase: u8,
    validation_query: Option<String>,
    reset_sql: Option<String>,
    exception_sorter: String,
    evidence: Option<DriverVerificationEvidence>,
    capabilities: DriverCapabilities,
}

impl DatabaseProfile {
    pub(crate) fn from_record(record: DatabaseProfileRecord) -> Result<Self, DriverRegistryError> {
        let id = DatabaseProfileId::new(record.profile_id)?;
        let db_type = DbType::of(&record.db_type).ok_or_else(|| {
            DriverRegistryError::InvalidManifest(format!(
                "profile '{id}' references unknown Druid DbType '{}'",
                record.db_type
            ))
        })?;
        if record.display_name.trim().is_empty()
            || record.provider_id.trim().is_empty()
            || record.artifact_id.trim().is_empty()
            || record.exception_sorter.trim().is_empty()
        {
            return Err(DriverRegistryError::InvalidManifest(format!(
                "profile '{id}' must define displayName, providerId, artifactId and exceptionSorter"
            )));
        }
        if !(1..=3).contains(&record.delivery_phase) {
            return Err(DriverRegistryError::InvalidManifest(format!(
                "profile '{id}' has invalid deliveryPhase {}",
                record.delivery_phase
            )));
        }
        // Missing capability fields deserialize to `false`. Product profiles must never inherit
        // optimistic adapter capabilities: only explicit catalog declarations backed by contract
        // evidence may describe product support.
        let capabilities = record.capabilities;
        let driver_class = record
            .driver_class
            .or_else(|| jdbc_driver_class(id.as_str()).map(str::to_owned));
        Ok(Self {
            id,
            display_name: record.display_name,
            db_type,
            protocol_family: record.protocol_family,
            runtime_mode: record.runtime_mode,
            provider_id: record.provider_id,
            artifact_id: record.artifact_id,
            artifact_version: record.artifact_version,
            driver_class,
            default_port: record.default_port,
            support_status: record.support_status,
            wall_mode: record.wall_mode,
            delivery_phase: record.delivery_phase,
            validation_query: record.validation_query,
            reset_sql: record.reset_sql,
            exception_sorter: record.exception_sorter,
            evidence: record.evidence,
            capabilities,
        })
    }

    #[must_use]
    pub fn id(&self) -> &DatabaseProfileId {
        &self.id
    }
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    #[must_use]
    pub const fn db_type(&self) -> DbType {
        self.db_type
    }
    #[must_use]
    pub const fn protocol_family(&self) -> ProtocolFamily {
        self.protocol_family
    }
    #[must_use]
    pub const fn runtime_mode(&self) -> DriverRuntimeMode {
        self.runtime_mode
    }
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }
    #[must_use]
    pub fn artifact_version(&self) -> Option<&str> {
        self.artifact_version.as_deref()
    }
    #[must_use]
    pub fn driver_class(&self) -> Option<&str> {
        self.driver_class.as_deref()
    }
    #[must_use]
    pub const fn default_port(&self) -> Option<u16> {
        self.default_port
    }
    #[must_use]
    pub const fn support_status(&self) -> DriverSupportStatus {
        self.support_status
    }
    #[must_use]
    pub const fn wall_mode(&self) -> WallMode {
        self.wall_mode
    }
    #[must_use]
    pub const fn delivery_phase(&self) -> u8 {
        self.delivery_phase
    }
    #[must_use]
    pub fn validation_query(&self) -> Option<&str> {
        self.validation_query.as_deref()
    }
    #[must_use]
    pub fn reset_sql(&self) -> Option<&str> {
        self.reset_sql.as_deref()
    }
    #[must_use]
    pub fn exception_sorter(&self) -> &str {
        &self.exception_sorter
    }
    #[must_use]
    pub fn evidence(&self) -> Option<&DriverVerificationEvidence> {
        self.evidence.as_ref()
    }
    #[must_use]
    pub const fn capabilities(&self) -> DriverCapabilities {
        self.capabilities
    }
}

/// JDBC Agent 产品的默认 `DriverManager` 类名；用户导入的 bundle 仍可通过
/// `META-INF/services/java.sql.Driver` 自注册。本表用于诊断、审计与旧驱动兼容。
fn jdbc_driver_class(profile_id: &str) -> Option<&'static str> {
    Some(match profile_id {
        "h2" => "org.h2.Driver",
        "hsqldb" => "org.hsqldb.jdbc.JDBCDriver",
        "access" => "net.ucanaccess.jdbc.UcanaccessDriver",
        "derby" => "org.apache.derby.jdbc.EmbeddedDriver",
        "firebird" => "org.firebirdsql.jdbc.FBDriver",
        "oracle" => "oracle.jdbc.OracleDriver",
        "oceanbase-oracle" => "com.oceanbase.jdbc.Driver",
        "dameng" => "dm.jdbc.driver.DmDriver",
        "yashandb" => "com.yashandb.jdbc.Driver",
        "gbase8a" => "com.gbase.jdbc.Driver",
        "gbase8s" => "com.gbasedbt.jdbc.Driver",
        "xugudb" => "com.xugu.cloudjdbc.Driver",
        "oscar" => "com.oscar.Driver",
        "sundb" => "sunje.goldilocks.jdbc.GoldilocksDriver",
        "iris" => "com.intersystems.jdbc.IRISDriver",
        "sap-hana" => "com.sap.db.jdbc.Driver",
        "db2" => "com.ibm.db2.jcc.DB2Driver",
        "informix" => "com.informix.jdbc.IfxDriver",
        "db2-for-i" => "com.ibm.as400.access.AS400JDBCDriver",
        "sap-maxdb" => "com.sap.dbtech.jdbc.DriverSapDB",
        "sqlserver" | "azure-sql" | "azure-synapse" => {
            "com.microsoft.sqlserver.jdbc.SQLServerDriver"
        }
        "sybase-ase" => "com.sybase.jdbc4.jdbc.SybDriver",
        "clickhouse" => "com.clickhouse.jdbc.ClickHouseDriver",
        "databend" => "com.databend.jdbc.DatabendDriver",
        "databricks-sql" => "com.databricks.client.jdbc.Driver",
        "snowflake" => "net.snowflake.client.jdbc.SnowflakeDriver",
        "teradata" => "com.teradata.jdbc.TeraDriver",
        "vertica" => "com.vertica.jdbc.Driver",
        "exasol" => "com.exasol.jdbc.EXADriver",
        "trino" => "io.trino.jdbc.TrinoDriver",
        "prestosql" => "com.facebook.presto.jdbc.PrestoDriver",
        "hive" | "spark-sql" => "org.apache.hive.jdbc.HiveDriver",
        "bigquery" => "com.simba.googlebigquery.jdbc.Driver",
        "kylin" => "org.apache.kylin.jdbc.Driver",
        "phoenix" => "org.apache.phoenix.jdbc.PhoenixDriver",
        "impala" => "com.cloudera.impala.jdbc.Driver",
        "athena" => "com.simba.athena.jdbc.Driver",
        "maxcompute" => "com.aliyun.odps.jdbc.OdpsDriver",
        "tdengine" => "com.taosdata.jdbc.TSDBDriver",
        _ => return None,
    })
}
