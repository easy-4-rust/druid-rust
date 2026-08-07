//! Java Druid vendor `ExceptionSorter` 差分契约。

use druid::core::{
    AbstractOracleExceptionSorter, Db2ExceptionSorter, ExceptionSorter, ExceptionSorterProperties,
    InformixExceptionSorter, MockExceptionSorter, MySqlExceptionSorter, NullExceptionSorter,
    OceanBaseOracleExceptionSorter, OracleExceptionSorter, PgExceptionSorter,
    PhoenixExceptionSorter, SqlException, SqlExceptionCause, SybaseExceptionSorter,
    ORACLE_FATAL_ERROR_CODES_PROPERTY,
};
use std::collections::BTreeSet;

#[test]
fn sql_exception_preserves_nullable_rdbc_fields_and_runtime_identity() {
    let exception = SqlException::new(7, None, None)
        .with_sql_state("08006")
        .with_class_name("vendor.DriverException")
        .with_assignable_type("vendor.BaseDriverException")
        .with_assignable_type("vendor.BaseDriverException")
        .recoverable()
        .with_cause(SqlExceptionCause::ClassName("vendor.RootCause".to_string()));

    assert_eq!(exception.error_code(), 7);
    assert_eq!(exception.sql_state(), Some("08006"));
    assert_eq!(exception.message(), None);
    assert_eq!(exception.class_name(), "vendor.DriverException");
    assert!(exception.is_instance_of("vendor.DriverException"));
    assert!(exception.is_instance_of("vendor.BaseDriverException"));
    assert!(exception.is_instance_of("java.sql.SQLException"));
    assert!(!exception.is_instance_of("vendor.UnrelatedException"));
    assert_eq!(
        exception.assignable_types(),
        &[
            "vendor.DriverException".to_string(),
            "java.sql.SQLException".to_string(),
            "vendor.BaseDriverException".to_string(),
        ]
    );
    assert!(exception.is_recoverable());
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName("vendor.RootCause".to_string())]
    );
}

#[test]
fn null_and_pg_sorters_match_java_recoverable_and_sqlstate_rules() {
    let mut null = NullExceptionSorter;
    let mut pg = PgExceptionSorter;
    let properties = ExceptionSorterProperties::from([("ignored".to_string(), "1".to_string())]);
    null.config_from_properties(None);
    null.config_from_properties(Some(&properties));
    pg.config_from_properties(None);
    pg.config_from_properties(Some(&properties));

    assert!(std::ptr::eq(
        NullExceptionSorter::get_instance(),
        NullExceptionSorter::get_instance()
    ));
    assert!(!null.is_exception_fatal(&SqlException::driver(0, "anything").recoverable()));
    assert!(pg.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    assert!(pg.is_exception_fatal(
        &SqlException::driver(0, "connection exception").with_sql_state("08003")
    ));
    assert!(!pg.is_exception_fatal(&SqlException::driver(0, "state is null")));
    assert!(
        !pg.is_exception_fatal(&SqlException::driver(0, "syntax error").with_sql_state("42601"))
    );
}

#[test]
fn mysql_sorter_matches_every_java_fatal_branch_and_cause_depth_limit() {
    let mut sorter = MySqlExceptionSorter;
    let properties = ExceptionSorterProperties::new();
    sorter.config_from_properties(None);
    sorter.config_from_properties(Some(&properties));

    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    assert!(sorter.is_exception_fatal(
        &SqlException::driver(0, "connection exception").with_sql_state("08001")
    ));
    assert!(!sorter
        .is_exception_fatal(&SqlException::driver(1062, "duplicate").with_sql_state("23000")));

    for error_code in [
        1004, 1005, 1015, 1021, 1023, 1037, 1038, 1040, 1041, 1042, 1043, 1045, 1047, 1081, 1129,
        1130, 1142, 1227, 1290, -9000, -8500, -8000,
    ] {
        assert!(
            sorter.is_exception_fatal(&SqlException::driver(error_code, "vendor error")),
            "Java fatal error code {error_code} 必须丢弃连接"
        );
    }
    for error_code in [
        -9001, -7999, 0, 1044, 1046, 1048, 1049, 1050, 1051, 1052, 1053, 1062,
    ] {
        assert!(
            !sorter.is_exception_fatal(&SqlException::driver(error_code, "ordinary error")),
            "Java 非致命 error code {error_code} 不得误杀连接"
        );
    }

    assert!(sorter.is_exception_fatal(
        &SqlException::driver(0, "driver")
            .with_class_name("com.mysql.cj.rdbc.exceptions.CommunicationsException")
    ));
    assert!(sorter.is_exception_fatal(&SqlException::driver(
        0,
        concat!(
            "Streaming result set com.mysql.rdbc.RowDataDynamic ",
            "is still active. No statements may be issued when any streaming result sets are ",
            "open and in use on a given connection. Ensure that you have called .close() on any ",
            "active streaming result sets before attempting more queries."
        )
    )));
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "Communications link failure")));
    assert!(!sorter.is_exception_fatal(&SqlException::driver(1, "Communications link failure")));
    for message in [
        "Could not create connection",
        "No datasource configured",
        "No alive datasource",
    ] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(1, message)));
    }

    assert!(sorter.is_exception_fatal(
        &SqlException::driver(1, "nested").with_cause(SqlExceptionCause::SocketTimeout)
    ));
    assert!(
        sorter.is_exception_fatal(&SqlException::driver(1, "nested").with_cause(
            SqlExceptionCause::ClassName("vendor.CommunicationsException".to_string())
        ))
    );

    let sixth_socket_timeout =
        (0..5).fold(SqlException::driver(1, "nested"), |exception, index| {
            exception.with_cause(SqlExceptionCause::ClassName(format!(
                "vendor.OrdinaryCause{index}"
            )))
        });
    let sixth_socket_timeout = sixth_socket_timeout.with_cause(SqlExceptionCause::SocketTimeout);
    assert!(!sorter.is_exception_fatal(&sixth_socket_timeout));

    assert!(!sorter.is_exception_fatal(&SqlException::new(1, None, None)));
}

#[test]
fn db2_sorter_matches_recoverable_sqlstate_and_all_java_error_codes() {
    let mut sorter = Db2ExceptionSorter;
    sorter.config_from_properties(None);
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    assert!(sorter.is_exception_fatal(
        &SqlException::driver(0, "connection exception").with_sql_state("08003")
    ));
    for error_code in [-512, -514, -516, -518, -525, -909, -918, -924] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(error_code, "db2")));
    }
    assert!(
        !sorter.is_exception_fatal(&SqlException::driver(-911, "deadlock").with_sql_state("40001"))
    );
}

#[test]
fn informix_sorter_matches_recoverable_and_all_java_error_codes() {
    let mut sorter = InformixExceptionSorter;
    sorter.config_from_properties(None);
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    for error_code in [
        -710, -79716, -79730, -79734, -79735, -79736, -79756, -79757, -79758, -79759, -79760,
        -79788, -79811, -79812, -79836, -79837, -79879,
    ] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(error_code, "informix")));
    }
    assert!(!sorter.is_exception_fatal(&SqlException::driver(-746, "ordinary")));
}

#[test]
fn sybase_sorter_matches_recoverable_nullable_message_and_jz_codes() {
    let mut sorter = SybaseExceptionSorter;
    let properties = ExceptionSorterProperties::new();
    sorter.config_from_properties(None);
    sorter.config_from_properties(Some(&properties));
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "jZ0c0 connection dead")));
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "JZ0C1 io killed connection")));
    assert!(!sorter.is_exception_fatal(&SqlException::new(0, None, None)));
    assert!(!sorter.is_exception_fatal(&SqlException::driver(0, "ordinary")));
}

#[test]
fn abstract_oracle_sorter_matches_java_property_parsing_and_set_semantics() {
    let mut sorter = AbstractOracleExceptionSorter::new();
    sorter.config_from_properties(None);
    sorter.config_from_properties(Some(&ExceptionSorterProperties::new()));
    assert!(sorter.fatal_error_codes().is_empty());
    assert!(!sorter.contains(1));

    let properties = ExceptionSorterProperties::from([(
        ORACLE_FATAL_ERROR_CODES_PROPERTY.to_string(),
        "1,2,3,a,,2, 4,-5".to_string(),
    )]);
    sorter.config_from_properties(Some(&properties));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([-5, 1, 2, 3]));
    assert!(sorter.contains(3));

    let appended = ExceptionSorterProperties::from([(
        ORACLE_FATAL_ERROR_CODES_PROPERTY.to_string(),
        "4".to_string(),
    )]);
    sorter.config_from_properties(Some(&appended));
    assert_eq!(
        sorter.fatal_error_codes(),
        &BTreeSet::from([-5, 1, 2, 3, 4])
    );

    sorter.set_fatal_error_codes(BTreeSet::from([9]));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([9]));
}

#[test]
fn oracle_sorter_matches_all_java_code_message_and_custom_code_branches() {
    let mut default_sorter = OracleExceptionSorter::default();
    default_sorter.set_fatal_error_codes(BTreeSet::new());
    assert!(!default_sorter.is_exception_fatal(&SqlException::driver(0, "ordinary")));

    let mut sorter = OracleExceptionSorter::new();
    if let Ok(property) = std::env::var(ORACLE_FATAL_ERROR_CODES_PROPERTY) {
        for error_code in property
            .split(',')
            .filter_map(|item| item.parse::<i32>().ok())
        {
            assert!(sorter.fatal_error_codes().contains(&error_code));
        }
    }
    sorter.set_fatal_error_codes(BTreeSet::new());
    sorter.config_from_properties(None);
    sorter.config_from_properties(Some(&ExceptionSorterProperties::new()));

    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    for error_code in [
        28, 600, 1012, 1014, 1033, 1034, 1035, 1089, 1090, 1092, 1094, 2396, 3106, 3111, 3113,
        3114, 3134, 3135, 3136, 3138, 3142, 3143, 3144, 3145, 3149, 6801, 6802, 6805, 9918, 9920,
        9921, 17001, 17002, 17008, 17009, 17024, 17089, 17401, 17409, 17410, 17416, 17438, 17442,
        25407, 25408, 25409, 25425, 29276, 30676,
    ] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(error_code, "oracle")));
        assert!(sorter.is_exception_fatal(&SqlException::driver(-error_code, "oracle")));
    }
    for error_code in [12100, 12101, 12200, 12298, 12299] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(error_code, "TNS")));
    }
    for error_code in [
        0,
        27,
        29,
        12099,
        12300,
        19999,
        20000,
        20999,
        21000,
        i32::MIN,
    ] {
        assert!(
            !sorter.is_exception_fatal(&SqlException::driver(error_code, "ordinary")),
            "Java 非致命 Oracle error code {error_code} 不得误杀连接"
        );
    }

    for message in [
        "socket read failed",
        "控制套接字错误",
        "connection has already been closed",
        "broken pipe",
        "管道已结束",
    ] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(1, message)));
        assert!(!sorter.is_exception_fatal(&SqlException::driver(20000, message)));
        assert!(!sorter.is_exception_fatal(&SqlException::driver(20999, message)));
        assert!(sorter.is_exception_fatal(&SqlException::driver(21000, message)));
    }

    let properties = ExceptionSorterProperties::from([(
        ORACLE_FATAL_ERROR_CODES_PROPERTY.to_string(),
        "1,2,3,a,".to_string(),
    )]);
    sorter.config_from_properties(Some(&properties));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([1, 2, 3]));
    for error_code in [1, 2, 3, -1, -2, -3] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(error_code, "custom")));
    }
    assert!(!sorter.is_exception_fatal(&SqlException::new(4, None, None)));

    sorter.set_fatal_error_codes(BTreeSet::from([7]));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([7]));
    assert!(sorter.is_exception_fatal(&SqlException::new(-7, None, None)));
}

#[test]
fn oceanbase_oracle_sorter_preserves_java_extensions_to_oracle_rules() {
    let mut default_sorter = OceanBaseOracleExceptionSorter::default();
    default_sorter.set_fatal_error_codes(BTreeSet::new());
    assert!(!default_sorter.is_exception_fatal(&SqlException::driver(0, "ordinary")));

    let mut sorter = OceanBaseOracleExceptionSorter::new();
    if let Ok(property) = std::env::var(ORACLE_FATAL_ERROR_CODES_PROPERTY) {
        for error_code in property
            .split(',')
            .filter_map(|item| item.parse::<i32>().ok())
        {
            assert!(sorter.fatal_error_codes().contains(&error_code));
        }
    }
    sorter.set_fatal_error_codes(BTreeSet::new());
    sorter.config_from_properties(None);
    sorter.config_from_properties(Some(&ExceptionSorterProperties::new()));

    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    assert!(sorter.is_exception_fatal(
        &SqlException::driver(0, "connection exception").with_sql_state("08006")
    ));
    assert!(
        !sorter.is_exception_fatal(&SqlException::driver(0, "ordinary").with_sql_state("42000"))
    );

    for error_code in [
        28, 600, 1012, 1014, 1033, 1034, 1035, 1089, 1090, 1092, 1094, 2396, 3106, 3111, 3113,
        3114, 3134, 3135, 3136, 3138, 3142, 3143, 3144, 3145, 3149, 6801, 6802, 6805, 9918, 9920,
        9921, 17001, 17002, 17008, 17009, 17024, 17089, 17401, 17409, 17410, 17416, 17438, 17442,
        25407, 25408, 25409, 25425, 29276, 30676, 12100, 12299,
    ] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(-error_code, "oceanbase")));
    }

    for message in [
        "socket read failed",
        "控制套接字错误",
        "connection has already been closed",
        "broken pipe",
        "管道已结束",
    ] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(1, message)));
        assert!(!sorter.is_exception_fatal(&SqlException::driver(20000, message)));
    }
    for message in [
        "communications link failure",
        "could not create connection",
        "access denied for user",
        "no datasource",
        "no alive datasource",
    ] {
        assert!(sorter.is_exception_fatal(&SqlException::driver(20000, message)));
    }

    let properties = ExceptionSorterProperties::from([(
        ORACLE_FATAL_ERROR_CODES_PROPERTY.to_string(),
        "4".to_string(),
    )]);
    sorter.config_from_properties(Some(&properties));
    assert_eq!(sorter.fatal_error_codes(), &BTreeSet::from([4]));
    assert!(sorter.is_exception_fatal(&SqlException::new(-4, None, None)));
    assert!(!sorter.is_exception_fatal(&SqlException::new(5, None, None)));
}

#[test]
fn phoenix_and_mock_sorters_match_java_message_singleton_and_instanceof_rules() {
    let properties = ExceptionSorterProperties::new();
    let mut phoenix = PhoenixExceptionSorter;
    phoenix.config_from_properties(None);
    phoenix.config_from_properties(Some(&properties));
    assert!(phoenix.is_exception_fatal(&SqlException::driver(
        0,
        "Phoenix: Connection is null or closed"
    )));
    assert!(!phoenix.is_exception_fatal(&SqlException::driver(0, "connection is null or closed")));
    assert!(!phoenix.is_exception_fatal(&SqlException::new(0, None, None)));

    let mut mock = MockExceptionSorter;
    mock.config_from_properties(None);
    mock.config_from_properties(Some(&properties));
    assert!(std::ptr::eq(
        MockExceptionSorter::get_instance(),
        MockExceptionSorter::get_instance()
    ));
    let exact = SqlException::driver(0, "closed")
        .with_class_name("com.alibaba.druid.mock.MockConnectionClosedException");
    assert!(mock.is_exception_fatal(&exact));
    let subclass = SqlException::driver(0, "closed")
        .with_class_name("example.CustomMockConnectionClosedException")
        .with_assignable_type("com.alibaba.druid.mock.MockConnectionClosedException");
    assert!(mock.is_exception_fatal(&subclass));
    assert!(!mock.is_exception_fatal(
        &SqlException::driver(0, "closed").with_class_name("java.sql.SQLException")
    ));
}
