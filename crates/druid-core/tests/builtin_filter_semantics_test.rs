//! 内置 Filter 语义差分测试（C9 覆盖率批次：core/filter 7 文件）。
//!
//! `ConfigTools`/`CharsetParameter` 在生产代码中标 deprecated 是正确防线；
//! 本测试必须覆盖它们以保留 Java Druid 旧配置兼容语义，故在测试模块放开。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 对照实现：
//! - `ConfigFilter`/`ConfigTools`（`com.alibaba.druid.filter.config.*`）：
//!   filters=config 启用、三段 decrypt 优先级、file:// 与普通路径加载、
//!   properties 覆盖、RSA 密文往返。
//! - `EncodingConvertFilter`/`CharsetConvert`/`CharsetParameter`
//!   （`com.alibaba.druid.filter.encoding.*`）：同名编码关闭转换、
//!   异名开启、encode/decode 方向、Value 直通。
//! - `MySQL8DateTimeSqlTypeFilter`/`MySQL8DateTimeResultSetMetaData`
//!   （`com.alibaba.druid.filter.mysql8datetime.*`）：LocalDateTime→Timestamp
//!   类名恢复、值恒等。
//! - `LogFilter`（`com.alibaba.druid.filter.logging.LogFilter`）：
//!   七键 configFromProperties、分类名、开关读写。

#![allow(deprecated)]

extern crate druid_core as druid;
use druid::core::filter::config::{ConfigFilter, ConfigTools};
use druid::core::filter::encoding::{CharsetConvert, CharsetParameter, EncodingConvertFilter};
use druid::core::filter::mysql8datetime::{
    MySQL8DateTimeResultSetMetaData, MySQL8DateTimeSqlTypeFilter,
};
use druid::core::log_filter::LogFilter;
use druid::core::{ResultSetColumnMeta, ResultSetColumnType, Value};
use std::collections::HashMap;

fn props(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

// ── ConfigFilter 启用与 decrypt 判定 ──────────────────────────

/// Java `ConfigFilter#isEnabled`：filters 含 `config` 或全限定类名。
#[test]
fn config_filter_is_enabled_matches_java_filters_key() {
    assert!(ConfigFilter::is_enabled(&props(&[("filters", "config")])));
    assert!(ConfigFilter::is_enabled(&props(&[(
        "filters",
        "stat,config,wall"
    )])));
    assert!(ConfigFilter::is_enabled(&props(&[(
        "filters",
        "com.alibaba.druid.filter.config.ConfigFilter"
    )])));
    // `!config` 前缀剥离后仍启用（Java ConfigFilter.isEnabled 的 strip 语义）。
    assert!(ConfigFilter::is_enabled(&props(&[("filters", "!config")])));
    assert!(!ConfigFilter::is_enabled(&props(&[(
        "filters",
        "stat,wall"
    )])));
    assert!(!ConfigFilter::is_enabled(&props(&[(
        "filters",
        "configfile"
    )])));
    assert!(!ConfigFilter::is_enabled(&HashMap::new()));
}

/// Java 三段 decrypt 优先级 + `Boolean.valueOf` 大小写规则。
#[tokio::test]
async fn config_filter_is_decrypt_precedence() {
    let filter = ConfigFilter::new();
    let empty = HashMap::new();

    // 连接属性最高。
    assert!(filter.is_decrypt(&props(&[("config.decrypt", "true")]), None, &empty));
    // 大小写不敏感（Java equalsIgnoreCase("true")）。
    assert!(filter.is_decrypt(&props(&[("config.decrypt", "TRUE")]), None, &empty));
    assert!(!filter.is_decrypt(&props(&[("config.decrypt", "false")]), None, &empty));
    assert!(!filter.is_decrypt(&props(&[("config.decrypt", "1")]), None, &empty));

    // 配置文件次之，system property 兜底。
    assert!(filter.is_decrypt(&empty, Some(&props(&[("config.decrypt", "true")])), &empty));
    assert!(filter.is_decrypt(&empty, None, &props(&[("druid.config.decrypt", "true")])));
    // 空字符串视为未设置（firstNonEmpty）。
    assert!(!filter.is_decrypt(&props(&[("config.decrypt", "")]), None, &empty));
}

// ── ConfigFilter 配置加载与属性解析 ───────────────────────────

/// Java `loadConfig`：file:// 与普通路径均支持 properties 格式。
#[tokio::test]
async fn config_filter_load_properties_from_plain_and_file_url() {
    let dir = std::env::temp_dir().join("druid-config-filter-test");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("datasource.properties");
    std::fs::write(&file_path, "url=jdbc:mock:xxx\nname=cfg-source\n").unwrap();

    let filter = ConfigFilter::new();

    let plain = filter
        .load_config(file_path.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(plain.get("url").map(String::as_str), Some("jdbc:mock:xxx"));
    assert_eq!(plain.get("name").map(String::as_str), Some("cfg-source"));

    let file_url = filter
        .load_config(&format!("file://{}", file_path.display()))
        .await
        .unwrap();
    assert_eq!(file_url.get("name").map(String::as_str), Some("cfg-source"));

    let _ = std::fs::remove_file(&file_path);
}

/// Java `loadConfig`：classpath: 前缀在显式根下解析。
#[tokio::test]
async fn config_filter_load_classpath_resource() {
    let dir = std::env::temp_dir().join("druid-config-filter-cp");
    std::fs::create_dir_all(dir.join("conf")).unwrap();
    std::fs::write(dir.join("conf").join("app.properties"), "key=cp-value\n").unwrap();

    let filter = ConfigFilter::with_runtime(reqwest::Client::new(), [dir.clone()]);
    let loaded = filter
        .load_config("classpath:conf/app.properties")
        .await
        .unwrap();
    assert_eq!(loaded.get("key").map(String::as_str), Some("cp-value"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 无 config.file 且无 decrypt：属性原样透传。
#[tokio::test]
async fn config_filter_resolve_passthrough_without_decrypt() {
    let filter = ConfigFilter::new();
    let source = props(&[
        ("url", "jdbc:mock:xxx"),
        ("username", "admin"),
        ("password", "plain-secret"),
    ]);
    let system = HashMap::new();
    let resolved = filter
        .resolve_properties_with_system(&source, &system)
        .await
        .unwrap();
    assert_eq!(
        resolved.get("password").map(String::as_str),
        Some("plain-secret")
    );
    assert_eq!(
        resolved.get("url").map(String::as_str),
        Some("jdbc:mock:xxx")
    );
}

/// config.file 属性覆盖原属性（Java config(dataSource, info) 合并语义）。
#[tokio::test]
async fn config_filter_config_file_overrides_properties() {
    let dir = std::env::temp_dir().join("druid-config-filter-override");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("override.properties");
    std::fs::write(&file_path, "password=overridden\nmaxActive=20\n").unwrap();

    let filter = ConfigFilter::with_runtime(reqwest::Client::new(), [dir.clone()]);
    let source = props(&[
        (
            "connectionProperties",
            &format!("config.file={}", file_path.display()),
        ),
        ("password", "original"),
    ]);
    let system = HashMap::new();
    let resolved = filter
        .resolve_properties_with_system(&source, &system)
        .await
        .unwrap();
    assert_eq!(
        resolved.get("password").map(String::as_str),
        Some("overridden")
    );
    assert_eq!(resolved.get("maxActive").map(String::as_str), Some("20"));

    let _ = std::fs::remove_dir_all(&dir);
}

/// decrypt=true 且无 config.file：用公钥恢复 connectionProperties.password。
#[tokio::test]
async fn config_filter_decrypts_password_without_config_file() {
    let cipher = ConfigTools::encrypt("decrypt-me").unwrap();

    let filter = ConfigFilter::new();
    let source = props(&[
        (
            "connectionProperties",
            format!("config.decrypt=true;password={cipher}").as_str(),
        ),
        ("username", "admin"),
    ]);
    let system = HashMap::new();
    let resolved = filter
        .resolve_properties_with_system(&source, &system)
        .await
        .unwrap();
    assert_eq!(
        resolved.get("password").map(String::as_str),
        Some("decrypt-me")
    );
    assert_eq!(resolved.get("username").map(String::as_str), Some("admin"));
}

// ── ConfigTools RSA 往返 ──────────────────────────────────────

/// Java 默认密钥对加解密往返。
#[test]
fn config_tools_default_key_round_trip() {
    let cipher = ConfigTools::encrypt("my-secret").unwrap();
    assert_ne!(cipher, "my-secret");
    let plain = ConfigTools::decrypt(Some(&cipher)).unwrap();
    assert_eq!(plain.as_deref(), Some("my-secret"));
}

/// Java 可空契约：None/空串原样返回。
#[test]
fn config_tools_null_and_empty_passthrough() {
    assert_eq!(ConfigTools::decrypt(None).unwrap(), None);

    assert_eq!(ConfigTools::decrypt(Some("")).unwrap(), Some(String::new()));
}

/// 自生成密钥对的加解密往返（genKeyPair → encrypt → decrypt）。
#[test]
fn config_tools_generated_key_pair_round_trip() {
    let [private_key, public_key] = ConfigTools::gen_key_pair(1024).unwrap();
    let cipher =
        ConfigTools::encrypt_with_key_text(Some(&private_key), "generated-secret").unwrap();
    let plain =
        ConfigTools::decrypt_with_public_key_text(Some(&public_key), Some(&cipher)).unwrap();
    assert_eq!(plain.as_deref(), Some("generated-secret"));
}

/// 非法输入返回结构化错误。
#[test]
fn config_tools_invalid_inputs_return_errors() {
    let result = ConfigTools::decrypt(Some("not-base64!!!"));
    assert!(result.is_err());

    let bad_key = ConfigTools::get_public_key(Some("not-base64"));
    assert!(bad_key.is_err());

    // 默认公钥可解析。

    assert!(ConfigTools::get_public_key(None).is_ok());
}

// ── Encoding 家族 ────────────────────────────────────────────

/// Java `CharsetConvert`：任一编码为 null 关闭转换。
#[test]
fn charset_convert_disabled_when_either_encoding_missing() {
    let converter = CharsetConvert::new(None, Some("GBK")).unwrap();
    assert_eq!(converter.encode("hello").unwrap(), "hello");
    assert_eq!(converter.decode("hello").unwrap(), "hello");

    let converter = CharsetConvert::new(Some("UTF-8"), None).unwrap();
    assert_eq!(converter.encode("value").unwrap(), "value");
}

/// Java：同名编码（忽略大小写）不启用转换。
#[test]
fn charset_convert_same_encoding_disabled() {
    let converter = CharsetConvert::new(Some("UTF-8"), Some("utf-8")).unwrap();
    assert_eq!(converter.encode("passthrough").unwrap(), "passthrough");
}

/// Java `new String(s.getBytes(source), target)`：异名编码启用转换且往返守恒。
#[test]
fn charset_convert_enabled_round_trip() {
    let converter = CharsetConvert::new(Some("UTF-8"), Some("GBK")).unwrap();
    assert_eq!(converter.client_encoding(), Some("UTF-8"));
    assert_eq!(converter.server_encoding(), Some("GBK"));

    let original = "中文测试";
    let encoded = converter.encode(original).unwrap();
    let decoded = converter.decode(&encoded).unwrap();
    assert_eq!(decoded, original, "encode→decode must round-trip");
}

/// 不支持的编码名立即报错（Java `UnsupportedEncodingException` 对应）。
#[test]
fn charset_convert_unsupported_encoding_errors() {
    assert!(CharsetConvert::new(Some("UTF-8"), Some("no-such-charset")).is_err());
}

/// `EncodingConvertFilter`：Value 字符串转换、非字符串直通。
#[test]
fn encoding_convert_filter_value_dispatch() {
    let filter = EncodingConvertFilter::new(Some("UTF-8"), Some("GBK")).unwrap();
    let text = Value::String("字段".to_owned());
    let encoded = filter.encode_value(text.clone()).unwrap();
    let decoded = filter.decode_value(encoded).unwrap();
    assert_eq!(decoded, text);

    let non_text = Value::Int(42);
    assert_eq!(filter.encode_value(non_text.clone()).unwrap(), non_text);
    assert_eq!(
        filter.decode_value(Value::Bool(true)).unwrap(),
        Value::Bool(true)
    );
}

/// `CharsetParameter` 保留 Java 键与 getter/setter 语义。
#[test]
fn charset_parameter_keys_and_accessors() {
    let mut parameter = CharsetParameter::default();
    assert_eq!(CharsetParameter::CLIENT_ENCODING_KEY, "clientEncoding");
    assert_eq!(CharsetParameter::SERVER_ENCODING_KEY, "serverEncoding");
    assert!(parameter.client_encoding().is_none());
    parameter.set_client_encoding(Some("UTF-8".to_owned()));
    parameter.set_server_encoding(Some("GBK".to_owned()));
    assert_eq!(parameter.client_encoding(), Some("UTF-8"));
    assert_eq!(parameter.server_encoding(), Some("GBK"));
    parameter.set_client_encoding(None);
    assert!(parameter.client_encoding().is_none());
}

// ── MySQL8DateTime 家族 ──────────────────────────────────────

/// 值替换恒等（Rust 值模型 canonical 即 Timestamp）。
#[test]
fn mysql8_datetime_value_replace_is_identity() {
    use chrono::NaiveDateTime;
    let stamp = Value::Timestamp(
        NaiveDateTime::parse_from_str("2026-08-13 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
    );
    assert_eq!(
        MySQL8DateTimeSqlTypeFilter::get_object_replace_local_date_time(stamp.clone()),
        stamp
    );
    assert_eq!(
        MySQL8DateTimeSqlTypeFilter::get_object_replace_local_date_time(Value::Int(1)),
        Value::Int(1)
    );
    assert_eq!(
        MySQL8DateTimeSqlTypeFilter::get_object_replace_local_date_time(Value::Null),
        Value::Null
    );
}

/// Filter 名与 before/after no-op。
#[tokio::test]
async fn mysql8_datetime_filter_identity_hooks() {
    use druid::core::{AfterFilter, BeforeFilter, ExecContext, ExecResult};
    let filter = MySQL8DateTimeSqlTypeFilter::new();
    assert_eq!(BeforeFilter::name(&filter), "mysql8DateTime");

    let params: Vec<Value> = Vec::new();
    let mut context = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: druid::core::ExecOperation::Execute,
    };
    BeforeFilter::before(&filter, &mut context).await.unwrap();
    AfterFilter::after(
        &filter,
        &context,
        &Ok(ExecResult::default()),
        std::time::Duration::ZERO,
    )
    .await
    .unwrap();
    let after_name = <MySQL8DateTimeSqlTypeFilter as AfterFilter>::name(&filter);
    assert_eq!(after_name, "mysql8DateTime");
}

/// metadata 装饰器：LocalDateTime 类名恢复为 Timestamp，其余透传。
#[test]
fn mysql8_datetime_metadata_restores_timestamp_class_name() {
    use druid::core::ResultSetMetaData;

    let dt_column = ResultSetColumnMeta::new("dt_col", ResultSetColumnType::Timestamp, true)
        .with_type_identity("DATETIME", "java.time.LocalDateTime");
    let id_column = ResultSetColumnMeta::new("id_col", ResultSetColumnType::Integer, true)
        .with_type_identity("BIGINT", "java.lang.Long");
    let base = ResultSetMetaData::new(vec![dt_column, id_column]);

    let wrapped = MySQL8DateTimeResultSetMetaData::new(base);
    assert_eq!(wrapped.column_class_name(1).unwrap(), "java.sql.Timestamp");
    assert_eq!(wrapped.column_class_name(2).unwrap(), "java.lang.Long");
    let handle = wrapped.into_result_set_meta_data();
    assert_eq!(handle.column_class_name(1).unwrap(), "java.sql.Timestamp");
}

// ── LogFilter ────────────────────────────────────────────────

/// Java `configFromProperties` 七键的精确匹配与大小写敏感值。
#[test]
fn log_filter_config_from_properties_seven_keys() {
    let filter = LogFilter::new();
    // 先全部关闭。
    filter.config_from_properties(&props(&[
        ("druid.log.conn", "false"),
        ("druid.log.stmt", "false"),
        ("druid.log.rs", "false"),
        ("druid.log.stmt.executableSql", "true"),
        ("druid.log.conn.logError", "false"),
        ("druid.log.stmt.logError", "false"),
        ("druid.log.rs.logError", "false"),
    ]));
    assert!(!filter.is_connection_log_enabled());
    assert!(!filter.is_statement_log_enabled());
    assert!(!filter.is_result_set_log_enabled());
    assert!(filter.is_statement_executable_sql_log_enabled());
    assert!(!filter.is_connection_log_error_enabled());
    assert!(!filter.is_statement_log_error_enabled());
    assert!(!filter.is_result_set_log_error_enabled());

    // 恢复 + 非法值不改状态（Java Boolean.parseBoolean 仅认 true）。
    filter.config_from_properties(&props(&[
        ("druid.log.conn", "true"),
        ("druid.log.stmt", "yes"),
    ]));
    assert!(filter.is_connection_log_enabled());
    assert!(!filter.is_statement_log_enabled());
}

/// Java 默认分类名与 setter。
#[test]
fn log_filter_default_categories_and_setters() {
    let filter = LogFilter::new();
    assert_eq!(filter.data_source_category(), "druid.sql.DataSource");
    assert_eq!(filter.connection_category(), "druid.sql.Connection");
    assert_eq!(filter.statement_category(), "druid.sql.Statement");
    assert_eq!(filter.result_set_category(), "druid.sql.ResultSet");

    filter.set_data_source_category("custom.ds");
    filter.set_connection_category("custom.conn");
    filter.set_statement_category("custom.stmt");
    filter.set_result_set_category("custom.rs");
    assert_eq!(filter.data_source_category(), "custom.ds");
    assert_eq!(filter.connection_category(), "custom.conn");
    assert_eq!(filter.statement_category(), "custom.stmt");
    assert_eq!(filter.result_set_category(), "custom.rs");
}

/// Java 默认开关快照（全部启用，executableSql 默认关闭）。
#[test]
fn log_filter_default_switches() {
    let filter = LogFilter::new();
    assert!(filter.is_data_source_log_enabled());
    assert!(filter.is_connection_log_enabled());
    assert!(filter.is_statement_log_enabled());
    assert!(filter.is_result_set_log_enabled());
    assert!(!filter.is_statement_executable_sql_log_enabled());
    assert!(filter.is_connection_connect_before_log_enabled());
    assert!(filter.is_statement_execute_batch_after_log_enabled());
    assert!(filter.is_result_set_next_after_log_enabled());

    filter.set_data_source_log_enabled(false);
    filter.set_result_set_close_after_log_enabled(false);
    assert!(!filter.is_data_source_log_enabled());
    assert!(!filter.is_result_set_close_after_log_enabled());
}

// ── EncodingConvertFilter / LogFilter trait 钩子直调 ──────────

/// `BeforeFilter` 钩子：`before()` 用 `encode` 改写 SQL；SQL/批次入口同样编码。
#[tokio::test]
async fn encoding_convert_filter_before_hooks_encode_sql() {
    use druid::core::{AfterFilter, BeforeFilter, ExecContext, ExecOperation, ExecResult};

    let filter = EncodingConvertFilter::new(Some("UTF-8"), Some("GBK")).unwrap();
    assert_eq!(BeforeFilter::name(&filter), "encoding");
    assert_eq!(AfterFilter::name(&filter), "encoding");

    let params: Vec<Value> = Vec::new();
    let mut context = ExecContext {
        connection_id: 9,
        statement_id: None,
        sql: "SELECT '字段' FROM t".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    BeforeFilter::before(&filter, &mut context).await.unwrap();
    assert_eq!(context.sql, filter.encode("SELECT '字段' FROM t").unwrap());
    AfterFilter::after(
        &filter,
        &context,
        &Ok(ExecResult::default()),
        std::time::Duration::ZERO,
    )
    .await
    .unwrap();

    // 预编译与批次 SQL 入口同样走 encode（Java EncodingConvertFilter 语义）。
    let prepared =
        BeforeFilter::prepare_statement_sql(&filter, "INSERT INTO t VALUES ('值')").unwrap();
    assert_eq!(
        prepared,
        filter.encode("INSERT INTO t VALUES ('值')").unwrap()
    );
    let batched = BeforeFilter::statement_add_batch_sql(&filter, "UPDATE t SET a = '甲'").unwrap();
    assert_eq!(batched, filter.encode("UPDATE t SET a = '甲'").unwrap());
}

/// `config_from_properties` 重建转换器（Java init 从连接属性读取编码）。
#[test]
fn encoding_convert_filter_config_from_properties_rebuilds() {
    use druid::core::BeforeFilter;

    let filter = EncodingConvertFilter::new(None, None).unwrap();
    assert_eq!(filter.encode("stable").unwrap(), "stable");

    BeforeFilter::config_from_properties(
        &filter,
        &props(&[("clientEncoding", "UTF-8"), ("serverEncoding", "GBK")]),
    )
    .unwrap();
    let original = "配置";
    let encoded = filter.encode(original).unwrap();
    assert_eq!(filter.decode(&encoded).unwrap(), original);

    // Clone 共享同一可变转换器状态。
    let cloned = filter.clone();
    assert_eq!(cloned.encode(original).unwrap(), encoded);
}

/// `LogFilter` trait 身份与 `before`/`after` no-op。
#[tokio::test]
async fn log_filter_trait_hooks_are_noop() {
    use druid::core::{AfterFilter, BeforeFilter, ExecContext, ExecOperation, ExecResult};

    let filter = LogFilter::new();
    assert_eq!(BeforeFilter::name(&filter), "log");
    assert_eq!(AfterFilter::name(&filter), "log");

    let params: Vec<Value> = Vec::new();
    let mut context = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: std::time::Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    BeforeFilter::before(&filter, &mut context).await.unwrap();
    assert_eq!(context.sql, "SELECT 1");
    AfterFilter::after(
        &filter,
        &context,
        &Ok(ExecResult::default()),
        std::time::Duration::ZERO,
    )
    .await
    .unwrap();
}
