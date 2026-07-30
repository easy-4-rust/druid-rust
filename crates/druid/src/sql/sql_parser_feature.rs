//! Druid SQL parser 特性位。

/// SQL 解析器特性。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.SQLParserFeature`。枚举声明顺序是
/// 公共二进制契约：每项 mask 均为 Java `1 << ordinal()`，不得按名称排序或
/// 改用独立布尔字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SqlParserFeature {
    /// 保留 INSERT values 子句原始文本。
    KeepInsertValueClauseOriginalString = 0,
    /// 保留 SELECT 列表原始文本。
    KeepSelectListOriginalString = 1,
    /// 使用 INSERT 列缓存。
    UseInsertColumnsCache = 2,
    /// 启用二元表达式分组。
    EnableSqlBinaryOpExprGroup = 3,
    /// 为参数化路径优化。
    OptimizedForParameterized = 4,
    /// 参数化优化时跳过值。
    OptimizedForForParameterizedSkipValue = 5,
    /// 保留注释。
    KeepComments = 6,
    /// 跳过注释。
    SkipComments = 7,
    /// 使用 Wall 严格模式。
    StrictForWall = 8,
    /// 识别 TDDL hint。
    TddlHint = 9,
    /// 识别 DRDS async DDL。
    DrdsAsyncDdl = 10,
    /// 识别 DRDS baseline。
    DrdsBaseline = 11,
    /// 启用 INSERT Reader 路径。
    InsertReader = 12,
    /// 忽略名称引号。
    IgnoreNameQuotes = 13,
    /// 保留名称引号。
    KeepNameQuotes = 14,
    /// 为 SELECT item 生成别名。
    SelectItemGenerateAlias = 15,
    /// 将管道符解释为字符串连接。
    PipesAsConcat = 16,
    /// 检查 INSERT value 类型。
    InsertValueCheckType = 17,
    /// 保留 INSERT native value。
    InsertValueNative = 18,
    /// 启用 current-time 表达式。
    EnableCurrentTimeExpr = 19,
    /// 启用 current-user 表达式。
    EnableCurrentUserExpr = 20,
    /// 保留源码位置。
    KeepSourceLocation = 21,
    /// 支持 Unicode code point。
    SupportUnicodeCodePoint = 22,
    /// 解析失败时输出 SQL。
    PrintSqlWhileParsingFailed = 23,
    /// 启用多 UNION。
    EnableMultiUnion = 24,
    /// 启用 Spark 兼容。
    Spark = 25,
    /// 启用 Presto 兼容。
    Presto = 26,
    /// MySQL 标准注释兼容。
    MySqlSupportStandardComment = 27,
    /// 启用模板解析。
    Template = 28,
}

impl SqlParserFeature {
    /// Java 声明顺序中的全部特性。
    pub const ALL: &'static [Self] = &[
        Self::KeepInsertValueClauseOriginalString,
        Self::KeepSelectListOriginalString,
        Self::UseInsertColumnsCache,
        Self::EnableSqlBinaryOpExprGroup,
        Self::OptimizedForParameterized,
        Self::OptimizedForForParameterizedSkipValue,
        Self::KeepComments,
        Self::SkipComments,
        Self::StrictForWall,
        Self::TddlHint,
        Self::DrdsAsyncDdl,
        Self::DrdsBaseline,
        Self::InsertReader,
        Self::IgnoreNameQuotes,
        Self::KeepNameQuotes,
        Self::SelectItemGenerateAlias,
        Self::PipesAsConcat,
        Self::InsertValueCheckType,
        Self::InsertValueNative,
        Self::EnableCurrentTimeExpr,
        Self::EnableCurrentUserExpr,
        Self::KeepSourceLocation,
        Self::SupportUnicodeCodePoint,
        Self::PrintSqlWhileParsingFailed,
        Self::EnableMultiUnion,
        Self::Spark,
        Self::Presto,
        Self::MySqlSupportStandardComment,
        Self::Template,
    ];

    /// 返回 Java `Enum#ordinal()`。
    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self as u32
    }

    /// 返回 Java `1 << ordinal()` 对应的有符号 32 位 mask。
    #[must_use]
    pub const fn mask(self) -> i32 {
        1_i32 << self as u32
    }

    /// 判断指定 mask 是否启用了该特性。
    ///
    /// 对应 Java：`SQLParserFeature#isEnabled(int, SQLParserFeature)`。
    #[must_use]
    pub const fn is_enabled(features: i32, feature: Self) -> bool {
        features & feature.mask() != 0
    }

    /// 开启或关闭指定特性并返回新 mask。
    ///
    /// 对应 Java：`SQLParserFeature#config(int, SQLParserFeature, boolean)`。
    #[must_use]
    pub const fn config(features: i32, feature: Self, state: bool) -> i32 {
        if state {
            features | feature.mask()
        } else {
            features & !feature.mask()
        }
    }

    /// 合并特性切片。
    ///
    /// 对应 Java：`SQLParserFeature#of(SQLParserFeature...)` 的非 null 分支。
    #[must_use]
    pub fn of(features: &[Self]) -> i32 {
        features
            .iter()
            .fold(0, |value, feature| value | feature.mask())
    }

    /// 合并可空特性切片；`None` 与 Java varargs 数组为 null 一样返回 0。
    #[must_use]
    pub fn of_nullable(features: Option<&[Self]>) -> i32 {
        features.map_or(0, Self::of)
    }

    /// 返回 Java 枚举常量名称，供配置和差分 fixture 使用。
    #[must_use]
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::KeepInsertValueClauseOriginalString => "KeepInsertValueClauseOriginalString",
            Self::KeepSelectListOriginalString => "KeepSelectListOriginalString",
            Self::UseInsertColumnsCache => "UseInsertColumnsCache",
            Self::EnableSqlBinaryOpExprGroup => "EnableSQLBinaryOpExprGroup",
            Self::OptimizedForParameterized => "OptimizedForParameterized",
            Self::OptimizedForForParameterizedSkipValue => "OptimizedForForParameterizedSkipValue",
            Self::KeepComments => "KeepComments",
            Self::SkipComments => "SkipComments",
            Self::StrictForWall => "StrictForWall",
            Self::TddlHint => "TDDLHint",
            Self::DrdsAsyncDdl => "DRDSAsyncDDL",
            Self::DrdsBaseline => "DRDSBaseline",
            Self::InsertReader => "InsertReader",
            Self::IgnoreNameQuotes => "IgnoreNameQuotes",
            Self::KeepNameQuotes => "KeepNameQuotes",
            Self::SelectItemGenerateAlias => "SelectItemGenerateAlias",
            Self::PipesAsConcat => "PipesAsConcat",
            Self::InsertValueCheckType => "InsertValueCheckType",
            Self::InsertValueNative => "InsertValueNative",
            Self::EnableCurrentTimeExpr => "EnableCurrentTimeExpr",
            Self::EnableCurrentUserExpr => "EnableCurrentUserExpr",
            Self::KeepSourceLocation => "KeepSourceLocation",
            Self::SupportUnicodeCodePoint => "SupportUnicodeCodePoint",
            Self::PrintSqlWhileParsingFailed => "PrintSQLWhileParsingFailed",
            Self::EnableMultiUnion => "EnableMultiUnion",
            Self::Spark => "Spark",
            Self::Presto => "Presto",
            Self::MySqlSupportStandardComment => "MySQLSupportStandardComment",
            Self::Template => "Template",
        }
    }

    /// 按 Java 枚举名称严格解析。
    #[must_use]
    pub fn value_of(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|feature| feature.java_name() == name)
    }
}
