//! INSERT values 流式处理协议。

use crate::core::{DruidError, RdbcObject, RdbcString};
use bigdecimal::BigDecimal;
use num_bigint::BigInt;

/// Java `Number` 在 INSERT integer lexer 路径中的实际返回域。
///
/// `Lexer#integerValue()` 按范围依次返回 Integer、Long 或 BigInteger，三种身份
/// 不能压成 i64。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlInsertNumber {
    /// Java Integer。
    Integer(i32),
    /// Java Long。
    Long(i64),
    /// Java `BigInteger`。
    BigInteger(BigInt),
}

/// `processFunction(..., Object...)` 的无损参数值域。
#[derive(Debug, Clone, PartialEq)]
pub enum SqlInsertFunctionValue {
    /// Java String，保持 UTF-16 code unit。
    String(RdbcString),
    /// Java Number。
    Number(SqlInsertNumber),
    /// Java `BigDecimal`。
    Decimal(BigDecimal),
    /// Java Boolean。
    Boolean(bool),
    /// Java Date 及其 SQL 子类的 epoch 毫秒。
    DateMillis(i64),
    /// Java null。
    Null,
    /// 其他 RDBC/vendor Object。
    Object(RdbcObject),
}

/// 流式处理 INSERT VALUES 的调用方协议。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.SQLInsertValueHandler`。关联 `Row`
/// 映射 Java Object 行身份；所有方法保持原 index、重载类型及 `SQLException`
/// 传播。字符串使用 UTF-16 `RdbcString`，不得因 Rust UTF-8 丢失 surrogate。
pub trait SqlInsertValueHandler {
    /// 调用方拥有的单行对象。
    type Row;

    /// 创建新行。
    fn new_row(&mut self) -> Result<Self::Row, DruidError>;

    /// 处理 Integer/Long/BigInteger。
    fn process_integer(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: SqlInsertNumber,
    ) -> Result<(), DruidError>;

    /// 处理字符串 literal。
    fn process_string(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: RdbcString,
    ) -> Result<(), DruidError>;

    /// 处理 `DATE '...'` 字符串重载。
    fn process_date_string(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: RdbcString,
    ) -> Result<(), DruidError>;

    /// 处理 `java.util.Date` 重载；值为 Unix epoch 毫秒。
    fn process_date_millis(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: i64,
    ) -> Result<(), DruidError>;

    /// 处理 `TIMESTAMP '...'` 字符串重载。
    fn process_timestamp_string(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: RdbcString,
    ) -> Result<(), DruidError>;

    /// 处理 timestamp 的 `java.util.Date` 重载；值为 Unix epoch 毫秒。
    fn process_timestamp_millis(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: i64,
    ) -> Result<(), DruidError>;

    /// 处理 `TIME '...'`。
    fn process_time(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: RdbcString,
    ) -> Result<(), DruidError>;

    /// 处理 `BigDecimal`。
    fn process_decimal(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: BigDecimal,
    ) -> Result<(), DruidError>;

    /// 处理 boolean。
    fn process_boolean(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        value: bool,
    ) -> Result<(), DruidError>;

    /// 处理 SQL NULL。
    fn process_null(&mut self, row: &mut Self::Row, index: i32) -> Result<(), DruidError>;

    /// 处理函数表达式。
    ///
    /// `values` 保留 Java Object varargs 的标量、日期、资源和 vendor custom
    /// 动态身份；当前 Java生产链常用空参数和字符串参数，但协议不把它收窄。
    fn process_function(
        &mut self,
        row: &mut Self::Row,
        index: i32,
        function_name: RdbcString,
        function_name_hash_code_64: i64,
        values: Vec<SqlInsertFunctionValue>,
    ) -> Result<(), DruidError>;

    /// 完成一行；传入的是前述回调处理的同一行对象。
    fn process_row(&mut self, row: Self::Row) -> Result<(), DruidError>;

    /// 完成全部 VALUES 行。
    fn process_complete(&mut self) -> Result<(), DruidError>;
}
