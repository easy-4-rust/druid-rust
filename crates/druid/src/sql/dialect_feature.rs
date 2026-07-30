//! 方言 lexer/parser 特性集合。

macro_rules! define_feature_enum {
    (
        $(#[$enum_meta:meta])*
        $name:ident {
            $($variant:ident => ($mask:expr, $java_name:literal)),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                #[doc = concat!("对应 Java `", stringify!($name), ".", $java_name, "`。")]
                $variant,
            )+
        }

        impl $name {
            /// Java 声明顺序中的全部枚举常量。
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// 返回 Java `long` mask 的无符号位表示。
            #[must_use]
            pub const fn mask(self) -> u64 {
                match self {
                    $(Self::$variant => $mask,)+
                }
            }

            /// 返回 Java 枚举常量名称。
            #[must_use]
            pub const fn java_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $java_name,)+
                }
            }

            /// 返回 Java `Enum#ordinal()`。
            #[must_use]
            pub fn ordinal(self) -> usize {
                Self::ALL
                    .iter()
                    .position(|candidate| *candidate == self)
                    .expect("feature must be declared in ALL")
            }

            /// 按 Java 枚举名称严格解析。
            #[must_use]
            pub fn value_of(name: &str) -> Option<Self> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|feature| feature.java_name() == name)
            }

            /// 判断 mask 是否启用本项。
            #[must_use]
            pub const fn is_enabled(self, features: u64) -> bool {
                features & self.mask() != 0
            }

            /// 打开或关闭本项并返回新 mask。
            #[must_use]
            pub const fn config(self, features: u64, state: bool) -> u64 {
                if state {
                    features | self.mask()
                } else {
                    features & !self.mask()
                }
            }
        }
    };
}

define_feature_enum! {
    /// Lexer 特性。对应 Java `DialectFeature.LexerFeature`。
    LexerFeature {
        ScanSqlTypeBlockComment => (1_u64, "ScanSQLTypeBlockComment"),
        ScanSqlTypeWithSemi => (1_u64 << 1, "ScanSQLTypeWithSemi"),
        ScanSqlTypeWithFrom => (1_u64 << 2, "ScanSQLTypeWithFrom"),
        ScanSqlTypeWithFunction => (1_u64 << 3, "ScanSQLTypeWithFunction"),
        ScanSqlTypeWithBegin => (1_u64 << 4, "ScanSQLTypeWithBegin"),
        ScanSqlTypeWithAt => (1_u64 << 5, "ScanSQLTypeWithAt"),
        NextTokenColon => (1_u64 << 6, "NextTokenColon"),
        NextTokenPrefixN => (1_u64 << 7, "NextTokenPrefixN"),
        ScanString2PutDoubleBackslash => (1_u64 << 8, "ScanString2PutDoubleBackslash"),
        ScanStringDoubleBackslash => (1_u64 << 8, "ScanStringDoubleBackslash"),
        ScanAliasU => (1_u64 << 9, "ScanAliasU"),
        ScanNumberPrefixB => (1_u64 << 10, "ScanNumberPrefixB"),
        ScanNumberCommonProcess => (1_u64 << 11, "ScanNumberCommonProcess"),
        ScanVariableAt => (1_u64 << 12, "ScanVariableAt"),
        ScanVariableGreaterThan => (1_u64 << 13, "ScanVariableGreaterThan"),
        ScanVariableSkipIdentifiers => (1_u64 << 14, "ScanVariableSkipIdentifiers"),
        ScanVariableMoveToSemi => (1_u64 << 15, "ScanVariableMoveToSemi"),
        ScanHiveCommentDoubleSpace => (1_u64 << 16, "ScanHiveCommentDoubleSpace"),
        ScanSubAsIdentifier => (1_u64 << 17, "ScanSubAsIdentifier"),
    }
}

define_feature_enum! {
    /// Parser 特性。对应 Java `DialectFeature.ParserFeature`。
    ParserFeature {
        AcceptUnion => (1_u64, "AcceptUnion"),
        QueryRestSemi => (1_u64 << 1, "QueryRestSemi"),
        AsofJoin => (1_u64 << 2, "AsofJoin"),
        GlobalJoin => (1_u64 << 3, "GlobalJoin"),
        JoinAt => (1_u64 << 4, "JoinAt"),
        JoinRightTableWith => (1_u64 << 5, "JoinRightTableWith"),
        JoinRightTableFrom => (1_u64 << 6, "JoinRightTableFrom"),
        JoinRightTableAlias => (1_u64 << 7, "JoinRightTableAlias"),
        PostNaturalJoin => (1_u64 << 8, "PostNaturalJoin"),
        MultipleJoinOn => (1_u64 << 9, "MultipleJoinOn"),
        Udj => (1_u64 << 10, "UDJ"),
        UserDefinedJoin => (1_u64 << 10, "UserDefinedJoin"),
        TwoConsecutiveUnion => (1_u64 << 11, "TwoConsecutiveUnion"),
        QueryTable => (1_u64 << 12, "QueryTable"),
        GroupByAll => (1_u64 << 13, "GroupByAll"),
        RewriteGroupByCubeRollupToFunction => (1_u64 << 14, "RewriteGroupByCubeRollupToFunction"),
        GroupByPostDesc => (1_u64 << 15, "GroupByPostDesc"),
        GroupByItemOrder => (1_u64 << 16, "GroupByItemOrder"),
        SqlDateExpr => (1_u64 << 17, "SQLDateExpr"),
        SqlTimestampExpr => (1_u64 << 18, "SQLTimestampExpr"),
        PrimaryVariantColon => (1_u64 << 19, "PrimaryVariantColon"),
        PrimaryBangBangSupport => (1_u64 << 20, "PrimaryBangBangSupport"),
        PrimaryTwoConsecutiveSet => (1_u64 << 21, "PrimaryTwoConsecutiveSet"),
        PrimaryLbraceOdbcEscape => (1_u64 << 22, "PrimaryLbraceOdbcEscape"),
        ParseAllIdentifier => (1_u64 << 23, "ParseAllIdentifier"),
        PrimaryRestCommaAfterLparen => (1_u64 << 24, "PrimaryRestCommaAfterLparen"),
        InRestSpecificOperation => (1_u64 << 25, "InRestSpecificOperation"),
        AdditiveRestPipesAsConcat => (1_u64 << 26, "AdditiveRestPipesAsConcat"),
        ParseAssignItemRparenCommaSetReturn => (1_u64 << 27, "ParseAssignItemRparenCommaSetReturn"),
        ParseAssignItemEqSemiReturn => (1_u64 << 28, "ParseAssignItemEqSemiReturn"),
        ParseAssignItemSkip => (1_u64 << 29, "ParseAssignItemSkip"),
        ParseAssignItemEqeq => (1_u64 << 30, "ParseAssignItemEqeq"),
        ParseSelectItemPrefixX => (1_u64 << 31, "ParseSelectItemPrefixX"),
        ParseLimitBy => (1_u64 << 32, "ParseLimitBy"),
        ParseStatementListWhen => (1_u64 << 33, "ParseStatementListWhen"),
        ParseStatementListSelectUnsupportedSyntax => (
            1_u64 << 34,
            "ParseStatementListSelectUnsupportedSyntax"
        ),
        ParseStatementListUpdatePlanCache => (1_u64 << 35, "ParseStatementListUpdatePlanCache"),
        ParseStatementListRollbackReturn => (1_u64 << 36, "ParseStatementListRollbackReturn"),
        ParseStatementListCommitReturn => (1_u64 << 37, "ParseStatementListCommitReturn"),
        ParseStatementListLparenContinue => (1_u64 << 38, "ParseStatementListLparenContinue"),
        ParseRevokeFromUser => (1_u64 << 39, "ParseRevokeFromUser"),
        ParseDropTableTables => (1_u64 << 40, "ParseDropTableTables"),
        ParseCreateSql => (1_u64 << 41, "ParseCreateSql"),
        CreateTableBodySupplemental => (1_u64 << 42, "CreateTableBodySupplemental"),
        TableAliasConnectWhere => (1_u64 << 43, "TableAliasConnectWhere"),
        TableAliasAsof => (1_u64 << 44, "TableAliasAsof"),
        TableAliasLock => (1_u64 << 45, "TableAliasLock"),
        TableAliasPartition => (1_u64 << 46, "TableAliasPartition"),
        TableAliasTable => (1_u64 << 47, "TableAliasTable"),
        TableAliasBetween => (1_u64 << 48, "TableAliasBetween"),
        TableAliasRest => (1_u64 << 49, "TableAliasRest"),
        AsCommaFrom => (1_u64 << 50, "AsCommaFrom"),
        AsSkip => (1_u64 << 51, "AsSkip"),
        AsSequence => (1_u64 << 52, "AsSequence"),
        AsDatabase => (1_u64 << 53, "AsDatabase"),
        AsDefault => (1_u64 << 54, "AsDefault"),
        AliasLiteralFloat => (1_u64 << 55, "AliasLiteralFloat"),
    }
}

/// Lexer 或 Parser 特性的 Java `Feature` 联合类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectFeatureValue {
    /// Lexer mask。
    Lexer(LexerFeature),
    /// Parser mask。
    Parser(ParserFeature),
}

/// 方言的 lexer/parser 双 mask 配置。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.DialectFeature`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialectFeature {
    lexer_feature: u64,
    parser_feature: u64,
}

impl DialectFeature {
    /// 创建 Java 默认配置。
    #[must_use]
    pub fn new() -> Self {
        let mut value = Self {
            lexer_feature: 0,
            parser_feature: 0,
        };
        value.config_features(&[
            DialectFeatureValue::Lexer(LexerFeature::ScanNumberPrefixB),
            DialectFeatureValue::Lexer(LexerFeature::ScanNumberCommonProcess),
            DialectFeatureValue::Parser(ParserFeature::AcceptUnion),
            DialectFeatureValue::Parser(ParserFeature::SqlTimestampExpr),
            DialectFeatureValue::Parser(ParserFeature::PrimaryBangBangSupport),
            DialectFeatureValue::Parser(ParserFeature::AdditiveRestPipesAsConcat),
            DialectFeatureValue::Parser(ParserFeature::ParseStatementListSelectUnsupportedSyntax),
        ]);
        value
    }

    /// 对应 Java 的两个可空 List 构造器：先应用默认值，再打开和关闭指定项。
    #[must_use]
    pub fn with_lists(
        config_features: Option<&[DialectFeatureValue]>,
        unconfig_features: Option<&[DialectFeatureValue]>,
    ) -> Self {
        let mut value = Self::new();
        if let Some(features) = config_features {
            value.config_features(features);
        }
        if let Some(features) = unconfig_features {
            value.unconfig_features(features);
        }
        value
    }

    /// 对应 Java `DialectFeature(Feature...)`：从两个零 mask 开始。
    #[must_use]
    pub fn from_features(features: Option<&[DialectFeatureValue]>) -> Self {
        let mut value = Self {
            lexer_feature: 0,
            parser_feature: 0,
        };
        if let Some(features) = features {
            value.config_features(features);
        }
        value
    }

    /// 对应 Java `DialectFeature(boolean, Feature...)`：先应用默认值。
    #[must_use]
    pub fn with_enabled(enable: bool, features: Option<&[DialectFeatureValue]>) -> Self {
        let mut value = Self::new();
        if let Some(features) = features {
            for feature in features {
                value.config_feature(*feature, enable);
            }
        }
        value
    }

    /// 打开或关闭一个 lexer/parser 特性。
    pub fn config_feature(&mut self, feature: DialectFeatureValue, state: bool) {
        match feature {
            DialectFeatureValue::Lexer(feature) => {
                self.lexer_feature = feature.config(self.lexer_feature, state);
            }
            DialectFeatureValue::Parser(feature) => {
                self.parser_feature = feature.config(self.parser_feature, state);
            }
        }
    }

    /// 打开全部指定特性。
    pub fn config_features(&mut self, features: &[DialectFeatureValue]) {
        for feature in features {
            self.config_feature(*feature, true);
        }
    }

    /// 关闭全部指定特性。
    pub fn unconfig_features(&mut self, features: &[DialectFeatureValue]) {
        for feature in features {
            self.config_feature(*feature, false);
        }
    }

    /// 判断指定特性是否启用。
    #[must_use]
    pub const fn is_enabled(&self, feature: DialectFeatureValue) -> bool {
        match feature {
            DialectFeatureValue::Lexer(feature) => feature.is_enabled(self.lexer_feature),
            DialectFeatureValue::Parser(feature) => feature.is_enabled(self.parser_feature),
        }
    }

    /// 返回 lexer mask，位表示与 Java `long` 一致。
    #[must_use]
    pub const fn lexer_feature(&self) -> u64 {
        self.lexer_feature
    }

    /// 返回 parser mask，位表示与 Java `long` 一致。
    #[must_use]
    pub const fn parser_feature(&self) -> u64 {
        self.parser_feature
    }
}

impl Default for DialectFeature {
    fn default() -> Self {
        Self::new()
    }
}
