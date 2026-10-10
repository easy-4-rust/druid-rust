//! RowSet setter 与 RowSetInternal#getParams 所需的参数状态合同；不认证完整 RowSet。
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::extensions::RowSetParameterStore;
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter};
use druid_rdbc::types::{
    RdbcCalendarArgument, RdbcCharacterLength, RdbcInputStream, RdbcObject, RdbcReader,
    RdbcStreamLength,
};

fn index(value: usize) -> ParameterIndex {
    ParameterIndex::new(value).unwrap()
}

#[test]
fn parameters_survive_repeated_reads_and_rebinding_replaces_only_selected_slot() {
    let mut store = RowSetParameterStore::new();
    assert!(store.get_params().unwrap().is_empty());
    store.set_index(index(2), PreparedInputParameter::Int(22));
    store.set_index(
        index(1),
        PreparedInputParameter::String(Some("first".into())),
    );
    let before = vec![
        PreparedInputParameter::String(Some("first".into())),
        PreparedInputParameter::Int(22),
    ];
    assert_eq!(store.get_params().unwrap(), before);
    assert_eq!(store.get_params().unwrap(), before);
    store.set_index(index(2), PreparedInputParameter::Int(23));
    assert_eq!(
        store.get_params().unwrap(),
        vec![before[0].clone(), PreparedInputParameter::Int(23)]
    );
    assert_eq!(store.len(), 2);
}

#[test]
fn sparse_extreme_index_is_rejected_without_allocation_or_lost_binding() {
    let mut store = RowSetParameterStore::new();
    store.set_index(index(usize::MAX), PreparedInputParameter::Int(7));
    assert!(matches!(
        store.get_params(),
        Err(RdbcError::SqlException(_))
    ));
    assert_eq!(store.len(), 1);
    store.clear_parameters();
    store.set_index(index(1), PreparedInputParameter::Int(8));
    assert_eq!(
        store.get_params().unwrap(),
        vec![PreparedInputParameter::Int(8)]
    );
    assert!(ParameterIndex::try_from(0_i64).is_err());
    assert!(ParameterIndex::try_from(-1_i64).is_err());
}

#[test]
fn names_require_driver_resolution_and_last_binding_wins_across_aliases() {
    let mut store = RowSetParameterStore::new();
    store.set_index(index(1), PreparedInputParameter::Int(1));
    store
        .set_named(Some(" exact 名称 "), PreparedInputParameter::Int(2))
        .unwrap();
    assert!(
        store.get_params().is_err(),
        "never invent lexical name-to-index correspondence"
    );
    let mut seen = Vec::new();
    assert_eq!(
        store
            .resolve_params(|name| {
                seen.push(name.to_owned());
                Ok(index(1))
            })
            .unwrap(),
        vec![PreparedInputParameter::Int(2)]
    );
    assert_eq!(seen, vec![" exact 名称 "]);
    store.set_index(index(1), PreparedInputParameter::Int(3));
    assert_eq!(
        store.resolve_params(|_| Ok(index(1))).unwrap(),
        vec![PreparedInputParameter::Int(3)]
    );
    store
        .set_named(Some(" exact 名称 "), PreparedInputParameter::Int(4))
        .unwrap();
    assert_eq!(
        store.resolve_params(|_| Ok(index(1))).unwrap(),
        vec![PreparedInputParameter::Int(4)]
    );
    assert_eq!(
        store.len(),
        2,
        "repeated setter cannot append unbounded history"
    );
}

#[test]
fn invalid_names_and_original_resolution_errors_do_not_mutate_parameters() {
    let mut store = RowSetParameterStore::new();
    store.set_index(index(1), PreparedInputParameter::Int(1));
    for name in [None, Some("")] {
        assert!(store
            .set_named(name, PreparedInputParameter::Int(2))
            .is_err());
    }
    assert_eq!(
        store.get_params().unwrap(),
        vec![PreparedInputParameter::Int(1)]
    );
    store
        .set_named(Some("second"), PreparedInputParameter::Int(2))
        .unwrap();
    let failure = SqlException::new(
        193,
        Some("42000".into()),
        Some("exact resolver failure".into()),
    );
    let Err(RdbcError::SqlException(error)) =
        store.resolve_params(|_| Err(RdbcError::SqlException(Box::new(failure.clone()))))
    else {
        panic!("preserve original resolver SQL exception")
    };
    assert_eq!(*error, failure);
    assert_eq!(
        store.resolve_params(|_| Ok(index(2))).unwrap(),
        vec![
            PreparedInputParameter::Int(1),
            PreparedInputParameter::Int(2)
        ]
    );
}

#[test]
fn parameter_snapshots_preserve_null_target_scale_calendar_and_lob_overloads() {
    let descriptors = vec![
        PreparedInputParameter::null(12),
        PreparedInputParameter::null_with_type_name(12, None),
        PreparedInputParameter::null_with_type_name(2002, Some("APP.TYPE".into())),
        PreparedInputParameter::Boolean(true),
        PreparedInputParameter::Byte(-7),
        PreparedInputParameter::Short(-32768),
        PreparedInputParameter::Int(7),
        PreparedInputParameter::Long(i64::MAX),
        PreparedInputParameter::Float(1.25),
        PreparedInputParameter::Double(2.5),
        PreparedInputParameter::BigDecimal(None),
        PreparedInputParameter::String(None),
        PreparedInputParameter::NString(None),
        PreparedInputParameter::Bytes(Some(vec![0, 255])),
        PreparedInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified,
        },
        PreparedInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None),
        },
        PreparedInputParameter::Time {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None),
        },
        PreparedInputParameter::Timestamp {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None),
        },
        PreparedInputParameter::object(None),
        PreparedInputParameter::object_with_sql_type(Some(RdbcObject::Integer(8)), 4),
        PreparedInputParameter::object_with_sql_type_and_scale(Some(RdbcObject::Integer(9)), 3, -4),
        PreparedInputParameter::Ref(None),
        PreparedInputParameter::Blob(None),
        PreparedInputParameter::Clob(None),
        PreparedInputParameter::NClob(None),
        PreparedInputParameter::Array(None),
        PreparedInputParameter::Url(None),
        PreparedInputParameter::RowId(None),
        PreparedInputParameter::SqlXml(None),
        PreparedInputParameter::BlobStream {
            stream: None,
            length: RdbcStreamLength::Long(-2),
        },
        PreparedInputParameter::ClobReader {
            reader: None,
            length: RdbcCharacterLength::Long(-3),
        },
        PreparedInputParameter::NClobReader {
            reader: None,
            length: RdbcCharacterLength::Unspecified,
        },
    ];
    let mut store = RowSetParameterStore::new();
    for (offset, parameter) in descriptors.iter().enumerate() {
        store.set_index(index(offset + 1), parameter.clone());
    }
    assert_eq!(store.get_params().unwrap(), descriptors);
}

#[test]
fn storing_snapshotting_and_clearing_do_not_read_or_close_callers_streams() {
    let stream = RdbcInputStream::from_bytes(b"abc".to_vec());
    let reader = RdbcReader::from_utf16(vec![0x0061, 0xd800, 0x0062]);
    let mut store = RowSetParameterStore::new();
    let byte_parameter = PreparedInputParameter::BinaryStream {
        stream: Some(stream.clone()),
        length: RdbcStreamLength::Int(-9),
    };
    let character_parameter = PreparedInputParameter::NCharacterStream {
        reader: Some(reader.clone()),
        length: RdbcCharacterLength::Long(3),
    };
    store.set_index(index(1), byte_parameter.clone());
    store.set_index(index(2), character_parameter.clone());
    let snapshot = store.get_params().unwrap();
    assert_eq!(snapshot, vec![byte_parameter, character_parameter]);
    let mut byte = [0];
    assert_eq!(stream.read(&mut byte).unwrap(), 1);
    assert_eq!(byte, [b'a']);
    let PreparedInputParameter::BinaryStream {
        stream: Some(shared),
        ..
    } = &snapshot[0]
    else {
        panic!("byte handle required")
    };
    assert_eq!(shared.read(&mut byte).unwrap(), 1);
    assert_eq!(byte, [b'b'], "snapshot must share the original cursor");
    store.clear_parameters();
    assert!(store.is_empty());
    assert!(!stream.is_closed());
    assert_eq!(stream.read_to_end().unwrap(), b"c");
    let mut units = [0; 3];
    assert_eq!(reader.read_utf16(&mut units).unwrap(), 3);
    assert_eq!(units, [0x0061, 0xd800, 0x0062]);
}
