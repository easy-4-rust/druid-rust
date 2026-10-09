# Task8 canonical 外部编译核验

当前结论：FAIL；本次为全69语义行的canonical编译诊断，不构成final验收或生产行为认证。

实际 `cargo check -p druid-rdbc --all-features --locked --offline --message-format=json` 原session4987 exit0/12.70s，取该命令明确返回的rmeta；随后使用同一Rust1.97.1对每行生成独立外部消费者。全部68个Rust目标实际执行rustc，37PASS/31FAIL；SQLPermission的host目标单独标记NOT_RUST_TARGET，未当作PASS。

TRAIT/CALLBACK使用真实泛型trait bound，DTO使用真实参数类型；ENUM_KIND/MODULE使用公开import，仅证明名字可解析，不认证底层精确enum/module形态。

| Java对象 | 当前形态 | canonical | 编译结果 |
| --- | --- | --- | --- |
| java.sql.Array | TRAIT | `druid_rdbc::resource::Array` | FAIL / E0404 |
| java.sql.BatchUpdateException | ENUM_KIND | `druid_rdbc::error::BatchUpdateException` | FAIL / E0432 |
| java.sql.Blob | TRAIT | `druid_rdbc::resource::Blob` | FAIL / E0404 |
| java.sql.CallableStatement | TRAIT | `druid_rdbc::statement::CallableStatement` | PASS |
| java.sql.ClientInfoStatus | ENUM_KIND | `druid_rdbc::connection::ClientInfoStatus` | FAIL / E0432 |
| java.sql.Clob | TRAIT | `druid_rdbc::resource::Clob` | FAIL / E0404 |
| java.sql.Connection | TRAIT | `druid_rdbc::connection::Connection` | PASS |
| java.sql.DataTruncation | ENUM_KIND | `druid_rdbc::error::DataTruncation` | FAIL / E0432 |
| java.sql.DatabaseMetaData | TRAIT | `druid_rdbc::metadata::DatabaseMetaData` | PASS |
| java.sql.Date | DTO | `druid_rdbc::types::Date` | PASS |
| java.sql.Driver | TRAIT | `druid_rdbc::driver::Driver` | PASS |
| java.sql.DriverAction | CALLBACK | `druid_rdbc::driver::DriverAction` | FAIL / E0405 |
| java.sql.DriverManager | MODULE | `druid_rdbc::driver::DriverRegistry` | PASS |
| java.sql.DriverPropertyInfo | DTO | `druid_rdbc::driver::DriverPropertyInfo` | PASS |
| java.sql.JDBCType | ENUM_KIND | `druid_rdbc::types::JdbcType` | FAIL / E0432 |
| java.sql.NClob | TRAIT | `druid_rdbc::resource::NClob` | FAIL / E0404 |
| java.sql.ParameterMetaData | TRAIT | `druid_rdbc::metadata::ParameterMetaData` | PASS |
| java.sql.PreparedStatement | TRAIT | `druid_rdbc::statement::PreparedStatement` | PASS |
| java.sql.PseudoColumnUsage | ENUM_KIND | `druid_rdbc::metadata::PseudoColumnUsage` | FAIL / E0432 |
| java.sql.Ref | TRAIT | `druid_rdbc::resource::Ref` | FAIL / E0404 |
| java.sql.ResultSet | TRAIT | `druid_rdbc::result_set::ResultSet` | PASS |
| java.sql.ResultSetMetaData | TRAIT | `druid_rdbc::metadata::ResultSetMetaData` | PASS |
| java.sql.RowId | DTO | `druid_rdbc::types::RowId` | FAIL / E0425 |
| java.sql.RowIdLifetime | ENUM_KIND | `druid_rdbc::metadata::RowIdLifetime` | PASS |
| java.sql.SQLClientInfoException | ENUM_KIND | `druid_rdbc::error::SqlClientInfoException` | FAIL / E0432 |
| java.sql.SQLData | TRAIT | `druid_rdbc::resource::SqlData` | FAIL / E0405 |
| java.sql.SQLDataException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::Data` | PASS |
| java.sql.SQLException | ENUM_KIND | `druid_rdbc::error::SqlException` | PASS |
| java.sql.SQLFeatureNotSupportedException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::FeatureNotSupported` | PASS |
| java.sql.SQLInput | TRAIT | `druid_rdbc::resource::SqlInput` | FAIL / E0405 |
| java.sql.SQLIntegrityConstraintViolationException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::IntegrityConstraintViolation` | PASS |
| java.sql.SQLInvalidAuthorizationSpecException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::InvalidAuthorizationSpec` | FAIL / E0432 |
| java.sql.SQLNonTransientConnectionException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::NonTransientConnection` | PASS |
| java.sql.SQLNonTransientException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::NonTransient` | PASS |
| java.sql.SQLOutput | TRAIT | `druid_rdbc::resource::SqlOutput` | FAIL / E0405 |
| java.sql.SQLPermission | EXCLUDED_RUNTIME | `host::process_security_policy` | NOT_RUST_TARGET |
| java.sql.SQLRecoverableException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::Recoverable` | PASS |
| java.sql.SQLSyntaxErrorException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::Syntax` | PASS |
| java.sql.SQLTimeoutException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::Timeout` | PASS |
| java.sql.SQLTransactionRollbackException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::TransactionRollback` | PASS |
| java.sql.SQLTransientConnectionException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::TransientConnection` | PASS |
| java.sql.SQLTransientException | ENUM_KIND | `druid_rdbc::error::SqlExceptionKind::Transient` | PASS |
| java.sql.SQLType | TRAIT | `druid_rdbc::types::SqlType` | PASS |
| java.sql.SQLWarning | ENUM_KIND | `druid_rdbc::error::SqlWarning` | PASS |
| java.sql.SQLXML | TRAIT | `druid_rdbc::resource::SqlXml` | FAIL / E0404 |
| java.sql.Savepoint | DTO | `druid_rdbc::connection::Savepoint` | PASS |
| java.sql.Statement | TRAIT | `druid_rdbc::statement::Statement` | PASS |
| java.sql.Struct | TRAIT | `druid_rdbc::resource::Struct` | FAIL / E0405 |
| java.sql.Time | DTO | `druid_rdbc::types::Time` | PASS |
| java.sql.Timestamp | DTO | `druid_rdbc::types::Timestamp` | PASS |
| java.sql.Types | MODULE | `druid_rdbc::types::types` | FAIL / E0603 |
| java.sql.Wrapper | TRAIT | `druid_rdbc::wrapper::Wrapper` | PASS |
| javax.sql.CommonDataSource | TRAIT | `druid_rdbc::data_source::CommonDataSource` | PASS |
| javax.sql.ConnectionEvent | DTO | `druid_rdbc::data_source::ConnectionEvent` | FAIL / E0425 |
| javax.sql.ConnectionEventListener | CALLBACK | `druid_rdbc::data_source::ConnectionEventListener` | FAIL / E0405 |
| javax.sql.ConnectionPoolDataSource | TRAIT | `druid_rdbc::data_source::ConnectionPoolDataSource` | PASS |
| javax.sql.DataSource | TRAIT | `druid_rdbc::data_source::DataSource` | PASS |
| javax.sql.PooledConnection | TRAIT | `druid_rdbc::data_source::PooledConnection` | PASS |
| javax.sql.RowSet | TRAIT | `druid_rdbc::extensions::RowSet` | FAIL / E0405 |
| javax.sql.RowSetEvent | DTO | `druid_rdbc::extensions::RowSetEvent` | FAIL / E0425 |
| javax.sql.RowSetInternal | TRAIT | `druid_rdbc::extensions::RowSetInternal` | FAIL / E0405 |
| javax.sql.RowSetListener | CALLBACK | `druid_rdbc::extensions::RowSetListener` | FAIL / E0405 |
| javax.sql.RowSetMetaData | TRAIT | `druid_rdbc::extensions::RowSetMetaData` | FAIL / E0405 |
| javax.sql.RowSetReader | CALLBACK | `druid_rdbc::extensions::RowSetReader` | FAIL / E0405 |
| javax.sql.RowSetWriter | CALLBACK | `druid_rdbc::extensions::RowSetWriter` | FAIL / E0405 |
| javax.sql.StatementEvent | DTO | `druid_rdbc::data_source::StatementEvent` | FAIL / E0425 |
| javax.sql.StatementEventListener | CALLBACK | `druid_rdbc::data_source::StatementEventListener` | FAIL / E0405 |
| javax.sql.XAConnection | TRAIT | `druid_rdbc::data_source::XaConnection` | PASS |
| javax.sql.XADataSource | TRAIT | `druid_rdbc::data_source::XaDataSource` | PASS |

失败范围包括6个资源句柄别名被声明为TRAIT（E0404）、24个名字/路径无法解析（E0405/E0425/E0432）及私有types模块（E0603）。这些结果支持修正真实公开对象或正式语义映射；不能只改LIVE状态或改摘要。

[完整命令、每行源码、错误输出和SHA](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes1.summary.json)。原Cargo日志、rmeta副本和68份探针源码保留在F证据目录。

1092源码与doc-link-repair冻结逐项无变化；此前两个Python控制脚本变化已单独记录，oracle/inventory未修改。Task6 FIX_REQUIRED、Task8 INCOMPLETE、18顶层任务继续OPEN。
