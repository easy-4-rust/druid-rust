//! 扩展 Adapter 共用的 Prepared 参数物化策略。

use druid::core::{
    DruidError, PreparedInputParameter, RdbcCharacterLength, RdbcObject, RdbcStreamLength, Value,
};

/// 将标准 RDBC 参数资源转换成扩展驱动可绑定的通用值。
///
/// 该对象只允许由具体 `PhysicalPreparedStatement::set_parameter` 调用，因此读取
/// 时点仍处于物理 Adapter 边界。Ref、Array 和 vendor custom 类型保持明确
/// unsupported，不会用字符串冒充。
pub(crate) struct PreparedParameterMaterializer;

impl PreparedParameterMaterializer {
    fn stream_length(length: RdbcStreamLength) -> Result<Option<usize>, DruidError> {
        match length {
            RdbcStreamLength::Unspecified => Ok(None),
            RdbcStreamLength::Int(length) => usize::try_from(length).map(Some).map_err(|_| {
                DruidError::InvalidArgument("stream length must not be negative".to_string())
            }),
            RdbcStreamLength::Long(length) => usize::try_from(length).map(Some).map_err(|_| {
                DruidError::InvalidArgument(
                    "stream length must be non-negative and fit usize".to_string(),
                )
            }),
        }
    }

    fn character_length(length: RdbcCharacterLength) -> Result<Option<usize>, DruidError> {
        match length {
            RdbcCharacterLength::Unspecified => Ok(None),
            RdbcCharacterLength::Int(length) => usize::try_from(length).map(Some).map_err(|_| {
                DruidError::InvalidArgument("reader length must not be negative".to_string())
            }),
            RdbcCharacterLength::Long(length) => usize::try_from(length).map(Some).map_err(|_| {
                DruidError::InvalidArgument(
                    "reader length must be non-negative and fit usize".to_string(),
                )
            }),
        }
    }

    fn read_stream(
        stream: &druid::core::RdbcInputStream,
        length: RdbcStreamLength,
    ) -> Result<Vec<u8>, DruidError> {
        let Some(length) = Self::stream_length(length)? else {
            return stream.read_to_end();
        };
        let mut bytes = vec![0_u8; length];
        let mut offset = 0;
        while offset < length {
            let read = stream.read(&mut bytes[offset..])?;
            if read == 0 {
                return Err(DruidError::DriverError(format!(
                    "InputStream ended after {offset} bytes; declared length is {length}"
                )));
            }
            offset += read;
        }
        Ok(bytes)
    }

    fn read_reader(
        reader: &druid::core::RdbcReader,
        length: RdbcCharacterLength,
    ) -> Result<String, DruidError> {
        let Some(length) = Self::character_length(length)? else {
            return reader.read_to_string();
        };
        let mut code_units = vec![0_u16; length];
        let mut offset = 0;
        while offset < length {
            let read = reader.read_utf16(&mut code_units[offset..])?;
            if read == 0 {
                return Err(DruidError::DriverError(format!(
                    "Reader ended after {offset} UTF-16 units; declared length is {length}"
                )));
            }
            offset += read;
        }
        String::from_utf16(&code_units).map_err(|error| {
            DruidError::DriverError(format!("Reader contains invalid UTF-16: {error}"))
        })
    }

    fn immediate_rdbc_object(value: &RdbcObject) -> Result<Value, DruidError> {
        match value {
            RdbcObject::RowId(value) => Ok(Value::Bytes(value.bytes().to_vec())),
            RdbcObject::CharacterStream(value) | RdbcObject::NCharacterStream(value) => {
                Self::read_reader(value, RdbcCharacterLength::Unspecified).map(Value::String)
            }
            _ => PreparedInputParameter::object(Some(value.clone())).scalar_value(),
        }
    }

    async fn deferred_rdbc_object(value: &RdbcObject) -> Result<Value, DruidError> {
        match value {
            RdbcObject::SqlXml(value) => value.string().await?.to_rust_string().map(Value::String),
            RdbcObject::Blob(value) => {
                let length = i32::try_from(value.length().await?).map_err(|_| {
                    DruidError::InvalidArgument(
                        "Blob length exceeds RDBC getBytes int range".to_string(),
                    )
                })?;
                value.get_bytes(1, length).await.map(Value::Bytes)
            }
            RdbcObject::Clob(value) => {
                let length = i32::try_from(value.length().await?).map_err(|_| {
                    DruidError::InvalidArgument(
                        "Clob length exceeds RDBC getSubString int range".to_string(),
                    )
                })?;
                value
                    .get_sub_string(1, length)
                    .await?
                    .to_rust_string()
                    .map(Value::String)
            }
            RdbcObject::NClob(value) => {
                let length = i32::try_from(value.length().await?).map_err(|_| {
                    DruidError::InvalidArgument(
                        "NClob length exceeds RDBC getSubString int range".to_string(),
                    )
                })?;
                value
                    .get_sub_string(1, length)
                    .await?
                    .to_rust_string()
                    .map(Value::String)
            }
            _ => Self::immediate_rdbc_object(value),
        }
    }

    /// Materializes parameters that do not require asynchronous RDBC resource access.
    ///
    /// `None` means that the descriptor must be retained and materialized by `materialize()` at
    /// the asynchronous execution boundary.
    pub(crate) fn materialize_immediate(
        parameter: &PreparedInputParameter,
    ) -> Result<Option<Value>, DruidError> {
        if matches!(
            parameter,
            PreparedInputParameter::Blob(Some(_))
                | PreparedInputParameter::Clob(Some(_))
                | PreparedInputParameter::NClob(Some(_))
                | PreparedInputParameter::SqlXml(Some(_))
                | PreparedInputParameter::Object {
                    value: Some(
                        RdbcObject::Blob(_)
                            | RdbcObject::Clob(_)
                            | RdbcObject::NClob(_)
                            | RdbcObject::SqlXml(_)
                    ),
                    ..
                }
        ) {
            return Ok(None);
        }

        match parameter {
            PreparedInputParameter::AsciiStream { stream, length } => stream
                .as_ref()
                .map(|stream| {
                    let bytes = Self::read_stream(stream, *length)?;
                    String::from_utf8(bytes)
                        .map(Value::String)
                        .map_err(|error| {
                            DruidError::DriverError(format!(
                                "ASCII stream is not valid UTF-8: {error}"
                            ))
                        })
                })
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::UnicodeStream { stream, length } => stream
                .as_ref()
                .map(|stream| {
                    let bytes = Self::read_stream(stream, RdbcStreamLength::Int(*length))?;
                    String::from_utf8(bytes)
                        .map(Value::String)
                        .map_err(|error| {
                            DruidError::DriverError(format!(
                                "Unicode stream is not valid UTF-8: {error}"
                            ))
                        })
                })
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::BinaryStream { stream, length }
            | PreparedInputParameter::BlobStream { stream, length } => stream
                .as_ref()
                .map(|stream| Self::read_stream(stream, *length).map(Value::Bytes))
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::CharacterStream { reader, length }
            | PreparedInputParameter::NCharacterStream { reader, length }
            | PreparedInputParameter::ClobReader { reader, length }
            | PreparedInputParameter::NClobReader { reader, length } => reader
                .as_ref()
                .map(|reader| Self::read_reader(reader, *length).map(Value::String))
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::RowId(Some(value)) => {
                Ok(Some(Value::Bytes(value.bytes().to_vec())))
            }
            PreparedInputParameter::Object {
                value: Some(value), ..
            } => Self::immediate_rdbc_object(value).map(Some),
            _ => parameter.scalar_value().map(Some),
        }
    }

    /// Materializes one complete descriptor at an asynchronous execution boundary.
    pub(crate) async fn materialize(
        parameter: &PreparedInputParameter,
    ) -> Result<Value, DruidError> {
        if let Some(value) = Self::materialize_immediate(parameter)? {
            return Ok(value);
        }

        match parameter {
            PreparedInputParameter::Blob(Some(value)) => {
                Self::deferred_rdbc_object(&RdbcObject::Blob(value.clone())).await
            }
            PreparedInputParameter::Clob(Some(value)) => {
                Self::deferred_rdbc_object(&RdbcObject::Clob(value.clone())).await
            }
            PreparedInputParameter::NClob(Some(value)) => {
                Self::deferred_rdbc_object(&RdbcObject::NClob(value.clone())).await
            }
            PreparedInputParameter::SqlXml(Some(value)) => {
                value.string().await?.to_rust_string().map(Value::String)
            }
            PreparedInputParameter::Object {
                value: Some(value), ..
            } => Self::deferred_rdbc_object(value).await,
            _ => unreachable!("immediate parameters already returned"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PreparedParameterMaterializer;
    use druid::core::{
        PreparedInputParameter, RdbcCharacterLength, RdbcInputStream, RdbcObject, RdbcReader,
        RdbcRowId, RdbcStreamLength, Value,
    };

    #[tokio::test]
    async fn materializes_stream_reader_object_and_null_families_at_execution_time() {
        let ascii = RdbcInputStream::from_bytes(b"ascii-tail".to_vec());
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::AsciiStream {
                stream: Some(ascii.clone()),
                length: RdbcStreamLength::Int(5),
            })
            .await
            .unwrap(),
            Value::String("ascii".to_string())
        );
        assert_eq!(ascii.read_to_end().unwrap(), b"-tail");

        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::UnicodeStream {
                stream: Some(RdbcInputStream::from_bytes("国字".as_bytes().to_vec())),
                length: i32::try_from("国字".len()).unwrap(),
            })
            .await
            .unwrap(),
            Value::String("国字".to_string())
        );
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes([1, 2, 3])),
                length: RdbcStreamLength::Long(3),
            })
            .await
            .unwrap(),
            Value::Bytes(vec![1, 2, 3])
        );
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("A😀B")),
                length: RdbcCharacterLength::Int(3),
            })
            .await
            .unwrap(),
            Value::String("A😀".to_string())
        );
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::Object {
                value: Some(RdbcObject::NCharacterStream(RdbcReader::from_string(
                    "对象"
                ))),
                target_sql_type: None,
                scale_or_length: None,
            })
            .await
            .unwrap(),
            Value::String("对象".to_string())
        );
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::RowId(Some(
                RdbcRowId::new([7, 8])
            )))
            .await
            .unwrap(),
            Value::Bytes(vec![7, 8])
        );

        for parameter in [
            PreparedInputParameter::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            },
            PreparedInputParameter::BlobStream {
                stream: None,
                length: RdbcStreamLength::Unspecified,
            },
            PreparedInputParameter::NClobReader {
                reader: None,
                length: RdbcCharacterLength::Unspecified,
            },
            PreparedInputParameter::RowId(None),
        ] {
            assert_eq!(
                PreparedParameterMaterializer::materialize(&parameter)
                    .await
                    .unwrap(),
                Value::Null
            );
        }
    }

    #[tokio::test]
    async fn rejects_invalid_lengths_short_resources_and_invalid_encodings() {
        let cases = [
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes([1])),
                length: RdbcStreamLength::Int(-1),
            },
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes([1])),
                length: RdbcStreamLength::Long(-1),
            },
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes([1])),
                length: RdbcStreamLength::Int(2),
            },
            PreparedInputParameter::UnicodeStream {
                stream: Some(RdbcInputStream::from_bytes([0xff])),
                length: 1,
            },
        ];
        for parameter in cases {
            assert!(PreparedParameterMaterializer::materialize(&parameter)
                .await
                .is_err());
        }

        for parameter in [
            PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("x")),
                length: RdbcCharacterLength::Int(-1),
            },
            PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("x")),
                length: RdbcCharacterLength::Long(-1),
            },
            PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("x")),
                length: RdbcCharacterLength::Int(2),
            },
            PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_utf16(vec![0xd800])),
                length: RdbcCharacterLength::Int(1),
            },
        ] {
            assert!(PreparedParameterMaterializer::materialize(&parameter)
                .await
                .is_err());
        }
    }
}
