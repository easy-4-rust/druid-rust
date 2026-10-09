/// 同一次SQLite原生prepare提供的列标签与声明类型。
/// 对应 Java: java.sql.ResultSetMetaData#getColumnLabel/getColumnTypeName。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteQueryColumn {
    label: String,
    declared_type: Option<String>,
}

impl SqliteQueryColumn {
    /// 保存原生列标签与声明类型；参数不得使用SQL猜测或合成列名，返回拥有型快照。
    pub fn new(label: String, declared_type: Option<String>) -> Self {
        Self {
            label,
            declared_type,
        }
    }

    /// 返回原生标签，保留大小写与重复列顺序。
    pub fn label(&self) -> &str {
        &self.label
    }

    /// 返回可空的原生声明类型；表达式未提供声明类型时返回None。
    pub fn declared_type(&self) -> Option<&str> {
        self.declared_type.as_deref()
    }
}
