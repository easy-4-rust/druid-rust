//! `java.sql.PreparedStatement` 资源 setter 在标准 PreparedStatement 句柄上的绑定契约。
//!
//! 覆盖 `setArray`/`setBlob`/`setClob`/`setNClob`/`setRef`/`setRowId`/`setSQLXML`/`setURL`
//! 家族：记录型 mock 只捕获 `PreparedInputParameter` 描述符与 1-based 索引，
//! 不代替真实驱动执行，也不在池化层把资源句柄提前物化成标量。
//!
//! 对应 Java: `java.sql.PreparedStatement` 的 `setXxx(...)` 方法族。

use std::sync::{Arc, Mutex};

use druid::core::{
    DruidPooledConnection, RdbcError, RdbcRowId, RdbcUrl, ResourceHandleFactory, ResourceOperations,
};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::resource::{
    Array, Blob, BlobPattern, Clob, ClobPattern, NClob, RdbcInputStream, RdbcOutputStream,
    RdbcReader, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource, Ref, SqlXml,
};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    ParameterIndex, PreparedInputParameter, PreparedParameter, PreparedStatement, Statement,
    StatementOptions,
};
use druid_rdbc::types::{RdbcObject, RdbcString, RdbcTypeMap};

/// 记录型 mock 捕获的全部绑定痕迹。
#[derive(Default)]
struct Recorder {
    /// `set_input_parameter` 收到的 (1-based index, descriptor) 顺序序列。
    input_parameters: Vec<(ParameterIndex, PreparedInputParameter)>,
    /// `set_parameter` 收到的标量快照；资源描述符不得落到这里。
    parameters: Vec<PreparedParameter>,
    /// mock connection 实际创建的 prepared statement 数量。
    prepared_statements: usize,
}

/// 记录型标准连接；只为暴露 prepared statement 绑定痕迹。
struct RecordingConnection {
    recorder: Arc<Mutex<Recorder>>,
}

druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);

#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.recorder.lock().unwrap().prepared_statements += 1;
        Ok(Box::new(RecordingPreparedStatement {
            recorder: Arc::clone(&self.recorder),
            sql: sql.to_owned(),
            closed: false,
        }))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

/// 记录型标准 prepared statement；把资源描述符原样记录到共享 Vec。
struct RecordingPreparedStatement {
    recorder: Arc<Mutex<Recorder>>,
    sql: String,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(RecordingPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl Statement for RecordingPreparedStatement {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RecordingPreparedStatement {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }

    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.recorder.lock().unwrap().parameters.push(parameter);
        Ok(())
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.recorder
            .lock()
            .unwrap()
            .input_parameters
            .push((index, parameter));
        Ok(())
    }
}

/// 未被断言路径触碰的资源操作统一返回明确的不支持错误，避免 panic 掩盖真实失败。
fn unsupported(operation: &'static str) -> RdbcError {
    RdbcError::feature_not_supported(operation)
}

/// 生成 1-based 参数索引。
fn parameter_index(value: usize) -> ParameterIndex {
    ParameterIndex::new(value).expect("1-based parameter index")
}

/// 记录型连接工厂；只负责产出记录型标准连接。
struct RecordingFactory {
    recorder: Arc<Mutex<Recorder>>,
}

#[async_trait::async_trait]
impl ConnectionFactory for RecordingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(RecordingConnection {
            recorder: Arc::clone(&self.recorder),
        }))
    }

    fn name(&self) -> &str {
        "prepared-resource-setter-fixture"
    }
}

/// 用记录型 mock 构造带 prepared statement 缓存的 DruidPool。
async fn make_pool(recorder: Arc<Mutex<Recorder>>) -> DruidPool {
    DruidPool::builder()
        .factory(Arc::new(RecordingFactory { recorder }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(8)
        .build()
        .await
        .expect("pool")
}

/// 在池化连接上准备一个标准 prepared statement 句柄。
async fn prepare_statement(connection: &mut DruidPooledConnection) -> Box<dyn PreparedStatement> {
    Connection::prepare_statement(
        connection,
        "INSERT INTO resource_setters(id) VALUES (?)",
        StatementOptions::default(),
    )
    .await
    .expect("prepared statement")
}

/// `java.sql.Array` mock；只声明能力集，不实现任何读操作。
#[derive(Debug)]
struct MockArray;

#[async_trait::async_trait]
impl Array for MockArray {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::array()
    }

    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        Err(unsupported("Array#getBaseTypeName"))
    }

    async fn base_type(&mut self) -> Result<i32, RdbcError> {
        Err(unsupported("Array#getBaseType"))
    }

    async fn values(&mut self) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(unsupported("Array#getArray"))
    }

    async fn values_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(unsupported("Array#getArray(Map)"))
    }

    async fn values_range(
        &mut self,
        _index: i64,
        _count: i32,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(unsupported("Array#getArray(long,int)"))
    }

    async fn values_range_with_type_map(
        &mut self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(unsupported("Array#getArray(long,int,Map)"))
    }

    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(unsupported("Array#getResultSet"))
    }

    async fn result_set_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(unsupported("Array#getResultSet(Map)"))
    }

    async fn result_set_range(
        &mut self,
        _index: i64,
        _count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(unsupported("Array#getResultSet(long,int)"))
    }

    async fn result_set_range_with_type_map(
        &mut self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(unsupported("Array#getResultSet(long,int,Map)"))
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

/// `java.sql.Blob` mock；只声明能力集，不实现任何读操作。
#[derive(Debug)]
struct MockBlob;

#[async_trait::async_trait]
impl Blob for MockBlob {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::blob()
    }

    async fn length(&mut self) -> Result<i64, RdbcError> {
        Err(unsupported("Blob#length"))
    }

    async fn get_bytes(&mut self, _position: i64, _length: i32) -> Result<Vec<u8>, RdbcError> {
        Err(unsupported("Blob#getBytes"))
    }

    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(unsupported("Blob#getBinaryStream"))
    }

    async fn position_bytes(
        &mut self,
        _pattern: &[u8],
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(unsupported("Blob#position(byte[],long)"))
    }

    async fn position_blob(
        &mut self,
        _pattern: BlobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(unsupported("Blob#position(Blob,long)"))
    }

    async fn set_bytes(&mut self, _position: i64, _bytes: &[u8]) -> Result<i32, RdbcError> {
        Err(unsupported("Blob#setBytes"))
    }

    async fn set_bytes_range(
        &mut self,
        _position: i64,
        _bytes: &[u8],
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(unsupported("Blob#setBytes(long,byte[],int,int)"))
    }

    async fn set_binary_stream(&mut self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(unsupported("Blob#setBinaryStream"))
    }

    async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
        Err(unsupported("Blob#truncate"))
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn get_binary_stream_range(
        &mut self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        Err(unsupported("Blob#getBinaryStream(long,long)"))
    }
}

/// 为 `Clob` 与 `NClob` mock 生成同一份只读能力契约。
macro_rules! impl_clob_contract {
    ($type:ty) => {
        #[async_trait::async_trait]
        impl Clob for $type {
            fn capabilities(&self) -> ResourceOperations {
                ResourceOperations::clob()
            }

            async fn length(&mut self) -> Result<i64, RdbcError> {
                Err(unsupported("Clob#length"))
            }

            async fn get_sub_string(
                &mut self,
                _position: i64,
                _length: i32,
            ) -> Result<RdbcString, RdbcError> {
                Err(unsupported("Clob#getSubString"))
            }

            async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
                Err(unsupported("Clob#getCharacterStream"))
            }

            async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
                Err(unsupported("Clob#getAsciiStream"))
            }

            async fn position_string(
                &mut self,
                _pattern: &RdbcString,
                _start: i64,
            ) -> Result<Option<i64>, RdbcError> {
                Err(unsupported("Clob#position(String,long)"))
            }

            async fn position_clob(
                &mut self,
                _pattern: ClobPattern<'_>,
                _start: i64,
            ) -> Result<Option<i64>, RdbcError> {
                Err(unsupported("Clob#position(Clob,long)"))
            }

            async fn set_string(
                &mut self,
                _position: i64,
                _value: &RdbcString,
            ) -> Result<i32, RdbcError> {
                Err(unsupported("Clob#setString"))
            }

            async fn set_string_range(
                &mut self,
                _position: i64,
                _value: &RdbcString,
                _offset: i32,
                _length: i32,
            ) -> Result<i32, RdbcError> {
                Err(unsupported("Clob#setString(long,String,int,int)"))
            }

            async fn set_ascii_stream(
                &mut self,
                _position: i64,
            ) -> Result<RdbcOutputStream, RdbcError> {
                Err(unsupported("Clob#setAsciiStream"))
            }

            async fn set_character_stream(
                &mut self,
                _position: i64,
            ) -> Result<RdbcWriter, RdbcError> {
                Err(unsupported("Clob#setCharacterStream"))
            }

            async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
                Err(unsupported("Clob#truncate"))
            }

            async fn free(&mut self) -> Result<(), RdbcError> {
                Ok(())
            }

            async fn get_character_stream_range(
                &mut self,
                _position: i64,
                _length: i64,
            ) -> Result<RdbcReader, RdbcError> {
                Err(unsupported("Clob#getCharacterStream(long,long)"))
            }
        }
    };
}

/// `java.sql.Clob` mock。
#[derive(Debug)]
struct MockClob;

impl_clob_contract!(MockClob);

/// `java.sql.NClob` mock。
#[derive(Debug)]
struct MockNClob;

impl_clob_contract!(MockNClob);

impl NClob for MockNClob {}

/// `java.sql.Ref` mock。
#[derive(Debug)]
struct MockRef;

#[async_trait::async_trait]
impl Ref for MockRef {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::reference()
    }

    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        Err(unsupported("Ref#getBaseTypeName"))
    }

    async fn object(&mut self) -> Result<RdbcObject, RdbcError> {
        Err(unsupported("Ref#getObject"))
    }

    async fn object_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError> {
        Err(unsupported("Ref#getObject(Map)"))
    }

    async fn set_object(&mut self, _value: RdbcObject) -> Result<(), RdbcError> {
        Err(unsupported("Ref#setObject"))
    }
}

/// `java.sql.SQLXML` mock。
#[derive(Debug)]
struct MockSqlXml;

#[async_trait::async_trait]
impl SqlXml for MockSqlXml {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::sql_xml()
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(unsupported("SQLXML#getBinaryStream"))
    }

    async fn set_binary_stream(&mut self) -> Result<RdbcOutputStream, RdbcError> {
        Err(unsupported("SQLXML#setBinaryStream"))
    }

    async fn character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        Err(unsupported("SQLXML#getCharacterStream"))
    }

    async fn set_character_stream(&mut self) -> Result<RdbcWriter, RdbcError> {
        Err(unsupported("SQLXML#setCharacterStream"))
    }

    async fn string(&mut self) -> Result<RdbcString, RdbcError> {
        Err(unsupported("SQLXML#getString"))
    }

    async fn set_string(&mut self, _value: &RdbcString) -> Result<(), RdbcError> {
        Err(unsupported("SQLXML#setString"))
    }

    async fn source(
        &mut self,
        _representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError> {
        Err(unsupported("SQLXML#getSource"))
    }

    async fn result(
        &mut self,
        _representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError> {
        Err(unsupported("SQLXML#setResult"))
    }
}

#[tokio::test]
async fn prepared_resource_setters_bind_null_resources_to_expected_variants_and_indexes() {
    let recorder = Arc::new(Mutex::new(Recorder::default()));
    let pool = make_pool(Arc::clone(&recorder)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut prepared = prepare_statement(&mut connection).await;

    // 每个 Rust 描述符对应 java.sql.PreparedStatement 的一个资源 setter 家族。
    let expected: Vec<(usize, PreparedInputParameter)> = vec![
        (1, PreparedInputParameter::Array(None)),
        (2, PreparedInputParameter::Blob(None)),
        (3, PreparedInputParameter::Clob(None)),
        (4, PreparedInputParameter::NClob(None)),
        (5, PreparedInputParameter::Ref(None)),
        (6, PreparedInputParameter::RowId(None)),
        (7, PreparedInputParameter::SqlXml(None)),
        (8, PreparedInputParameter::Url(None)),
    ];

    for (index, descriptor) in &expected {
        prepared
            .set_input_parameter(parameter_index(*index), descriptor.clone())
            .expect("资源描述符必须被标准句柄接受");
    }

    {
        let guard = recorder.lock().unwrap();
        assert_eq!(
            guard.input_parameters.len(),
            expected.len(),
            "每次资源绑定只能在标准句柄上产生一次调用"
        );
        assert!(
            guard.parameters.is_empty(),
            "资源描述符不得退化为标量 set_parameter"
        );
        assert_eq!(guard.prepared_statements, 1);

        for (position, (index, descriptor)) in expected.iter().enumerate() {
            let (recorded_index, recorded_descriptor) = &guard.input_parameters[position];
            assert_eq!(
                recorded_index.get(),
                *index,
                "参数位置必须按 1-based 原样转发"
            );
            assert_eq!(
                recorded_descriptor, descriptor,
                "资源描述符必须在标准句柄上保持同一个 variant 与关键字段"
            );
        }

        // 逐 variant 断言：资源句柄为空时必须是 None，而不是缺省句柄。
        assert!(matches!(
            &guard.input_parameters[0].1,
            PreparedInputParameter::Array(None)
        ));
        assert!(matches!(
            &guard.input_parameters[1].1,
            PreparedInputParameter::Blob(None)
        ));
        assert!(matches!(
            &guard.input_parameters[2].1,
            PreparedInputParameter::Clob(None)
        ));
        assert!(matches!(
            &guard.input_parameters[3].1,
            PreparedInputParameter::NClob(None)
        ));
        assert!(matches!(
            &guard.input_parameters[4].1,
            PreparedInputParameter::Ref(None)
        ));
        assert!(matches!(
            &guard.input_parameters[5].1,
            PreparedInputParameter::RowId(None)
        ));
        assert!(matches!(
            &guard.input_parameters[6].1,
            PreparedInputParameter::SqlXml(None)
        ));
        assert!(matches!(
            &guard.input_parameters[7].1,
            PreparedInputParameter::Url(None)
        ));
    }

    // 代理证据：只有经过 DruidPooledPreparedStatement 的私有缓存，
    // 关闭后的 prepared statement 才会回到池化缓存条目中。
    prepared.close().await.expect("prepared statement close");
    assert_eq!(
        pool.state().cached_prepared_statement_count,
        1,
        "断言必须发生在 DruidPooledPreparedStatement 标准句柄上"
    );
    connection.close().await.expect("connection close");
    pool.close().await;
}

#[tokio::test]
async fn prepared_resource_setters_bind_non_null_resource_handles_without_materializing_them() {
    let recorder = Arc::new(Mutex::new(Recorder::default()));
    let pool = make_pool(Arc::clone(&recorder)).await;
    let mut connection = pool.get().await.expect("pooled connection");
    let mut prepared = prepare_statement(&mut connection).await;

    let url = RdbcUrl::new("https://example.invalid/resource");
    let row_id = RdbcRowId::new(vec![0x01, 0x02, 0x03]);
    let blob = ResourceHandleFactory::blob(Box::new(MockBlob));
    let clob = ResourceHandleFactory::clob(Box::new(MockClob));
    let n_clob = ResourceHandleFactory::n_clob(Box::new(MockNClob));
    let array = ResourceHandleFactory::array(Box::new(MockArray));
    let reference = ResourceHandleFactory::reference(Box::new(MockRef));
    let sql_xml = ResourceHandleFactory::sql_xml(Box::new(MockSqlXml));

    let expected: Vec<(usize, PreparedInputParameter)> = vec![
        (1, PreparedInputParameter::Url(Some(url.clone()))),
        (2, PreparedInputParameter::RowId(Some(row_id.clone()))),
        (3, PreparedInputParameter::Blob(Some(blob.clone()))),
        (4, PreparedInputParameter::Clob(Some(clob.clone()))),
        (5, PreparedInputParameter::NClob(Some(n_clob.clone()))),
        (6, PreparedInputParameter::Array(Some(array.clone()))),
        (7, PreparedInputParameter::Ref(Some(reference.clone()))),
        (8, PreparedInputParameter::SqlXml(Some(sql_xml.clone()))),
    ];

    for (index, descriptor) in &expected {
        prepared
            .set_input_parameter(parameter_index(*index), descriptor.clone())
            .expect("资源句柄必须被标准句柄接受");
    }

    {
        let guard = recorder.lock().unwrap();
        assert_eq!(
            guard.input_parameters.len(),
            expected.len(),
            "每个非 NULL 资源句柄只能产生一次绑定调用"
        );
        assert!(
            guard.parameters.is_empty(),
            "资源句柄不得被池化层提前物化成标量"
        );

        for (position, (index, descriptor)) in expected.iter().enumerate() {
            let (recorded_index, recorded_descriptor) = &guard.input_parameters[position];
            assert_eq!(
                recorded_index.get(),
                *index,
                "参数位置必须按 1-based 原样转发"
            );
            assert_eq!(
                recorded_descriptor, descriptor,
                "非 NULL 资源句柄必须保持同一个 variant 与同一个句柄身份"
            );
        }

        // 关键字段：句柄内容未被读取或改写。
        assert_eq!(url.external_form(), "https://example.invalid/resource");
        assert_eq!(row_id.get_bytes(), vec![0x01, 0x02, 0x03]);
        assert!(matches!(
            &guard.input_parameters[0].1,
            PreparedInputParameter::Url(Some(_))
        ));
        assert!(matches!(
            &guard.input_parameters[1].1,
            PreparedInputParameter::RowId(Some(_))
        ));
        assert!(matches!(
            &guard.input_parameters[2].1,
            PreparedInputParameter::Blob(Some(_))
        ));
        assert!(matches!(
            &guard.input_parameters[3].1,
            PreparedInputParameter::Clob(Some(_))
        ));
        assert!(matches!(
            &guard.input_parameters[4].1,
            PreparedInputParameter::NClob(Some(_))
        ));
        assert!(matches!(
            &guard.input_parameters[5].1,
            PreparedInputParameter::Array(Some(_))
        ));
        assert!(matches!(
            &guard.input_parameters[6].1,
            PreparedInputParameter::Ref(Some(_))
        ));
        assert!(matches!(
            &guard.input_parameters[7].1,
            PreparedInputParameter::SqlXml(Some(_))
        ));
    }

    // 代理证据：只有经过 DruidPooledPreparedStatement 的私有缓存，
    // 关闭后的 prepared statement 才会回到池化缓存条目中。
    prepared.close().await.expect("prepared statement close");
    assert_eq!(
        pool.state().cached_prepared_statement_count,
        1,
        "断言必须发生在 DruidPooledPreparedStatement 标准句柄上"
    );
    connection.close().await.expect("connection close");
    pool.close().await;
}
