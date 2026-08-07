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

    fn rdbc_object(value: &RdbcObject) -> Result<Value, DruidError> {
        match value {
            RdbcObject::RowId(value) => Ok(Value::Bytes(value.bytes().to_vec())),
            RdbcObject::SqlXml(value) => value.string()?.to_rust_string().map(Value::String),
            RdbcObject::Blob(value) => {
                let length = i32::try_from(value.length()?).map_err(|_| {
                    DruidError::InvalidArgument(
                        "Blob length exceeds RDBC getBytes int range".to_string(),
                    )
                })?;
                value.get_bytes(1, length).map(Value::Bytes)
            }
            RdbcObject::Clob(value) => {
                let length = i32::try_from(value.length()?).map_err(|_| {
                    DruidError::InvalidArgument(
                        "Clob length exceeds RDBC getSubString int range".to_string(),
                    )
                })?;
                value
                    .get_sub_string(1, length)?
                    .to_rust_string()
                    .map(Value::String)
            }
            RdbcObject::NClob(value) => {
                let length = i32::try_from(value.length()?).map_err(|_| {
                    DruidError::InvalidArgument(
                        "NClob length exceeds RDBC getSubString int range".to_string(),
                    )
                })?;
                value
                    .get_sub_string(1, length)?
                    .to_rust_string()
                    .map(Value::String)
            }
            RdbcObject::CharacterStream(value) | RdbcObject::NCharacterStream(value) => {
                Self::read_reader(value, RdbcCharacterLength::Unspecified).map(Value::String)
            }
            _ => PreparedInputParameter::object(Some(value.clone())).scalar_value(),
        }
    }

    /// 在物理 setter 边界物化一个完整参数描述符。
    pub(crate) fn materialize(parameter: &PreparedInputParameter) -> Result<Value, DruidError> {
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
                .map(|value| value.unwrap_or(Value::Null)),
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
                .map(|value| value.unwrap_or(Value::Null)),
            PreparedInputParameter::BinaryStream { stream, length }
            | PreparedInputParameter::BlobStream { stream, length } => stream
                .as_ref()
                .map(|stream| Self::read_stream(stream, *length).map(Value::Bytes))
                .transpose()
                .map(|value| value.unwrap_or(Value::Null)),
            PreparedInputParameter::CharacterStream { reader, length }
            | PreparedInputParameter::NCharacterStream { reader, length }
            | PreparedInputParameter::ClobReader { reader, length }
            | PreparedInputParameter::NClobReader { reader, length } => reader
                .as_ref()
                .map(|reader| Self::read_reader(reader, *length).map(Value::String))
                .transpose()
                .map(|value| value.unwrap_or(Value::Null)),
            PreparedInputParameter::Blob(Some(value)) => {
                Self::rdbc_object(&RdbcObject::Blob(value.clone()))
            }
            PreparedInputParameter::Clob(Some(value)) => {
                Self::rdbc_object(&RdbcObject::Clob(value.clone()))
            }
            PreparedInputParameter::NClob(Some(value)) => {
                Self::rdbc_object(&RdbcObject::NClob(value.clone()))
            }
            PreparedInputParameter::RowId(Some(value)) => Ok(Value::Bytes(value.bytes().to_vec())),
            PreparedInputParameter::SqlXml(Some(value)) => {
                value.string()?.to_rust_string().map(Value::String)
            }
            PreparedInputParameter::Object {
                value: Some(value), ..
            } => Self::rdbc_object(value),
            _ => parameter.scalar_value(),
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

    #[test]
    fn materializes_stream_reader_object_and_null_families_at_setter_time() {
        let ascii = RdbcInputStream::from_bytes(b"ascii-tail".to_vec());
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::AsciiStream {
                stream: Some(ascii.clone()),
                length: RdbcStreamLength::Int(5),
            })
            .unwrap(),
            Value::String("ascii".to_string())
        );
        assert_eq!(ascii.read_to_end().unwrap(), b"-tail");

        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::UnicodeStream {
                stream: Some(RdbcInputStream::from_bytes("国字".as_bytes().to_vec())),
                length: i32::try_from("国字".len()).unwrap(),
            })
            .unwrap(),
            Value::String("国字".to_string())
        );
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes([1, 2, 3])),
                length: RdbcStreamLength::Long(3),
            })
            .unwrap(),
            Value::Bytes(vec![1, 2, 3])
        );
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::CharacterStream {
                reader: Some(RdbcReader::from_string("A😀B")),
                length: RdbcCharacterLength::Int(3),
            })
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
            .unwrap(),
            Value::String("对象".to_string())
        );
        assert_eq!(
            PreparedParameterMaterializer::materialize(&PreparedInputParameter::RowId(Some(
                RdbcRowId::new([7, 8])
            )))
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
                PreparedParameterMaterializer::materialize(&parameter).unwrap(),
                Value::Null
            );
        }
    }

    #[test]
    fn rejects_invalid_lengths_short_resources_and_invalid_encodings() {
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
            assert!(PreparedParameterMaterializer::materialize(&parameter).is_err());
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
            assert!(PreparedParameterMaterializer::materialize(&parameter).is_err());
        }
    }
}
