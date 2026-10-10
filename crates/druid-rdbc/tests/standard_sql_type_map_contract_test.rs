//! 固定 SQLInput#readObject 的自定义类型映射行为合同。
use druid_rdbc::error::{RdbcError, RdbcErrorKind, SqlException};
use druid_rdbc::extensions::{BufferedSqlInput, BufferedSqlOutput, SqlDataObject, SqlDataRegistry};
use druid_rdbc::resource::{SqlData, SqlInput, SqlOutput};
use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap, Value};
use std::cell::Cell;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Record {
    number: Cell<i32>,
    type_name: String,
}
#[async_trait::async_trait]
impl SqlData for Record {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok(self.type_name.clone())
    }
    async fn read_sql(
        &mut self,
        input: &mut dyn SqlInput,
        type_name: &str,
    ) -> Result<(), RdbcError> {
        self.type_name = type_name.into();
        let number = input.read_int().await?;
        self.number.set(number);
        Ok(())
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        output.write_int(self.number.get()).await
    }
}

async fn structured(number: i32) -> RdbcObject {
    let mut value = Record {
        number: Cell::new(number),
        type_name: "APP.RECORD".into(),
    };
    let mut out = BufferedSqlOutput::new();
    out.write_object(Some(&mut value)).await.unwrap();
    out.into_input().read_object().await.unwrap()
}

fn mapping() -> RdbcTypeMap {
    let mut map = RdbcTypeMap::new();
    map.insert(
        "APP.RECORD",
        RdbcTargetType::Custom("example.Record".into()),
    );
    map
}

#[tokio::test]
async fn mapped_object_constructs_send_only_sql_data_and_reads_qualified_name() {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&count);
    let mut registry = SqlDataRegistry::new();
    registry.register("example.Record", move || {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Record {
            number: Cell::new(-1),
            type_name: String::new(),
        }) as Box<dyn SqlData>)
    });
    let mut input = BufferedSqlInput::with_type_map(
        vec![structured(73).await, structured(74).await],
        mapping(),
        Arc::new(registry),
    );
    let first = input.read_object().await.unwrap();
    let second = input.read_object().await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_ne!(
        first, second,
        "each SQLInput value constructs a fresh application instance"
    );
    assert!(!input.was_null().unwrap());
    let RdbcObject::Custom(object) = first else {
        panic!("mapped SQLData must preserve its application object")
    };
    assert_eq!(object.class_name(), "example.Record");
    let record = object.downcast_ref::<SqlDataObject>().unwrap();
    assert_eq!(record.sql_type_name().await.unwrap(), "APP.RECORD");
    let mut output = BufferedSqlOutput::new();
    record.write_sql(&mut output).await.unwrap();
    assert_eq!(output.into_input().read_int().await.unwrap(), 73);
}

#[tokio::test]
async fn null_and_unmapped_structure_preserve_default_mapping_and_reference_identity() {
    let value = structured(81).await;
    let mut input = BufferedSqlInput::with_type_map(
        vec![RdbcObject::Scalar(Value::Null), value.clone()],
        RdbcTypeMap::new(),
        Arc::new(SqlDataRegistry::new()),
    );
    assert!(input.read_object().await.unwrap().is_null());
    assert!(input.was_null().unwrap());
    assert_eq!(input.read_object().await.unwrap(), value);
    assert!(!input.was_null().unwrap());
}

#[tokio::test]
async fn missing_registered_class_is_rejected_without_silently_returning_struct() {
    let mut input = BufferedSqlInput::with_type_map(
        vec![structured(82).await, RdbcObject::Integer(99)],
        mapping(),
        Arc::new(SqlDataRegistry::new()),
    );
    assert_eq!(
        input.read_object().await.unwrap_err().kind(),
        RdbcErrorKind::SqlFeatureNotSupported
    );
    assert_eq!(
        input.read_int().await.unwrap(),
        99,
        "the failed mapping consumes exactly the original attribute"
    );
}

#[tokio::test]
async fn constructor_error_keeps_original_sql_exception_and_does_not_publish_partial_object() {
    let mut registry = SqlDataRegistry::new();
    registry.register("example.Record", || {
        Err(RdbcError::SqlException(Box::new(SqlException::new(
            317,
            Some("22000".into()),
            Some("original constructor failure".into()),
        ))))
    });
    let mut input = BufferedSqlInput::with_type_map(
        vec![structured(83).await, RdbcObject::Integer(100)],
        mapping(),
        Arc::new(registry),
    );
    let Err(RdbcError::SqlException(error)) = input.read_object().await else {
        panic!("original SQL exception required")
    };
    assert_eq!(error.error_code(), 317);
    assert_eq!(error.sql_state(), Some("22000"));
    assert_eq!(input.read_int().await.unwrap(), 100);
}

struct Parent {
    child: Option<RdbcObject>,
}
#[async_trait::async_trait]
impl SqlData for Parent {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("APP.PARENT".into())
    }
    async fn read_sql(
        &mut self,
        input: &mut dyn SqlInput,
        type_name: &str,
    ) -> Result<(), RdbcError> {
        assert_eq!(type_name, "APP.PARENT");
        self.child = Some(input.read_object().await?);
        Ok(())
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        let RdbcObject::Custom(value) = self.child.as_ref().unwrap() else {
            panic!("nested child must be custom mapped")
        };
        value
            .downcast_ref::<SqlDataObject>()
            .unwrap()
            .write_sql(output)
            .await
    }
}

#[tokio::test]
async fn nested_sql_input_inherits_the_same_type_map_and_factory_registry() {
    // 使用标准 writeStruct 生成嵌套结构，不以手工字符串模拟 UDT。
    struct EncodedParent {
        child: RdbcObject,
    }
    #[async_trait::async_trait]
    impl SqlData for EncodedParent {
        async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
            Ok("APP.PARENT".into())
        }
        async fn read_sql(&mut self, input: &mut dyn SqlInput, _: &str) -> Result<(), RdbcError> {
            self.child = input.read_object().await?;
            Ok(())
        }
        async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
            let RdbcObject::Struct(child) = &self.child else {
                panic!("standard structure required")
            };
            output.write_struct(Some(child.clone())).await
        }
    }
    let mut parent = EncodedParent {
        child: structured(84).await,
    };
    let mut output = BufferedSqlOutput::new();
    output.write_object(Some(&mut parent)).await.unwrap();
    let encoded = output.into_input().read_object().await.unwrap();
    let mut registry = SqlDataRegistry::new();
    registry.register("example.Record", || {
        Ok(Box::new(Record {
            number: Cell::new(0),
            type_name: String::new(),
        }) as Box<dyn SqlData>)
    });
    registry.register("example.Parent", || {
        Ok(Box::new(Parent { child: None }) as Box<dyn SqlData>)
    });
    let mut map = mapping();
    map.insert(
        "APP.PARENT",
        RdbcTargetType::Custom("example.Parent".into()),
    );
    let mut input = BufferedSqlInput::with_type_map(vec![encoded], map, Arc::new(registry));
    let RdbcObject::Custom(mapped) = input.read_object().await.unwrap() else {
        panic!("mapped parent required")
    };
    let mut restored = BufferedSqlOutput::new();
    mapped
        .downcast_ref::<SqlDataObject>()
        .unwrap()
        .write_sql(&mut restored)
        .await
        .unwrap();
    assert_eq!(restored.into_input().read_int().await.unwrap(), 84);
}
