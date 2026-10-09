/// 默认对象Filter链保持六种原生入口、参数引用和资源返回身份。
#[tokio::test]
async fn task6_object_filter_default_preserves_resource_identity_and_native_overloads() {
    use crate::core::{FilterAdapter, FilterChain};
    use druid_rdbc::types::{
        RdbcObject, RdbcOpaqueObject, RdbcReader, RdbcTargetType, RdbcTypeMap,
    };
    use object_payload::ObjectPayload;
    use object_read_probe::ObjectReadProbe;
    use std::sync::Mutex;
    let stream = RdbcReader::from_string("资源对象😀");
    let payload = RdbcOpaqueObject::new(Arc::new(ObjectPayload("原对象".to_owned())));
    for object in [
        RdbcObject::CharacterStream(stream.clone()),
        RdbcObject::Custom(payload.clone()),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let arguments = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(FilterAdapter::new()));
        filters.add_result_set(Arc::new(FilterAdapter::new()));
        let (mut statement, mut connection) =
            task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
        let admission = statement.inner.begin_standard_admission().unwrap();
        let mut rows = DruidPooledResultSet::new_standard(
            Arc::clone(&statement.inner),
            Box::new(ObjectReadProbe {
                calls: Arc::clone(&calls),
                arguments: Arc::clone(&arguments),
                object: object.clone(),
                closed: false,
            }),
            &admission,
        )
        .await
        .unwrap();
        drop(admission);
        let map = RdbcTypeMap::new();
        let target = RdbcTargetType::Custom("example.Resource".to_owned());
        let index = ColumnIndex::new(1).unwrap();
        // 无行定位、无额外find_column、无目标类型预转换，原资源句柄无损返回。
        for returned in [
            rows.object(index),
            rows.object_by_label("ordinary"),
            rows.object_with_type_map(index, None),
            rows.object_with_type_map_by_label("mapped", Some(&map)),
            rows.object_as(index, &target),
            rows.object_as_by_label("typed", &target),
        ] {
            assert_eq!(returned.unwrap(), object);
        }
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "raw:ordinary:1",
                "raw:ordinary_label:ordinary",
                "raw:map:1:false",
                "raw:map_label:mapped:true",
                "raw:typed:1:Custom(\"example.Resource\")",
                "raw:typed_label:typed:Custom(\"example.Resource\")",
            ]
        );
        assert_eq!(
            *arguments.lock().unwrap(),
            vec![
                std::ptr::from_ref(&map) as usize,
                std::ptr::from_ref(&target) as usize,
                std::ptr::from_ref(&target) as usize,
            ]
        );
        assert_eq!(statement.exception_count(), 0);
        match rows.object_as_by_label("typed", &target).unwrap() {
            RdbcObject::CharacterStream(returned) => {
                let mut prefix = [0_u16; 2];
                assert_eq!(returned.read_utf16(&mut prefix).unwrap(), 2);
                assert_eq!(String::from_utf16(&prefix).unwrap(), "资源");
                assert_eq!(stream.read_to_string().unwrap(), "对象😀");
            }
            RdbcObject::Custom(returned) => {
                assert_eq!(returned, payload);
                assert!(std::ptr::eq(returned.value(), payload.value()));
                assert_eq!(
                    returned.downcast_ref::<ObjectPayload>().unwrap().0,
                    "原对象"
                );
            }
            other => panic!("完整对象不能转换为标量: {other:?}"),
        }
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}
