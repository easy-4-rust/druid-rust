//! Java 8 JDBCType 与 Rust 标准 SQL 类型的同输入差分。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use druid_rdbc::types::{JdbcType, SqlType};

#[test]
fn java_and_rust_agree_on_every_jdbc42_type_identity() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Jdbc42TypesOracle.java");
    let output_dir = std::env::temp_dir().join(format!(
        "druid-jdbc42-types-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("oracle")
    ));
    std::fs::create_dir(&output_dir).expect("create Java oracle output directory");

    // 编译固定 Java 8 语义的 oracle；缺少 JDK 时显式失败，不能把差分记成跳过。
    let compile = Command::new("javac")
        .args(["-encoding", "UTF-8", "-source", "8", "-target", "8", "-d"])
        .arg(&output_dir)
        .arg(&fixture)
        .output()
        .expect("javac is required for JDBC 4.2 differential evidence");
    assert!(
        compile.status.success(),
        "Java oracle compilation failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let result = Command::new("java")
        .arg("-cp")
        .arg(&output_dir)
        .arg("Jdbc42TypesOracle")
        .output()
        .expect("java is required for JDBC 4.2 differential evidence");
    assert!(
        result.status.success(),
        "Java oracle execution failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let oracle = String::from_utf8(result.stdout).expect("Java oracle UTF-8 output");
    let java_types: BTreeMap<_, _> = oracle
        .lines()
        .map(|line| {
            let mut columns = line.split('\t');
            let name = columns.next().expect("type name");
            let vendor = columns.next().expect("type vendor");
            let code: i32 = columns
                .next()
                .expect("type number")
                .parse()
                .expect("integer type number");
            assert!(columns.next().is_none(), "unexpected Java oracle column");
            (name.to_owned(), (vendor.to_owned(), code))
        })
        .collect();

    let rust_types = [
        JdbcType::Bit,
        JdbcType::TinyInt,
        JdbcType::SmallInt,
        JdbcType::Integer,
        JdbcType::BigInt,
        JdbcType::Float,
        JdbcType::Real,
        JdbcType::Double,
        JdbcType::Numeric,
        JdbcType::Decimal,
        JdbcType::Char,
        JdbcType::VarChar,
        JdbcType::LongVarChar,
        JdbcType::Date,
        JdbcType::Time,
        JdbcType::Timestamp,
        JdbcType::Binary,
        JdbcType::VarBinary,
        JdbcType::LongVarBinary,
        JdbcType::Null,
        JdbcType::Other,
        JdbcType::JavaObject,
        JdbcType::Distinct,
        JdbcType::Struct,
        JdbcType::Array,
        JdbcType::Blob,
        JdbcType::Clob,
        JdbcType::Ref,
        JdbcType::DataLink,
        JdbcType::Boolean,
        JdbcType::RowId,
        JdbcType::NChar,
        JdbcType::NVarChar,
        JdbcType::LongNVarChar,
        JdbcType::NClob,
        JdbcType::SqlXml,
        JdbcType::RefCursor,
        JdbcType::TimeWithTimezone,
        JdbcType::TimestampWithTimezone,
    ];
    let rust_map: BTreeMap<_, _> = rust_types
        .into_iter()
        .map(|sql_type| {
            assert_eq!(
                JdbcType::value_of(sql_type.vendor_type_number()).expect("known JDBC type"),
                sql_type
            );
            (
                sql_type.name().to_owned(),
                (sql_type.vendor().to_owned(), sql_type.vendor_type_number()),
            )
        })
        .collect();

    assert_eq!(java_types.len(), 39, "Java 8 JDBCType denominator changed");
    assert_eq!(rust_map.len(), 39, "Rust JDBCType denominator changed");
    assert_eq!(rust_map, java_types);

    let class_file = output_dir.join("Jdbc42TypesOracle.class");
    std::fs::remove_file(class_file).expect("remove Java oracle class");
    std::fs::remove_dir(output_dir).expect("remove Java oracle output directory");
}
