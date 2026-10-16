//! 标准缓冲结果的内存或磁盘行存储。

use druid_rdbc::error::RdbcError;
use druid_rdbc::types::{Row, Value};

#[cfg(feature = "jdbc-agent")]
use crate::jdbc_agent::AgentValue;
#[cfg(feature = "jdbc-agent")]
use std::fs::File;
#[cfg(feature = "jdbc-agent")]
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(feature = "jdbc-agent")]
use std::sync::Mutex;

/// 为标准结果集保存行；Callable 多结果集可使用有预算的磁盘暂存。
/// 对应 Java: java.sql.ResultSet 的客户端缓冲行。
pub(crate) enum StandardResultRows {
    Memory(Vec<Row>),
    #[cfg(feature = "jdbc-agent")]
    Spool {
        data: Mutex<File>,
        index: Mutex<File>,
        cache: Mutex<Option<(usize, Row)>>,
        count: usize,
    },
}

impl StandardResultRows {
    pub(crate) fn from_rows(rows: Vec<Row>) -> Self {
        Self::Memory(rows)
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Memory(rows) => rows.len(),
            #[cfg(feature = "jdbc-agent")]
            Self::Spool { count, .. } => *count,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn value(&self, position: usize, column: usize) -> Result<Option<Value>, RdbcError> {
        match self {
            Self::Memory(rows) => Ok(rows
                .get(position)
                .and_then(|row| row.values.get(column))
                .cloned()),
            #[cfg(feature = "jdbc-agent")]
            Self::Spool { cache, .. } => {
                let cached = cache
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some((cached_position, row)) = cached.as_ref() {
                    if *cached_position == position {
                        return Ok(row.values.get(column).cloned());
                    }
                }
                drop(cached);
                Ok(self
                    .row(position)?
                    .and_then(|row| row.values.get(column).cloned()))
            }
        }
    }

    #[cfg(feature = "jdbc-agent")]
    pub(crate) fn row(&self, position: usize) -> Result<Option<Row>, RdbcError> {
        match self {
            Self::Memory(rows) => Ok(rows.get(position).cloned()),
            #[cfg(feature = "jdbc-agent")]
            Self::Spool {
                data,
                index,
                cache,
                count,
            } => {
                if position >= *count {
                    return Ok(None);
                }
                let mut cached = cache
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some((cached_position, row)) = cached.as_ref() {
                    if *cached_position == position {
                        return Ok(Some(row.clone()));
                    }
                }
                let index_offset = u64::try_from(position)
                    .ok()
                    .and_then(|position| position.checked_mul(8))
                    .ok_or_else(|| Self::io_error("spool index overflow"))?;
                let mut index = index
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                index
                    .seek(SeekFrom::Start(index_offset))
                    .map_err(Self::io_error)?;
                let mut offset = [0_u8; 8];
                index.read_exact(&mut offset).map_err(Self::io_error)?;
                drop(index);
                let mut data = data
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                data.seek(SeekFrom::Start(u64::from_le_bytes(offset)))
                    .map_err(Self::io_error)?;
                let mut length = [0_u8; 4];
                data.read_exact(&mut length).map_err(Self::io_error)?;
                let mut encoded = vec![0_u8; u32::from_le_bytes(length) as usize];
                data.read_exact(&mut encoded).map_err(Self::io_error)?;
                let values: Vec<AgentValue> = serde_json::from_slice(&encoded)
                    .map_err(|error| Self::io_error(error.to_string()))?;
                let values = values
                    .into_iter()
                    .map(AgentValue::into_druid)
                    .collect::<Result<Vec<_>, _>>()?;
                let row = Row::new(values);
                *cached = Some((position, row.clone()));
                Ok(Some(row))
            }
        }
    }

    #[cfg(feature = "jdbc-agent")]
    pub(crate) fn new_spool() -> Result<Self, RdbcError> {
        Ok(Self::Spool {
            data: Mutex::new(tempfile::tempfile().map_err(Self::io_error)?),
            index: Mutex::new(tempfile::tempfile().map_err(Self::io_error)?),
            cache: Mutex::new(None),
            count: 0,
        })
    }

    /// 写入一行 Agent 协议值；扣除该次执行的剩余磁盘预算。
    /// 对应 Java: ResultSet 的完整行缓冲与资源耗尽错误。
    #[cfg(feature = "jdbc-agent")]
    pub(crate) fn push_agent_row(
        &mut self,
        values: Vec<AgentValue>,
        remaining_budget: &mut u64,
    ) -> Result<(), RdbcError> {
        let Self::Spool {
            data, index, count, ..
        } = self
        else {
            return Err(Self::io_error("spool storage required"));
        };
        let encoded =
            serde_json::to_vec(&values).map_err(|error| Self::io_error(error.to_string()))?;
        let length = u32::try_from(encoded.len())
            .map_err(|_| Self::io_error("spool row exceeds protocol size"))?;
        let increment = u64::from(length) + 4 + 8;
        if increment > *remaining_budget {
            return Err(RdbcError::DriverError(
                "JDBC Agent callable result spool budget exceeded (1 GiB)".to_owned(),
            ));
        }
        let data = data
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = index
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let offset = data.seek(SeekFrom::End(0)).map_err(Self::io_error)?;
        data.write_all(&length.to_le_bytes())
            .map_err(Self::io_error)?;
        data.write_all(&encoded).map_err(Self::io_error)?;
        index
            .write_all(&offset.to_le_bytes())
            .map_err(Self::io_error)?;
        *count += 1;
        *remaining_budget -= increment;
        Ok(())
    }

    #[cfg(feature = "jdbc-agent")]
    fn io_error(error: impl std::fmt::Display) -> RdbcError {
        RdbcError::DriverError(format!("JDBC Agent callable result spool: {error}"))
    }
}

#[cfg(all(test, feature = "jdbc-agent"))]
mod tests {
    use super::{AgentValue, StandardResultRows};
    use druid_rdbc::types::Value;

    #[test]
    fn spool_reads_rows_by_offset_and_enforces_shared_budget() {
        let mut rows = StandardResultRows::new_spool().expect("create spool");
        let mut budget = 1024;
        rows.push_agent_row(vec![AgentValue::Int(7)], &mut budget)
            .expect("first row");
        rows.push_agent_row(vec![AgentValue::String("second".to_owned())], &mut budget)
            .expect("second row");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.row(1).unwrap().unwrap().values,
            vec![Value::String("second".to_owned())]
        );
        assert_eq!(rows.row(0).unwrap().unwrap().values, vec![Value::Int(7)]);
        assert!(rows.row(2).unwrap().is_none());

        let before = budget;
        budget = 0;
        let error = rows
            .push_agent_row(vec![AgentValue::Int(9)], &mut budget)
            .expect_err("budget must reject additional disk use");
        assert!(error.to_string().contains("spool budget exceeded"));
        assert_eq!(rows.len(), 2);
        assert!(before > 0);
    }
}
