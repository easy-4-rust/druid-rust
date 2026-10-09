//! 原始元数据身份案例的真实记录型后端，禁止descriptor快照替代逐getter调用。
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetColumnType, ResultSetMetaData,
};
use druid_rdbc::result_set::ColumnIndex;
use std::sync::Mutex;

/// 记录原24次getter顺序和精确错误。对应 Java: java.sql.ResultSetMetaData。
#[derive(Debug)]
pub(super) struct MetadataIdentityProbe {
    calls: Mutex<Vec<&'static str>>,
    fail_method: Option<&'static str>,
}

impl MetadataIdentityProbe {
    /// 创建记录型后端；参数指定精确失败入口，返回未调用的探针。
    pub(super) fn new(fail_method: Option<&'static str>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_method,
        }
    }

    fn call<T>(&self, method: &'static str, value: T) -> Result<T, RdbcError> {
        self.calls.lock().unwrap().push(method);
        if self.fail_method == Some(method) {
            Err(RdbcError::DriverError(format!(
                "metadata probe failed at {method}"
            )))
        } else {
            Ok(value)
        }
    }

    /// 返回实际底层调用顺序，不读取或生成额外元数据。
    pub(super) fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

druid_rdbc::impl_standard_wrapper!(MetadataIdentityProbe => ResultSetMetaData);

impl ResultSetMetaData for MetadataIdentityProbe {
    fn column(&self, _column: ColumnIndex) -> Result<ColumnDescriptor, RdbcError> {
        self.call("column", ())?;
        Err(RdbcError::DriverError(
            "metadata probe requires individual getter dispatch".to_owned(),
        ))
    }
    fn column_count(&self) -> usize {
        self.call("column_count", 1)
            .expect("本案例未注入不可失败列数入口错误")
    }

    fn is_auto_increment(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_auto_increment", column_index == 1)
    }

    fn is_case_sensitive(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_case_sensitive", column_index == 1)
    }

    fn is_searchable(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_searchable", column_index == 1)
    }

    fn is_currency(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_currency", column_index == 1)
    }

    fn nullability(&self, column_index: ColumnIndex) -> Result<ColumnNullability, RdbcError> {
        let column_index = column_index.get();
        self.call(
            "nullability",
            if column_index == 1 {
                ColumnNullability::Nullable
            } else {
                ColumnNullability::Unknown
            },
        )
    }

    fn is_signed(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_signed", column_index == 1)
    }

    fn column_display_size(&self, column_index: ColumnIndex) -> Result<usize, RdbcError> {
        let column_index = column_index.get();
        self.call("column_display_size", column_index + 20)
    }

    fn column_label(&self, column_index: ColumnIndex) -> Result<String, RdbcError> {
        let column_index = column_index.get();
        self.call("column_label", format!("label_{column_index}"))
    }

    fn column_name(&self, column_index: ColumnIndex) -> Result<String, RdbcError> {
        let column_index = column_index.get();
        self.call("column_name", format!("name_{column_index}"))
    }

    fn schema_name(&self, column_index: ColumnIndex) -> Result<String, RdbcError> {
        let column_index = column_index.get();
        self.call("schema_name", format!("schema_{column_index}"))
    }

    fn precision(&self, column_index: ColumnIndex) -> Result<usize, RdbcError> {
        let column_index = column_index.get();
        self.call("precision", column_index + 30)
    }

    fn scale(&self, column_index: ColumnIndex) -> Result<usize, RdbcError> {
        let column_index = column_index.get();
        self.call("scale", column_index + 4)
    }

    fn table_name(&self, column_index: ColumnIndex) -> Result<String, RdbcError> {
        let column_index = column_index.get();
        self.call("table_name", format!("table_{column_index}"))
    }

    fn catalog_name(&self, column_index: ColumnIndex) -> Result<String, RdbcError> {
        let column_index = column_index.get();
        self.call("catalog_name", format!("catalog_{column_index}"))
    }

    fn column_type(&self, column_index: ColumnIndex) -> Result<ResultSetColumnType, RdbcError> {
        let column_index = column_index.get();
        self.call(
            "column_type",
            if column_index == 1 {
                ResultSetColumnType::Decimal
            } else {
                ResultSetColumnType::Unknown
            },
        )
    }

    fn column_type_name(&self, column_index: ColumnIndex) -> Result<String, RdbcError> {
        let column_index = column_index.get();
        self.call("column_type_name", format!("MONEY_{column_index}"))
    }

    fn is_read_only(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_read_only", column_index != 1)
    }

    fn is_writable(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_writable", column_index == 1)
    }

    fn is_definitely_writable(&self, column_index: ColumnIndex) -> Result<bool, RdbcError> {
        let column_index = column_index.get();
        self.call("is_definitely_writable", column_index == 1)
    }

    fn column_class_name(&self, column_index: ColumnIndex) -> Result<String, RdbcError> {
        let column_index = column_index.get();
        self.call(
            "column_class_name",
            format!("com.example.Money{column_index}"),
        )
    }
}
