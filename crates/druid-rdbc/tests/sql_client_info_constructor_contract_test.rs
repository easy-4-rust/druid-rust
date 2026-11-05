//! `java.sql.SQLClientInfoException` 的构造器与继承方法契约测试。
//!
//! 对应 Java: `java.sql.SQLClientInfoException` 的 9 个 `<init>` 重载、
//! `getFailedProperties()`，以及继承自 `java.sql.SQLException` 的
//! `getErrorCode()`、`getSQLState()`、`getNextException()`、
//! `setNextException(SQLException)` 和 `iterator()`。
//!
//! Rust 侧没有为 `SQLClientInfoException` 另建构造器：Java 构造器的
//! reason / SQLState / vendorCode / cause 全部落在
//! [`SqlException`] 值对象上，被拒绝属性落在
//! [`SqlClientInfoException::failed_properties`] 上。因此每个用例按 Java
//! 构造器的实参重建同一个 Rust 值对象，并逐项断言 reason、SQLState、
//! vendor code、cause 链与 failed properties 与输入完全一致。
//!
//! 属性名大小写敏感：Java `Map<String,ClientInfoStatus>` 的键不做任何
//! 大小写折叠，Rust `HashMap<String,String>` 同样按字节精确匹配，
//! `ApplicationName` 与 `applicationname` 是两个不同的属性。
//!
//! 对应 Java: java.sql.SQLClientInfoException

use std::collections::HashMap;

use druid_rdbc::error::{SqlClientInfoException, SqlException, SqlExceptionCause};

/// Java `java.sql.SQLClientInfoException` 的规范类名。
const SQL_CLIENT_INFO_EXCEPTION_CLASS: &str = "java.sql.SQLClientInfoException";
/// Java `java.sql.SQLException` 的规范类名，用于断言继承关系。
const SQL_EXCEPTION_CLASS: &str = "java.sql.SQLException";
/// Java cause 的运行时类名。
const CAUSE_CLASS: &str = "java.lang.IllegalStateException";
/// Java cause 的消息。
const CAUSE_MESSAGE: &str = "underlying driver failure";
/// 通用 SQLState，用于构造可判定的客户端信息失败。
const CLIENT_INFO_SQL_STATE: &str = "01S00";
/// 客户端信息失败的原因文本。
const CLIENT_INFO_REASON: &str = "client info rejected";
/// 客户端信息失败的厂商错误码。
const CLIENT_INFO_VENDOR_CODE: i32 = 7;

/// Java `Throwable#toString()` 的等价文本：`类名: 消息`。
///
/// Java `SQLException(Throwable)` 会把 reason 设为 `cause.toString()`，
/// Rust 侧必须自行拼出同一文本。
fn throwable_to_string(class_name: &str, message: &str) -> String {
    format!("{class_name}: {message}")
}

/// Java `Throwable` 在 Rust 侧的等价 cause 表示：保留 cause 的运行时类名。
fn java_cause(class_name: &str) -> SqlExceptionCause {
    SqlExceptionCause::ClassName(class_name.to_owned())
}

/// Java `Map<String,ClientInfoStatus> failedProperties` 的等价构造。
fn failed_properties() -> HashMap<String, String> {
    HashMap::from([
        (
            "ApplicationName".to_owned(),
            "REASON_VALUE_INVALID".to_owned(),
        ),
        (
            "ClientUser".to_owned(),
            "REASON_UNKNOWN_PROPERTY".to_owned(),
        ),
    ])
}

/// 断言 Rust 规范类型与 `java.sql` 兼容别名指向同一个类型。
///
/// Java 每个异常类在 Rust 侧只有一个规范类型；兼容别名不得另起一份类型，
/// 否则构造器身份无法与 Java 类一一对应。
fn assert_same_type_as_compat<T: 'static, U: 'static>(_: &T, _: &U) {
    assert_eq!(
        std::any::TypeId::of::<T>(),
        std::any::TypeId::of::<U>(),
        "规范类型与兼容别名必须是同一个类型"
    );
}

/// 断言客户端信息异常的运行时类型仍是 `SQLClientInfoException`。
fn assert_client_info_type(exception: &SqlClientInfoException) {
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert!(exception
        .exception
        .is_instance_of(SQL_CLIENT_INFO_EXCEPTION_CLASS));
    // SQLClientInfoException 继承 SQLException，父类型链必须保留。
    assert!(exception.exception.is_instance_of(SQL_EXCEPTION_CLASS));
}

/// 断言 failed properties 与输入完全一致（键、值、数量均不变）。
fn assert_failed_properties_preserved(
    exception: &SqlClientInfoException,
    expected: &HashMap<String, String>,
) {
    assert_eq!(exception.failed_properties, *expected);
    assert_eq!(exception.failed_properties.len(), expected.len());
    for (name, status) in expected {
        assert_eq!(
            exception.failed_properties.get(name),
            Some(status),
            "被拒绝属性 {name} 必须精确保存"
        );
    }
}

// ---------------------------------------------------------------------------
// java.sql.SQLClientInfoException 构造器
// ---------------------------------------------------------------------------

/// 对应 Java: `SQLClientInfoException()`。
///
/// 空构造器不携带任何诊断信息：reason、SQLState 为 null，厂商错误码为 0，
/// 没有 cause，也没有被拒绝的属性。
#[test]
fn sql_client_info_init_empty_keeps_default_fields() {
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, None)
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: HashMap::new(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), None);
    assert!(exception.exception.causes().is_empty());
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &HashMap::new());
    assert_same_type_as_compat(
        &exception,
        &druid_rdbc::compat::SqlClientInfoException {
            exception: SqlException::new(0, None, None)
                .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
            failed_properties: HashMap::new(),
        },
    );
}

/// 对应 Java: `SQLClientInfoException(String, String, int, Map)`。
///
/// reason、SQLState、厂商错误码与 failed properties 四个实参必须逐一精确
/// 落到对应的 Rust 字段上。
#[test]
fn sql_client_info_init_reason_state_code_failed_properties_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), CLIENT_INFO_VENDOR_CODE);
    assert_eq!(exception.exception.sql_state(), Some(CLIENT_INFO_SQL_STATE));
    assert_eq!(exception.exception.message(), Some(CLIENT_INFO_REASON));
    assert!(exception.exception.causes().is_empty());
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java:
/// `SQLClientInfoException(String, String, int, Map, Throwable)`。
///
/// 带 cause 的重载在保留四个诊断实参之外，还必须保留 cause 链。
#[test]
fn sql_client_info_init_reason_state_code_failed_properties_cause_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), CLIENT_INFO_VENDOR_CODE);
    assert_eq!(exception.exception.sql_state(), Some(CLIENT_INFO_SQL_STATE));
    assert_eq!(exception.exception.message(), Some(CLIENT_INFO_REASON));
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java: `SQLClientInfoException(String, String, Map)`。
///
/// 该重载不携带厂商错误码，厂商错误码必须保持 0。
#[test]
fn sql_client_info_init_reason_state_failed_properties_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            0,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), Some(CLIENT_INFO_SQL_STATE));
    assert_eq!(exception.exception.message(), Some(CLIENT_INFO_REASON));
    assert!(exception.exception.causes().is_empty());
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java: `SQLClientInfoException(String, String, Map, Throwable)`。
///
/// 带 cause 的重载同样不携带厂商错误码，厂商错误码必须保持 0。
#[test]
fn sql_client_info_init_reason_state_failed_properties_cause_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            0,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), Some(CLIENT_INFO_SQL_STATE));
    assert_eq!(exception.exception.message(), Some(CLIENT_INFO_REASON));
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java: `SQLClientInfoException(String, Map)`。
///
/// 该重载不携带 SQLState，SQLState 必须保持 null。
#[test]
fn sql_client_info_init_reason_failed_properties_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, Some(CLIENT_INFO_REASON.to_owned()))
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), Some(CLIENT_INFO_REASON));
    assert!(exception.exception.causes().is_empty());
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java: `SQLClientInfoException(String, Map, Throwable)`。
///
/// 该重载既不携带 SQLState 也不携带厂商错误码；cause 必须保留，
/// 且 reason 仍是构造器传入的文本，而不是 `cause.toString()`。
#[test]
fn sql_client_info_init_reason_failed_properties_cause_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, Some(CLIENT_INFO_REASON.to_owned()))
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
            .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), Some(CLIENT_INFO_REASON));
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java: `SQLClientInfoException(Map)`。
///
/// 该重载只携带被拒绝的属性，reason 与 SQLState 必须保持 null，
/// 厂商错误码必须保持 0。
#[test]
fn sql_client_info_init_failed_properties_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, None)
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), None);
    assert!(exception.exception.causes().is_empty());
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java: `SQLClientInfoException(Map, Throwable)`。
///
/// Java `SQLException(Throwable)` 把 reason 设为 `cause.toString()`，
/// 因此 reason 必须是 `类名: 消息`，SQLState 为 null，厂商错误码为 0。
#[test]
fn sql_client_info_init_failed_properties_cause_preserves_all_fields() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            0,
            None,
            Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(
        exception.exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_client_info_type(&exception);
    assert_failed_properties_preserved(&exception, &properties);
}

// ---------------------------------------------------------------------------
// java.sql.SQLClientInfoException 自有方法
// ---------------------------------------------------------------------------

/// 对应 Java: `SQLClientInfoException#getFailedProperties()`。
///
/// 被拒绝的属性必须原样返回；属性名大小写敏感，
/// `ApplicationName` 与 `applicationname` 是两个不同的键。
#[test]
fn sql_client_info_get_failed_properties_returns_case_sensitive_property_map() {
    let properties = HashMap::from([
        (
            "ApplicationName".to_owned(),
            "REASON_VALUE_INVALID".to_owned(),
        ),
        (
            "applicationname".to_owned(),
            "REASON_UNKNOWN_PROPERTY".to_owned(),
        ),
        (
            "APPLICATIONNAME".to_owned(),
            "REASON_STRING_VALUE_INVALID".to_owned(),
        ),
    ]);
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_failed_properties_preserved(&exception, &properties);
    // 大小写不同的三个属性名必须是三个独立条目，不能折叠成一个。
    assert_eq!(exception.failed_properties.len(), 3);
    assert_eq!(
        exception.failed_properties.get("ApplicationName"),
        Some(&"REASON_VALUE_INVALID".to_owned())
    );
    assert_eq!(
        exception.failed_properties.get("applicationname"),
        Some(&"REASON_UNKNOWN_PROPERTY".to_owned())
    );
    assert_eq!(
        exception.failed_properties.get("APPLICATIONNAME"),
        Some(&"REASON_STRING_VALUE_INVALID".to_owned())
    );
    // 精确匹配：不存在的大小写变体不能命中任何条目。
    assert_eq!(exception.failed_properties.get("Applicationname"), None);
}

// ---------------------------------------------------------------------------
// java.sql.SQLClientInfoException 继承自 java.sql.SQLException 的方法
// ---------------------------------------------------------------------------

/// 对应 Java: `SQLClientInfoException#getErrorCode()`。
///
/// 继承的 getter 只能读到客户端信息异常自身携带的 SQL 诊断，
/// 不得读取别的异常实例的厂商错误码。
#[test]
fn sql_client_info_inherited_get_error_code_returns_own_vendor_code() {
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: failed_properties(),
    };

    assert_eq!(exception.exception.error_code(), CLIENT_INFO_VENDOR_CODE);
    assert_ne!(exception.exception.error_code(), 0);
}

/// 对应 Java: `SQLClientInfoException#getSQLState()`。
#[test]
fn sql_client_info_inherited_get_sql_state_returns_own_sql_state() {
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: failed_properties(),
    };

    assert_eq!(exception.exception.sql_state(), Some(CLIENT_INFO_SQL_STATE));
}

/// 对应 Java: `SQLClientInfoException#getNextException()`。
///
/// 异常链必须挂在客户端信息异常自身携带的 SQL 诊断上。
#[test]
fn sql_client_info_inherited_get_next_exception_returns_chained_exception() {
    let mut exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: failed_properties(),
    };
    exception.exception.set_next_exception(
        SqlException::driver(1213, "retry transaction").with_sql_state("40002"),
    );

    let next = exception
        .exception
        .next_exception()
        .expect("next exception 必须保留在异常链中");
    assert_eq!(next.error_code(), 1213);
    assert_eq!(next.sql_state(), Some("40002"));
    assert_eq!(next.message(), Some("retry transaction"));
    assert_eq!(exception.exception.next_exceptions().len(), 1);
}

/// 对应 Java: `SQLClientInfoException#setNextException(SQLException)`。
///
/// 追加异常链后，原诊断字段不得被改写。
#[test]
fn sql_client_info_inherited_set_next_exception_appends_chain_without_touching_diagnostics() {
    let mut exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: failed_properties(),
    };
    let properties = exception.failed_properties.clone();

    exception
        .exception
        .set_next_exception(SqlException::driver(1213, "retry transaction"));

    assert_eq!(exception.exception.next_exceptions().len(), 1);
    assert_eq!(
        exception
            .exception
            .next_exception()
            .map(SqlException::error_code),
        Some(1213)
    );
    // 原诊断与被拒绝属性不受异常链追加影响。
    assert_eq!(exception.exception.error_code(), CLIENT_INFO_VENDOR_CODE);
    assert_eq!(exception.exception.sql_state(), Some(CLIENT_INFO_SQL_STATE));
    assert_eq!(exception.exception.message(), Some(CLIENT_INFO_REASON));
    assert_failed_properties_preserved(&exception, &properties);
}

/// 对应 Java: `SQLClientInfoException#iterator()`。
///
/// Java `Throwable#iterator()` 遍历的是 cause 链（不含自身），
/// Rust 侧等价入口是 `SqlException::causes()`，顺序为直接 cause 到根 cause。
#[test]
fn sql_client_info_inherited_iterator_exposes_cause_chain() {
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            CLIENT_INFO_VENDOR_CODE,
            Some(CLIENT_INFO_SQL_STATE.to_owned()),
            Some(CLIENT_INFO_REASON.to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
        .with_cause(SqlExceptionCause::SocketTimeout)
        .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: failed_properties(),
    };

    let causes = exception.exception.causes();
    assert_eq!(causes.len(), 2);
    assert_eq!(causes[0], SqlExceptionCause::SocketTimeout);
    assert_eq!(causes[1], java_cause(CAUSE_CLASS));
    // 没有 cause 的客户端信息异常，其 cause 链迭代结果为空。
    let without_cause = SqlClientInfoException {
        exception: SqlException::new(0, None, None)
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: HashMap::new(),
    };
    assert!(without_cause.exception.causes().is_empty());
}

/// 对应 Java: `java.sql.SQLClientInfoException` 类型本身。
///
/// Java 类声明在 Rust 侧只有 `druid_rdbc::error::SqlClientInfoException`
/// 这一个规范值类型，兼容别名不得另起一份类型。
#[test]
fn sql_client_info_type_entry_has_single_canonical_value_type() {
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, None)
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: failed_properties(),
    };

    assert_same_type_as_compat(
        &exception,
        &druid_rdbc::compat::SqlClientInfoException {
            exception: SqlException::new(0, None, None)
                .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
            failed_properties: failed_properties(),
        },
    );
    assert_client_info_type(&exception);
}
