//! `javax.sql.RowSet` 参数 setter 的固定重载：NULL 资源句柄、字节、NString 与 NCharacterStream。
//!
//! 每个用例清空参数后调用一个 setter，再读取 `params()` 快照，断言 descriptor
//! 家族与 by-name 解析到的槽位，避免只验证调用成功。

use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{RowSet, RowSetInternal, SharedRowSet};
use druid_rdbc::resource::{RdbcCharacterLength, RdbcReader};
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter};
use std::sync::Arc;

fn index(value: usize) -> ParameterIndex {
    ParameterIndex::new(value).unwrap()
}

fn row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source
        .set_named_parameter_resolver(Some(Arc::new(|_, name| {
            assert_eq!(name, "p");
            Ok(index(1))
        })))
        .unwrap();
    source
}

fn assert_single(source: &mut SharedRowSet, expected: PreparedInputParameter) {
    assert_eq!(
        source.params().unwrap(),
        vec![expected],
        "descriptor must be stored at the resolved slot"
    );
}

#[test]
fn row_set_null_type_name_array_blob_clob_ref_setters_store_exact_descriptors() {
    let mut source = row_set();

    source
        .set_null_with_type_name(index(1), 4, Some("MY_UDT"))
        .unwrap();
    assert_single(
        &mut source,
        PreparedInputParameter::Null {
            sql_type: 4,
            type_name: druid_rdbc::statement::PreparedTypeNameArgument::Specified(Some(
                "MY_UDT".to_owned(),
            )),
        },
    );

    source.clear_parameters().unwrap();
    source
        .set_null_with_type_name_by_name(Some("p"), 12, None)
        .unwrap();
    assert_single(
        &mut source,
        PreparedInputParameter::Null {
            sql_type: 12,
            type_name: druid_rdbc::statement::PreparedTypeNameArgument::Specified(None),
        },
    );

    source.clear_parameters().unwrap();
    source.set_array(index(1), None).unwrap();
    assert_single(&mut source, PreparedInputParameter::Array(None));

    source.clear_parameters().unwrap();
    source.set_blob(index(1), None).unwrap();
    assert_single(&mut source, PreparedInputParameter::Blob(None));

    source.clear_parameters().unwrap();
    source.set_blob_by_name(Some("p"), None).unwrap();
    assert_single(&mut source, PreparedInputParameter::Blob(None));

    source.clear_parameters().unwrap();
    source.set_clob(index(1), None).unwrap();
    assert_single(&mut source, PreparedInputParameter::Clob(None));

    source.clear_parameters().unwrap();
    source.set_clob_by_name(Some("p"), None).unwrap();
    assert_single(&mut source, PreparedInputParameter::Clob(None));

    source.clear_parameters().unwrap();
    source.set_ref(index(1), None).unwrap();
    assert_single(&mut source, PreparedInputParameter::Ref(None));
}

#[test]
fn row_set_bytes_nstring_and_ncharacter_stream_setters_store_exact_descriptors() {
    let mut source = row_set();

    source.set_bytes(index(1), Some(vec![1, 2, 3])).unwrap();
    assert_single(
        &mut source,
        PreparedInputParameter::Bytes(Some(vec![1, 2, 3])),
    );

    source.clear_parameters().unwrap();
    source
        .set_bytes_by_name(Some("p"), Some(vec![9, 8]))
        .unwrap();
    assert_single(&mut source, PreparedInputParameter::Bytes(Some(vec![9, 8])));

    source.clear_parameters().unwrap();
    source
        .set_n_string_by_name(Some("p"), Some("国字"))
        .unwrap();
    assert_single(
        &mut source,
        PreparedInputParameter::NString(Some("国字".to_owned())),
    );

    source.clear_parameters().unwrap();
    let reader = RdbcReader::from_string("reader");
    source
        .set_n_character_stream_with_length_by_name(Some("p"), Some(reader.clone()), 6)
        .unwrap();
    assert_single(
        &mut source,
        PreparedInputParameter::NCharacterStream {
            reader: Some(reader.clone()),
            length: RdbcCharacterLength::Long(6),
        },
    );
    // 绑定不得消费调用者 reader。
    assert_eq!(reader.read_to_string().unwrap(), "reader");
    assert_eq!(source.params().unwrap().len(), 1);
}
