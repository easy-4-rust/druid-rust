use druid::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use druid_rdbc::types::RdbcCalendarArgument;
use std::sync::{Arc, Mutex};
/// 从外部crate实现完整读取hook并观察默认value委托。对应 Java: com.alibaba.druid.filter.Filter。
pub struct OrderedReadFilter {
    pub name: &'static str,
    pub short_circuit: bool,
    pub log: Arc<Mutex<Vec<String>>>,
}
impl ResultSetFilter for OrderedReadFilter {
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:read:before", self.name));
        let result = if self.short_circuit && label.is_some() {
            assert_eq!(column, None);
            assert_eq!(
                request,
                &ResultSetRead::Timestamp(RdbcCalendarArgument::Specified(None))
            );
            if label == Some("fail") {
                Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                    "external filtered failure".to_owned(),
                )))
            } else {
                assert_eq!(label, Some("virtual"));
                Ok(Value::Timestamp(
                    chrono::NaiveDate::from_ymd_opt(2031, 1, 3)
                        .unwrap()
                        .and_hms_opt(3, 4, 5)
                        .unwrap(),
                ))
            }
        } else {
            next()
        };
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:read:after", self.name));
        result
    }
    fn standard_result_set_value_before(
        &self,
        _context: &ResultSetFilterContext,
        column: ColumnIndex,
    ) -> Result<(), DruidError> {
        assert_eq!(column.get(), 1);
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:value:before", self.name));
        Ok(())
    }
    fn standard_result_set_value_after(
        &self,
        _context: &ResultSetFilterContext,
        column: ColumnIndex,
        result: &mut Result<Value, DruidError>,
    ) -> Result<(), DruidError> {
        assert_eq!(column.get(), 1);
        assert_eq!(result, &Ok(Value::Int(1)));
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:value:after", self.name));
        Ok(())
    }
}
