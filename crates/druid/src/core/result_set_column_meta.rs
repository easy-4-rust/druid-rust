//! JDBC 结果集单列 metadata 描述。
//!
//! 对应 Java 平台对象 `java.sql.ResultSetMetaData` 的单列返回值集合。该 Rust
//! 描述符保留 driver 已知字段；未知 origin/shape 使用空值或零，不从 SQL 文本猜测。

use super::{ResultSetColumnType, ResultSetNullability};

/// 单列 metadata 描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultSetColumnMeta {
    pub(crate) label: String,
    pub(crate) name: String,
    pub(crate) schema_name: String,
    pub(crate) table_name: String,
    pub(crate) catalog_name: String,
    pub(crate) column_type: ResultSetColumnType,
    pub(crate) type_name: String,
    pub(crate) class_name: String,
    pub(crate) nullability: ResultSetNullability,
    pub(crate) auto_increment: bool,
    pub(crate) case_sensitive: bool,
    pub(crate) searchable: bool,
    pub(crate) currency: bool,
    pub(crate) signed: bool,
    pub(crate) display_size: usize,
    pub(crate) precision: usize,
    pub(crate) scale: usize,
    pub(crate) read_only: bool,
    pub(crate) writable: bool,
    pub(crate) definitely_writable: bool,
}

impl ResultSetColumnMeta {
    /// 使用 eager Adapter 可无损提供的标签、类型和布尔可空性创建列描述。
    pub fn new(label: impl Into<String>, column_type: ResultSetColumnType, nullable: bool) -> Self {
        let label = label.into();
        Self {
            name: label.clone(),
            label,
            schema_name: String::new(),
            table_name: String::new(),
            catalog_name: String::new(),
            column_type,
            type_name: column_type.type_name().to_string(),
            class_name: column_type.class_name().to_string(),
            nullability: if nullable {
                ResultSetNullability::Nullable
            } else {
                ResultSetNullability::NoNulls
            },
            auto_increment: false,
            case_sensitive: matches!(column_type, ResultSetColumnType::Text),
            searchable: true,
            currency: false,
            signed: column_type.is_signed(),
            display_size: 0,
            precision: 0,
            scale: 0,
            read_only: true,
            writable: false,
            definitely_writable: false,
        }
    }

    /// 覆盖列名及 schema/table/catalog 来源。
    pub fn with_origin(
        mut self,
        name: impl Into<String>,
        schema_name: impl Into<String>,
        table_name: impl Into<String>,
        catalog_name: impl Into<String>,
    ) -> Self {
        self.name = name.into();
        self.schema_name = schema_name.into();
        self.table_name = table_name.into();
        self.catalog_name = catalog_name.into();
        self
    }

    /// 覆盖 driver/vendor 类型名和 Java 类名。
    pub fn with_type_identity(
        mut self,
        type_name: impl Into<String>,
        class_name: impl Into<String>,
    ) -> Self {
        self.type_name = type_name.into();
        self.class_name = class_name.into();
        self
    }

    /// 覆盖显示宽度、精度和 scale。
    pub fn with_shape(mut self, display_size: usize, precision: usize, scale: usize) -> Self {
        self.display_size = display_size;
        self.precision = precision;
        self.scale = scale;
        self
    }

    /// 覆盖三态可空性。
    pub fn with_nullability(mut self, nullability: ResultSetNullability) -> Self {
        self.nullability = nullability;
        self
    }

    /// 覆盖 JDBC 布尔属性，参数顺序与本对象字段顺序一致。
    #[allow(clippy::too_many_arguments)]
    pub fn with_flags(
        mut self,
        auto_increment: bool,
        case_sensitive: bool,
        searchable: bool,
        currency: bool,
        signed: bool,
        read_only: bool,
        writable: bool,
        definitely_writable: bool,
    ) -> Self {
        self.auto_increment = auto_increment;
        self.case_sensitive = case_sensitive;
        self.searchable = searchable;
        self.currency = currency;
        self.signed = signed;
        self.read_only = read_only;
        self.writable = writable;
        self.definitely_writable = definitely_writable;
        self
    }
}
