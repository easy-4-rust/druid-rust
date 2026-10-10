use crate::connection::TransactionIsolation;
use crate::error::{RdbcError, SqlException};
use crate::statement::{ResultSetConcurrency, ResultSetType};
use crate::types::RdbcTypeMap;
use std::sync::{Arc, Mutex};

/// RowSet 连接、语句和游标请求属性的状态；不在 setter 中连接数据库。
/// 对应 Java: javax.sql.RowSet 的属性声明；本 Rust 扩展不替代完整 RowSet。
/// URL 与数据源保留当前值，优先最近非 null 来源；密码不参与 Debug 或序列化。
#[derive(Clone)]
pub struct RowSetProperties {
    url: Option<String>,
    data_source_name: Option<String>,
    username: Option<String>,
    password: Option<String>,
    command: Option<String>,
    preferred_url: bool,
    transaction_isolation: TransactionIsolation,
    type_map: Arc<Mutex<RdbcTypeMap>>,
    read_only: bool,
    max_field_size: i32,
    max_rows: i32,
    query_timeout: i32,
    escape_processing: bool,
    result_set_type: ResultSetType,
    concurrency: ResultSetConcurrency,
}

impl Default for RowSetProperties {
    fn default() -> Self {
        Self {
            url: None,
            data_source_name: None,
            username: None,
            password: None,
            command: None,
            preferred_url: true,
            // 接口不规定隔离默认值；此 Rust 状态采用 READ_COMMITTED 策略。
            transaction_isolation: TransactionIsolation::READ_COMMITTED,
            type_map: Arc::new(Mutex::new(RdbcTypeMap::new())),
            read_only: true,
            max_field_size: 0,
            max_rows: 0,
            query_timeout: 0,
            escape_processing: true,
            result_set_type: ResultSetType::ForwardOnly,
            concurrency: ResultSetConcurrency::ReadOnly,
        }
    }
}

impl std::fmt::Debug for RowSetProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 不把连接串、凭据、SQL 或应用 Map 内容交给诊断输出。
        f.debug_struct("RowSetProperties")
            .field("read_only", &self.read_only)
            .field("result_set_type", &self.result_set_type)
            .field("concurrency", &self.concurrency)
            .field("max_field_size", &self.max_field_size)
            .field("max_rows", &self.max_rows)
            .field("query_timeout", &self.query_timeout)
            .finish_non_exhaustive()
    }
}

impl RowSetProperties {
    /// 创建默认属性，无参数；返回尚无连接来源或 command 的状态。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回当前 URL，无参数；None 保留 Java null。对应 Java: RowSet#getUrl。
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// 保存原始 url，返回成功；非 null 更新连接优先级，不修剪或执行建连。
    /// 对应 Java: RowSet#setUrl。
    pub fn set_url(&mut self, url: Option<&str>) -> Result<(), RdbcError> {
        self.url = url.map(str::to_owned);
        if url.is_some() {
            self.preferred_url = true;
        }
        Ok(())
    }

    /// 返回当前逻辑数据源名，无参数；None 保留 Java null。
    /// 对应 Java: RowSet#getDataSourceName。
    pub fn data_source_name(&self) -> Option<&str> {
        self.data_source_name.as_deref()
    }

    /// 保存原始 name，返回成功；非 null 更新优先级，不执行命名服务查询。
    /// 对应 Java: RowSet#setDataSourceName。
    pub fn set_data_source_name(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.data_source_name = name.map(str::to_owned);
        if name.is_some() {
            self.preferred_url = false;
        }
        Ok(())
    }

    /// 返回执行时首选 URL，无参数；首选数据源仍存在或没有 URL 时返回 None。
    /// Rust 扩展对应 Java: RowSet#setUrl 中最近非 null 来源的选择规则。
    pub fn preferred_connection_url(&self) -> Option<&str> {
        if self.preferred_url || self.data_source_name.is_none() {
            self.url()
        } else {
            None
        }
    }

    /// 返回执行时首选数据源名，无参数；首选 URL 仍存在或没有数据源时返回 None。
    /// 首选来源被清除时回退到另一非 null 来源，不把空字符串当成 null。
    pub fn preferred_data_source_name(&self) -> Option<&str> {
        if !self.preferred_url || self.url.is_none() {
            self.data_source_name()
        } else {
            None
        }
    }

    /// 返回当前连接用户名，无参数；保留 None、空串和原始文本。对应 Java: RowSet#getUsername。
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// 保存参数 name 的原始连接用户名，返回成功；None 清除属性，不执行 SQL 或连接操作。
    /// 对应 Java: RowSet#setUsername。
    pub fn set_username(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.username = name.map(str::to_owned);
        Ok(())
    }

    /// 返回当前连接密码，无参数；保留 None、空串和原始文本。对应 Java: RowSet#getPassword。
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// 保存参数 password 的原始连接密码，返回成功；None 清除属性，不执行 SQL 或连接操作。
    /// 对应 Java: RowSet#setPassword。
    pub fn set_password(&mut self, password: Option<&str>) -> Result<(), RdbcError> {
        self.password = password.map(str::to_owned);
        Ok(())
    }

    /// 返回当前SQL 查询命令，无参数；保留 None、空串和原始文本。对应 Java: RowSet#getCommand。
    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    /// 保存参数 cmd 的原始SQL 查询命令，返回成功；None 清除属性，不执行 SQL 或连接操作。
    /// 对应 Java: RowSet#setCommand。
    pub fn set_command(&mut self, cmd: Option<&str>) -> Result<(), RdbcError> {
        self.command = cmd.map(str::to_owned);
        Ok(())
    }

    /// 返回请求的隔离级别，无参数；不宣称驱动实际支持。对应 Java: RowSet#getTransactionIsolation。
    pub fn transaction_isolation(&self) -> TransactionIsolation {
        self.transaction_isolation
    }

    /// 校验并保存参数 level；只允许 READ_UNCOMMITTED、READ_COMMITTED、REPEATABLE_READ、SERIALIZABLE。
    /// 返回成功或 SQL 异常；失败保留原值。对应 Java: RowSet#setTransactionIsolation。
    pub fn set_transaction_isolation(
        &mut self,
        level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        if ![1, 2, 4, 8].contains(&level.jdbc_code()) {
            return Err(Self::invalid_property(
                "invalid RowSet transaction isolation",
            ));
        }
        self.transaction_isolation = level;
        Ok(())
    }

    /// 返回同一个共享可变类型映射，无参数；调用者更改映射可见，克隆不复制其身份。
    /// 对应 Java: RowSet#getTypeMap，默认 Map 为空。
    pub fn type_map(&self) -> Arc<Mutex<RdbcTypeMap>> {
        Arc::clone(&self.type_map)
    }

    /// 安装参数 map 的同一共享实例，返回成功；显式方法参数的类型映射优先级由执行层处理。
    /// 对应 Java: RowSet#setTypeMap；Rust 非 nullable 参数要求传入有效 Map。
    pub fn set_type_map(&mut self, map: Arc<Mutex<RdbcTypeMap>>) -> Result<(), RdbcError> {
        self.type_map = map;
        Ok(())
    }

    /// 返回只读请求，无参数；实际更新还须检查并发模式和 provider 能力。
    /// 对应 Java: RowSet#isReadOnly。
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// 保存参数 value 的只读请求，返回成功；不隐式提升驱动能力。
    /// 对应 Java: RowSet#setReadOnly。
    pub fn set_read_only(&mut self, value: bool) -> Result<(), RdbcError> {
        self.read_only = value;
        Ok(())
    }

    /// 返回受限字符或二进制列的最大字节数，无参数；0 表示无限。对应 Java: RowSet#getMaxFieldSize。
    pub fn max_field_size(&self) -> i32 {
        self.max_field_size
    }

    /// 保存参数 max 的受限字符或二进制列的最大字节数，返回成功或 SQL 异常；负数拒绝且原值不变。
    /// 0 表示无限，完整保留 i32 范围；执行层负责实际限制。对应 Java: RowSet#setMaxFieldSize。
    pub fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError> {
        if max < 0 {
            return Err(Self::invalid_property("negative RowSet max_field_size"));
        }
        self.max_field_size = max;
        Ok(())
    }

    /// 返回最大结果行数，无参数；0 表示无限。对应 Java: RowSet#getMaxRows。
    pub fn max_rows(&self) -> i32 {
        self.max_rows
    }

    /// 保存参数 max 的最大结果行数，返回成功或 SQL 异常；负数拒绝且原值不变。
    /// 0 表示无限，完整保留 i32 范围；执行层负责实际限制。对应 Java: RowSet#setMaxRows。
    pub fn set_max_rows(&mut self, max: i32) -> Result<(), RdbcError> {
        if max < 0 {
            return Err(Self::invalid_property("negative RowSet max_rows"));
        }
        self.max_rows = max;
        Ok(())
    }

    /// 返回查询等待秒数，无参数；0 表示无限。对应 Java: RowSet#getQueryTimeout。
    pub fn query_timeout(&self) -> i32 {
        self.query_timeout
    }

    /// 保存参数 seconds 的查询等待秒数，返回成功或 SQL 异常；负数拒绝且原值不变。
    /// 0 表示无限，完整保留 i32 范围；执行层负责实际限制。对应 Java: RowSet#setQueryTimeout。
    pub fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        if seconds < 0 {
            return Err(Self::invalid_property("negative RowSet query_timeout"));
        }
        self.query_timeout = seconds;
        Ok(())
    }

    /// 返回是否启用 escape processing，无参数，默认 true。对应 Java: RowSet#getEscapeProcessing。
    pub fn escape_processing(&self) -> bool {
        self.escape_processing
    }

    /// 保存参数 enable，返回成功；实际 JDBC escape 替换由执行驱动负责。
    /// 对应 Java: RowSet#setEscapeProcessing。
    pub fn set_escape_processing(&mut self, enable: bool) -> Result<(), RdbcError> {
        self.escape_processing = enable;
        Ok(())
    }

    /// 返回请求的游标类型，无参数；不代表已取得敏感或可滚动游标。
    /// 对应 Java: ResultSet#getType（由 RowSet 继承）。
    pub fn result_set_type(&self) -> ResultSetType {
        self.result_set_type
    }

    /// 校验参数 type_code 并保存标准游标类型，返回成功或 SQL 异常，失败不修改旧值。
    /// 对应 Java: RowSet#setType；允许 TYPE_FORWARD_ONLY/SCROLL_INSENSITIVE/SCROLL_SENSITIVE。
    pub fn set_type(&mut self, type_code: i32) -> Result<(), RdbcError> {
        let result_set_type = match type_code {
            1003 => ResultSetType::ForwardOnly,
            1004 => ResultSetType::ScrollInsensitive,
            1005 => ResultSetType::ScrollSensitive,
            _ => return Err(Self::invalid_property("invalid RowSet cursor type")),
        };
        self.result_set_type = result_set_type;
        Ok(())
    }

    /// 返回请求的并发模式，无参数；provider 未确认前不能据此宣称可更新。
    /// 对应 Java: ResultSet#getConcurrency（由 RowSet 继承）。
    pub fn concurrency(&self) -> ResultSetConcurrency {
        self.concurrency
    }

    /// 校验参数 concurrency 并保存模式，返回成功或 SQL 异常，失败保留旧值。
    /// 对应 Java: RowSet#setConcurrency；允许 CONCUR_READ_ONLY/CONCUR_UPDATABLE。
    pub fn set_concurrency(&mut self, concurrency: i32) -> Result<(), RdbcError> {
        let concurrency = match concurrency {
            1007 => ResultSetConcurrency::ReadOnly,
            1008 => ResultSetConcurrency::Updatable,
            _ => return Err(Self::invalid_property("invalid RowSet concurrency")),
        };
        self.concurrency = concurrency;
        Ok(())
    }

    fn invalid_property(message: &'static str) -> RdbcError {
        RdbcError::SqlException(Box::new(SqlException::new(
            0,
            Some("HY092".into()),
            Some(message.into()),
        )))
    }
}
