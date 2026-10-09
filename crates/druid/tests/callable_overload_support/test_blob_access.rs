#![allow(dead_code, unused_imports)]
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    RdbcArrayAccess, RdbcBlob, RdbcBlobAccess, RdbcClob, RdbcClobAccess, RdbcInputStream,
    RdbcNClob, RdbcNClobAccess, RdbcOutputStream, RdbcReader, RdbcRefAccess, RdbcResourceAccess,
    RdbcSqlXmlAccess, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
    ResourceHandleFactory, ResourceOperations,
};
use druid_rdbc::types::{RdbcObject, RdbcString, RdbcTypeMap, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
/// 保留原始 Callable 测试资源逻辑。对应 Java: java.sql.CallableStatement。
/// Callable 契约测试使用的只读物理 Blob。
#[derive(Debug)]
pub(crate) struct TestBlobAccess {
    bytes: Vec<u8>,
    freed: AtomicBool,
}

impl TestBlobAccess {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.freed.load(Ordering::Acquire) {
            Err(RdbcError::DriverError("Blob has been freed".to_string()))
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for TestBlobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::blob()
    }

    async fn free(&self) -> Result<(), RdbcError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcBlobAccess for TestBlobAccess {
    async fn length(&self) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        Ok(i64::try_from(self.bytes.len()).expect("test Blob length fits i64"))
    }

    async fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        self.ensure_open()?;
        let start = usize::try_from(position - 1)
            .map_err(|_| RdbcError::DriverError("invalid Blob position".to_string()))?;
        let length = usize::try_from(length)
            .map_err(|_| RdbcError::DriverError("invalid Blob length".to_string()))?;
        let end = start
            .checked_add(length)
            .map(|end| end.min(self.bytes.len()))
            .ok_or_else(|| RdbcError::DriverError("Blob range overflow".to_string()))?;
        self.bytes
            .get(start..end)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| RdbcError::DriverError("invalid Blob range".to_string()))
    }

    async fn get_binary_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        self.ensure_open()?;
        Ok(RdbcInputStream::from_bytes(self.bytes.clone()))
    }

    async fn position_bytes(&self, pattern: &[u8], start: i64) -> Result<Option<i64>, RdbcError> {
        self.ensure_open()?;
        let start = usize::try_from(start - 1)
            .map_err(|_| RdbcError::DriverError("invalid Blob position".to_string()))?;
        Ok(self
            .bytes
            .get(start..)
            .and_then(|bytes| {
                bytes
                    .windows(pattern.len())
                    .position(|window| window == pattern)
            })
            .map(|position| i64::try_from(start + position + 1).expect("test position fits i64")))
    }

    async fn position_blob(
        &self,
        pattern: &RdbcBlob,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        let length = i32::try_from(pattern.length().await?)
            .map_err(|_| RdbcError::DriverError("test pattern is too large".to_string()))?;
        let bytes = pattern.get_bytes(1, length).await?;
        self.position_bytes(&bytes, start).await
    }

    async fn set_bytes(&self, _position: i64, _bytes: &[u8]) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_blob_set_bytes",
        })
    }

    async fn set_bytes_range(
        &self,
        _position: i64,
        _bytes: &[u8],
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_blob_set_bytes_range",
        })
    }

    async fn set_binary_stream(&self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_blob_set_binary_stream",
        })
    }

    async fn truncate(&self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_blob_truncate",
        })
    }

    async fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        let length = i32::try_from(length)
            .map_err(|_| RdbcError::DriverError("invalid Blob range length".to_string()))?;
        Ok(RdbcInputStream::from_bytes(
            self.get_bytes(position, length).await?,
        ))
    }
}

pub(crate) fn test_blob(bytes: impl Into<Vec<u8>>) -> RdbcBlob {
    ResourceHandleFactory::blob(Arc::new(TestBlobAccess {
        bytes: bytes.into(),
        freed: AtomicBool::new(false),
    }))
}
