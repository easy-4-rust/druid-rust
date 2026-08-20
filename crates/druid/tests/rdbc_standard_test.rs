use std::sync::Arc;

use druid::core::{Driver, DruidError, PhysicalConnection, SqlException, Value};
use druid::sql::{
    Date, DriverManager, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcString, RdbcType,
    RdbcUrl, RdbcWriter, SQLType, SqlExceptionKind, SqlInput, SqlOutput, Time, Timestamp, Types,
};

struct ProbeDriver;

#[test]
fn rdbc_facade_exports_only_rdbc_named_string_and_stream_resources() {
    let value = RdbcString::from_utf16([0x0041, 0xD800]);
    assert_eq!(value.as_utf16(), &[0x0041, 0xD800]);

    let input = RdbcInputStream::new(std::io::Cursor::new(vec![1_u8, 2, 3]));
    assert_eq!(
        input.read_to_end().expect("RDBC input must be readable"),
        [1, 2, 3]
    );

    let reader = RdbcReader::from_utf16(vec![0x0041, 0xD800]);
    assert_eq!(
        reader
            .read_to_end_utf16()
            .expect("RDBC reader must preserve UTF-16"),
        [0x0041, 0xD800]
    );

    // Compile-time checks for the remaining public RDBC resource handles.
    let _: Option<RdbcOutputStream> = None;
    let _: Option<RdbcWriter> = None;
}

#[async_trait::async_trait]
impl Driver for ProbeDriver {
    fn name(&self) -> &str {
        "rdbc-probe"
    }

    fn accepts_url(&self, url: &str) -> bool {
        url.starts_with("rdbc-probe:")
    }

    async fn connect(&self, _url: &str) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Err(DruidError::DriverError("probe reached".to_owned()))
    }
}

#[tokio::test]
async fn driver_manager_uses_accepts_url_and_supports_deregister() {
    let driver: Arc<dyn Driver> = Arc::new(ProbeDriver);
    DriverManager::register_driver(Arc::clone(&driver));
    assert!(matches!(
        DriverManager::get_connection("rdbc-probe:test").await,
        Err(DruidError::DriverError(message)) if message == "probe reached"
    ));
    assert!(DriverManager::deregister_driver(&driver));
    assert!(matches!(
        DriverManager::get_connection("rdbc-probe:test").await,
        Err(DruidError::DriverError(message)) if message.starts_with("No suitable driver")
    ));
}

#[test]
fn rdbc_42_type_numbers_and_udt_streams_are_lossless() {
    assert_eq!(Types::TIMESTAMP_WITH_TIMEZONE, 2014);
    assert_eq!(RdbcType::Decimal.vendor_type_number(), Types::DECIMAL);
    assert_eq!(RdbcType::SqlXml.name(), "SQLXML");

    let values = vec![Value::Null, Value::Int(7), Value::Bytes(vec![0, 255])];
    let mut input = SqlInput::new(values.clone());
    assert_eq!(input.read_value().expect("NULL 必须可读"), Value::Null);
    assert!(input.was_null());
    assert_eq!(input.read_value().expect("整数必须可读"), Value::Int(7));
    assert!(!input.was_null());

    let mut output = SqlOutput::new();
    for value in values.clone() {
        output.write_value(value);
    }
    assert_eq!(output.into_values(), values);
}

#[test]
fn sql_exception_keeps_sql_state_vendor_code_cause_and_next_chain() {
    let mut exception = SqlException::new(
        1064,
        Some("42000".to_owned()),
        Some("syntax error".to_owned()),
    );
    exception.set_next_exception(SqlException::new(
        0,
        Some("01000".to_owned()),
        Some("detail".to_owned()),
    ));
    assert_eq!(exception.error_code(), 1064);
    assert_eq!(exception.sql_state(), Some("42000"));
    assert_eq!(exception.kind(), SqlExceptionKind::Syntax);
    assert_eq!(
        exception.next_exception().and_then(SqlException::message),
        Some("detail")
    );
}

#[test]
fn rdbc_date_time_and_timestamp_follow_escape_formats_and_nanos() {
    let date = Date::value_of("2026-08-07").expect("RDBC DATE escape 必须可解析");
    assert_eq!(date.to_string(), "2026-08-07");
    let time = Time::value_of("13:14:15").expect("RDBC TIME escape 必须可解析");
    assert_eq!(time.to_string(), "13:14:15");
    let mut timestamp = Timestamp::value_of("2026-08-07 13:14:15.123456789")
        .expect("RDBC TIMESTAMP escape 必须保留纳秒");
    assert_eq!(timestamp.nanos(), 123_456_789);
    timestamp.set_nanos(7).expect("合法 nanos 必须可设置");
    assert_eq!(timestamp.nanos(), 7);
    assert!(timestamp.set_nanos(1_000_000_000).is_err());
}

#[test]
fn savepoint_named_and_unnamed_access_rules_match_rdbc() {
    let unnamed = druid::sql::Savepoint { id: 9, name: None };
    assert_eq!(unnamed.get_savepoint_id().expect("匿名保存点必须有 ID"), 9);
    assert!(unnamed.get_savepoint_name().is_err());
    let named = druid::sql::Savepoint {
        id: 10,
        name: Some("before_update".to_owned()),
    };
    assert_eq!(
        named.get_savepoint_name().expect("命名保存点必须有名称"),
        "before_update"
    );
    assert!(named.get_savepoint_id().is_err());
}

#[test]
fn unified_rdbc_url_separates_profile_endpoint_database_and_properties() {
    let url = RdbcUrl::parse(
        "rdbc://postgresql/localhost:5432/app/main?user=druid&password=secret&sslmode=require",
    )
    .expect("统一 RDBC URL 必须可解析");
    assert_eq!(url.profile(), "postgresql");
    assert_eq!(url.endpoint(), "localhost:5432");
    assert_eq!(url.database(), "app/main");
    assert_eq!(url.property("user"), Some("druid"));
    assert_eq!(url.property("password"), Some("secret"));
    assert_eq!(
        url.network_url("postgresql").expect("必须生成真实 URL"),
        "postgresql://localhost:5432/app/main"
    );
}

#[test]
fn java_style_mysql_rdbc_url_preserves_driver_properties() {
    let url = RdbcUrl::parse(
        "rdbc:mysql://cloud-mysql:13306/qumall_mall?characterEncoding=utf8&zeroDateTimeBehavior=convertToNull&useSSL=false&useJDBCCompliantTimezoneShift=true&useLegacyDatetimeCode=false&serverTimezone=GMT%2B8&allowMultiQueries=true&allowPublicKeyRetrieval=true",
    )
    .expect("Java 风格的 MySQL RDBC URL 必须可解析");

    assert_eq!(url.profile(), "mysql");
    assert_eq!(url.endpoint(), "cloud-mysql:13306");
    assert_eq!(url.database(), "qumall_mall");
    assert_eq!(url.property("characterEncoding"), Some("utf8"));
    assert_eq!(url.property("zeroDateTimeBehavior"), Some("convertToNull"));
    assert_eq!(url.property("useSSL"), Some("false"));
    assert_eq!(url.property("useJDBCCompliantTimezoneShift"), Some("true"));
    assert_eq!(url.property("useLegacyDatetimeCode"), Some("false"));
    assert_eq!(url.property("serverTimezone"), Some("GMT+8"));
    assert_eq!(url.property("allowMultiQueries"), Some("true"));
    assert_eq!(url.property("allowPublicKeyRetrieval"), Some("true"));
    assert_eq!(url.redacted(), "rdbc:mysql://cloud-mysql:13306/qumall_mall");
}
