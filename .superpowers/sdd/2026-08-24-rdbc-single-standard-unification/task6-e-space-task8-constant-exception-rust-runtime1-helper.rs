use druid_rdbc::types::types;
use druid_rdbc::error::SqlException;
fn main() {
    let constants = [
        ("BIT", types::BIT),
        ("TINYINT", types::TINYINT),
        ("SMALLINT", types::SMALLINT),
        ("INTEGER", types::INTEGER),
        ("BIGINT", types::BIGINT),
        ("FLOAT", types::FLOAT),
        ("REAL", types::REAL),
        ("DOUBLE", types::DOUBLE),
        ("NUMERIC", types::NUMERIC),
        ("DECIMAL", types::DECIMAL),
        ("CHAR", types::CHAR),
        ("VARCHAR", types::VARCHAR),
        ("LONGVARCHAR", types::LONGVARCHAR),
        ("DATE", types::DATE),
        ("TIME", types::TIME),
        ("TIMESTAMP", types::TIMESTAMP),
        ("BINARY", types::BINARY),
        ("VARBINARY", types::VARBINARY),
        ("LONGVARBINARY", types::LONGVARBINARY),
        ("NULL", types::NULL),
        ("OTHER", types::OTHER),
        ("JAVA_OBJECT", types::JAVA_OBJECT),
        ("DISTINCT", types::DISTINCT),
        ("STRUCT", types::STRUCT),
        ("ARRAY", types::ARRAY),
        ("BLOB", types::BLOB),
        ("CLOB", types::CLOB),
        ("REF", types::REF),
        ("DATALINK", types::DATALINK),
        ("BOOLEAN", types::BOOLEAN),
        ("ROWID", types::ROWID),
        ("NCHAR", types::NCHAR),
        ("NVARCHAR", types::NVARCHAR),
        ("LONGNVARCHAR", types::LONGNVARCHAR),
        ("NCLOB", types::NCLOB),
        ("SQLXML", types::SQLXML),
        ("REF_CURSOR", types::REF_CURSOR),
        ("TIME_WITH_TIMEZONE", types::TIME_WITH_TIMEZONE),
        ("TIMESTAMP_WITH_TIMEZONE", types::TIMESTAMP_WITH_TIMEZONE),
    ];
    for (name, number) in constants { println!("CONSTANT\t{name}\t{number}"); }
    let classes = [
        "java.sql.SQLTransientException",
        "java.sql.SQLTransientConnectionException",
        "java.sql.SQLNonTransientException",
        "java.sql.SQLNonTransientConnectionException",
        "java.sql.SQLRecoverableException",
        "java.sql.SQLDataException",
        "java.sql.SQLIntegrityConstraintViolationException",
        "java.sql.SQLInvalidAuthorizationSpecException",
        "java.sql.SQLSyntaxErrorException",
        "java.sql.SQLTransactionRollbackException",
        "java.sql.SQLFeatureNotSupportedException",
        "java.sql.SQLTimeoutException",
    ];
    for class_name in classes {
        for state in [None, Some("42S02")] {
            let e = SqlException::new(17, state.map(str::to_owned), Some("original".to_owned())).with_class_name(class_name);
            println!("KIND\t{}\t{}\t{:?}\t{}\t{}", e.class_name(), e.sql_state().unwrap_or("null"), e.kind(), e.error_code(), e.message().unwrap_or("null"));
        }
    }
}
