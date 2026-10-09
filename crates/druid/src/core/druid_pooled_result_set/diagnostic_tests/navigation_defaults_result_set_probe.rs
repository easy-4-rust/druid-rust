use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetNavigation};
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType,
};
use std::sync::{Arc, Mutex};
/// 记录原导航和属性原生调用，并返回固定 BASE 的完整属性值。
/// 对应 Java: java.sql.ResultSet。
#[derive(Debug)]
pub(super) struct NavigationDefaultsResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    direction: FetchDirection,
    size: i32,
    closed: bool,
    row_error: bool,
}
impl NavigationDefaultsResultSetProbe {
    /// 创建导航探针；参数为原生日志，返回未关闭的标准结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            calls,
            direction: FetchDirection::Unknown,
            size: 0,
            closed: false,
            row_error: false,
        }
    }
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }

    /// 配置原生 row 拒绝；无额外参数，返回仅导航可用的同一探针。
    pub(super) fn with_row_error(mut self) -> Self {
        self.row_error = true;
        self
    }
}
druid_rdbc::impl_standard_wrapper!(NavigationDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for NavigationDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.record("physical:next".to_owned());
        Ok(false)
    }
    async fn navigate(&mut self, navigation: ResultSetNavigation) -> Result<bool, RdbcError> {
        // 原同步导航的布尔返回和 void 状态分别通过标准描述符保留。
        let (call, present) = match navigation {
            ResultSetNavigation::Previous => ("physical:previous".to_owned(), false),
            ResultSetNavigation::BeforeFirst => ("physical:before_first".to_owned(), false),
            ResultSetNavigation::AfterLast => ("physical:after_last".to_owned(), false),
            ResultSetNavigation::First => ("physical:first".to_owned(), true),
            ResultSetNavigation::Last => ("physical:last".to_owned(), true),
            ResultSetNavigation::Absolute(row) => (format!("physical:absolute:{row}"), true),
            ResultSetNavigation::Relative(row) => (format!("physical:relative:{row}"), true),
            other => {
                return Err(RdbcError::InvalidArgument(format!(
                    "unexpected default navigation: {other:?}"
                )))
            }
        };
        self.record(call);
        Ok(present)
    }
    fn was_null(&self) -> Result<bool, RdbcError> {
        self.record("physical:was_null".to_owned());
        Ok(true)
    }
    fn is_before_first(&self) -> Result<bool, RdbcError> {
        self.record("physical:is_before_first".to_owned());
        Ok(true)
    }
    fn is_after_last(&self) -> Result<bool, RdbcError> {
        self.record("physical:is_after_last".to_owned());
        Ok(false)
    }
    fn is_first(&self) -> Result<bool, RdbcError> {
        self.record("physical:is_first".to_owned());
        Ok(false)
    }
    fn is_last(&self) -> Result<bool, RdbcError> {
        self.record("physical:is_last".to_owned());
        Ok(true)
    }
    fn row(&self) -> Result<i32, RdbcError> {
        self.record("physical:row".to_owned());
        if self.row_error {
            Err(RdbcError::UnsupportedOperation {
                operation: "result_set_row",
            })
        } else {
            Ok(7)
        }
    }
    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        self.record("physical:result_set_type".to_owned());
        Ok(ResultSetType::ScrollInsensitive)
    }
    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        self.record("physical:concurrency".to_owned());
        Ok(ResultSetConcurrency::ReadOnly)
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.record("physical:holdability".to_owned());
        Ok(ResultSetHoldability::HoldOverCommit)
    }
    fn cursor_name(&self) -> Result<Option<String>, RdbcError> {
        self.record("physical:cursor_name".to_owned());
        Ok(Some("cursor".to_owned()))
    }
    fn row_updated(&self) -> Result<bool, RdbcError> {
        self.record("physical:row_updated".to_owned());
        Ok(true)
    }
    fn row_inserted(&self) -> Result<bool, RdbcError> {
        self.record("physical:row_inserted".to_owned());
        Ok(false)
    }
    fn row_deleted(&self) -> Result<bool, RdbcError> {
        self.record("physical:row_deleted".to_owned());
        Ok(true)
    }
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.record("physical:fetch_direction".to_owned());
        Ok(self.direction)
    }
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.record("physical:fetch_size".to_owned());
        Ok(self.size)
    }
    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        self.record(format!(
            "physical:set_fetch_direction:{}",
            direction.rdbc_code()
        ));
        self.direction = direction;
        Ok(())
    }
    fn set_fetch_size(&mut self, size: i32) -> Result<(), RdbcError> {
        self.record(format!("physical:set_fetch_size:{size}"));
        self.size = size;
        Ok(())
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.record(format!("physical:find_column:{label}"));
        ColumnIndex::new(23)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.record("physical:close".to_owned());
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
