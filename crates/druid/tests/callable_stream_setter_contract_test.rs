//! `java.sql.CallableStatement` 流与资源索引 setter 的绑定合同。
//!
//! 对应 Java: CallableStatement 继承 `PreparedStatement` 的
//! `setAsciiStream/setBinaryStream/setCharacterStream/setNCharacterStream/
//! setBlob/setClob/setNClob/setObject`。测试只断言 1-based 索引与 descriptor
//! variant/length 原样到达驱动，不消费流，也不把 NULL 资源物化。

use druid::core::DruidPooledConnection;
use druid::core::RdbcParameter;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ParameterIndex, PreparedInputParameter, PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::RdbcObject;
use std::sync::{Arc, Mutex};

/// 单次驱动级绑定记录。
#[derive(Debug, Clone, PartialEq)]
struct TypedBinding {
    index: usize,
    parameter: PreparedInputParameter,
}

#[derive(Debug, Default)]
struct RecordingState {
    bindings: Mutex<Vec<TypedBinding>>,
}

impl RecordingState {
    fn snapshot(&self) -> Vec<TypedBinding> {
        self.bindings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

struct RecordingCallable {
    sql: String,
    state: Arc<RecordingState>,
}

druid_rdbc::impl_standard_wrapper!(RecordingCallable => CallableStatement);

#[async_trait::async_trait]
impl Statement for RecordingCallable {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for RecordingCallable {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.state
            .bindings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(TypedBinding {
                index: index.get(),
                parameter,
            });
        Ok(())
    }
}

#[async_trait::async_trait]
impl CallableStatement for RecordingCallable {
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        _parameter_name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        Ok(())
    }
}

struct RecordingConnection {
    state: Arc<RecordingState>,
}

druid_rdbc::impl_standard_wrapper!(RecordingConnection => Connection);

#[async_trait::async_trait]
impl Connection for RecordingConnection {
    async fn prepare_call(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(RecordingCallable {
            sql: sql.to_owned(),
            state: Arc::clone(&self.state),
        }))
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("recording-callable-stream".to_owned())
    }
}

struct RecordingFactory {
    state: Arc<RecordingState>,
}

#[async_trait::async_trait]
impl ConnectionFactory for RecordingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(RecordingConnection {
            state: Arc::clone(&self.state),
        }))
    }
}

fn index(value: usize) -> ParameterIndex {
    ParameterIndex::new(value).expect("1-based parameter index")
}

async fn recorded_callable() -> (
    Arc<RecordingState>,
    DruidPooledConnection,
    Box<dyn CallableStatement>,
) {
    let state = Arc::new(RecordingState::default());
    let pool = DruidPool::builder()
        .name("callable-stream-setter")
        .driver_name("recording-callable-stream")
        .factory(Arc::new(RecordingFactory {
            state: Arc::clone(&state),
        }) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("recording pool");
    let mut connection = pool.get().await.expect("pooled connection");
    let callable = connection
        .prepare_call("{call demo(?, ?)}", StatementOptions::default())
        .await
        .expect("callable handle");
    (state, connection, callable)
}

/// 期望的 22 次绑定：索引严格按调用顺序 1..=22。
fn expected_bindings() -> Vec<PreparedInputParameter> {
    let ascii = RdbcInputStream::from_bytes(b"ascii".to_vec());
    let binary = RdbcInputStream::from_bytes(vec![1, 2, 3]);
    let reader = RdbcReader::from_string("reader");
    vec![
        // setAsciiStream x3
        PreparedInputParameter::AsciiStream {
            stream: Some(ascii.clone()),
            length: RdbcStreamLength::Unspecified,
        },
        PreparedInputParameter::AsciiStream {
            stream: Some(ascii.clone()),
            length: RdbcStreamLength::Int(5),
        },
        PreparedInputParameter::AsciiStream {
            stream: Some(ascii),
            length: RdbcStreamLength::Long(5),
        },
        // setBinaryStream x3
        PreparedInputParameter::BinaryStream {
            stream: Some(binary.clone()),
            length: RdbcStreamLength::Unspecified,
        },
        PreparedInputParameter::BinaryStream {
            stream: Some(binary.clone()),
            length: RdbcStreamLength::Int(3),
        },
        PreparedInputParameter::BinaryStream {
            stream: Some(binary),
            length: RdbcStreamLength::Long(3),
        },
        // setCharacterStream x3
        PreparedInputParameter::CharacterStream {
            reader: Some(reader.clone()),
            length: RdbcCharacterLength::Unspecified,
        },
        PreparedInputParameter::CharacterStream {
            reader: Some(reader.clone()),
            length: RdbcCharacterLength::Int(6),
        },
        PreparedInputParameter::CharacterStream {
            reader: Some(reader.clone()),
            length: RdbcCharacterLength::Long(6),
        },
        // setNCharacterStream x2
        PreparedInputParameter::NCharacterStream {
            reader: Some(reader.clone()),
            length: RdbcCharacterLength::Unspecified,
        },
        PreparedInputParameter::NCharacterStream {
            reader: Some(reader),
            length: RdbcCharacterLength::Long(6),
        },
        // setBlob x3
        PreparedInputParameter::BlobStream {
            stream: Some(RdbcInputStream::from_bytes(vec![9, 8])),
            length: RdbcStreamLength::Unspecified,
        },
        PreparedInputParameter::BlobStream {
            stream: Some(RdbcInputStream::from_bytes(vec![7, 6])),
            length: RdbcStreamLength::Long(2),
        },
        PreparedInputParameter::Blob(None),
        // setClob x3
        PreparedInputParameter::ClobReader {
            reader: Some(RdbcReader::from_string("clob")),
            length: RdbcCharacterLength::Unspecified,
        },
        PreparedInputParameter::ClobReader {
            reader: Some(RdbcReader::from_string("clob-long")),
            length: RdbcCharacterLength::Long(9),
        },
        PreparedInputParameter::Clob(None),
        // setNClob x3
        PreparedInputParameter::NClobReader {
            reader: Some(RdbcReader::from_string("nclob")),
            length: RdbcCharacterLength::Unspecified,
        },
        PreparedInputParameter::NClobReader {
            reader: Some(RdbcReader::from_string("nclob-long")),
            length: RdbcCharacterLength::Long(10),
        },
        PreparedInputParameter::NClob(None),
        // setObject(Object, SQLType) / (Object, SQLType, int)
        PreparedInputParameter::object_with_sql_type(Some(RdbcObject::Integer(7)), 4),
        PreparedInputParameter::object_with_sql_type_and_scale(
            Some(RdbcObject::String("scaled".to_owned())),
            12,
            3,
        ),
    ]
}

fn apply_bindings(callable: &mut dyn CallableStatement) {
    let expected = expected_bindings();
    for (offset, parameter) in expected.into_iter().enumerate() {
        callable
            .set_input_parameter(index(offset + 1), parameter)
            .unwrap();
    }
}

#[tokio::test]
async fn callable_stream_and_resource_setters_preserve_variant_and_length() {
    let (state, _connection, mut callable) = recorded_callable().await;
    apply_bindings(callable.as_mut());
    let recorded = state.snapshot();
    let expected = expected_bindings();

    assert_eq!(
        recorded.len(),
        expected.len(),
        "one binding per setter call"
    );
    for (position, (binding, parameter)) in recorded.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            binding.index,
            position + 1,
            "binding must stay on its 1-based index"
        );
        // 流/Reader 在代理层会被 clone 成新的句柄 id，因此比较 variant 种类、
        // SQL 类型与 length 字段，而不是句柄身份。
        assert_eq!(
            std::mem::discriminant(&binding.parameter),
            std::mem::discriminant(parameter),
            "descriptor variant must survive"
        );
        assert_eq!(
            binding.parameter.sql_type(),
            parameter.sql_type(),
            "descriptor SQL type must survive"
        );
        assert_eq!(
            binding.parameter.length(),
            parameter.length(),
            "descriptor length must survive"
        );
    }
}

#[tokio::test]
async fn callable_stream_setters_do_not_consume_or_close_caller_streams() {
    let (state, _connection, mut callable) = recorded_callable().await;
    let ascii = RdbcInputStream::from_bytes(b"stream".to_vec());
    let reader = RdbcReader::from_string("reader");
    callable
        .set_input_parameter(
            index(1),
            PreparedInputParameter::AsciiStream {
                stream: Some(ascii.clone()),
                length: RdbcStreamLength::Long(6),
            },
        )
        .unwrap();
    callable
        .set_input_parameter(
            index(2),
            PreparedInputParameter::CharacterStream {
                reader: Some(reader.clone()),
                length: RdbcCharacterLength::Long(6),
            },
        )
        .unwrap();

    assert!(!ascii.is_closed());
    assert!(!reader.is_closed());
    assert_eq!(ascii.read_to_end().unwrap(), b"stream");
    assert_eq!(reader.read_to_string().unwrap(), "reader");
    assert_eq!(state.snapshot().len(), 2);
}
