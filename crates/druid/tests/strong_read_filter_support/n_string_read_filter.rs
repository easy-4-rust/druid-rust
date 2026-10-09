use druid::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use std::sync::{Arc, Mutex};

/// 外部实现者验证 national 描述符、短路及默认链的顺序。对应 Java: com.alibaba.druid.filter.Filter。
pub struct NStringReadFilter {
    pub name: &'static str,
    pub short_circuit: bool,
    pub log: Arc<Mutex<Vec<String>>>,
}

impl ResultSetFilter for NStringReadFilter {
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
            .push(format!("{}:read:{request:?}:before", self.name));
        let result = if self.short_circuit
            && matches!(request, ResultSetRead::NString)
            && matches!(label, Some("virtual" | "fail"))
        {
            assert_eq!(column, None);
            if label == Some("fail") {
                Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                    "external NString failure".to_owned(),
                )))
            } else {
                Ok(Value::String("外部短路😀".to_owned()))
            }
        } else {
            next()
        };
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:read:{request:?}:after", self.name));
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
        _result: &mut Result<Value, DruidError>,
    ) -> Result<(), DruidError> {
        assert_eq!(column.get(), 1);
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:value:after", self.name));
        Ok(())
    }
}
