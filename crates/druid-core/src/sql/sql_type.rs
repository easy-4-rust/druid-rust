//! Druid SQL 语句分类。

macro_rules! define_sql_types {
    ($($variant:ident => $java_name:literal),+ $(,)?) => {
        /// SQL 语句的 Druid 分类。
        ///
        /// 对应 Java：`com.alibaba.druid.sql.parser.SQLType`。该分类比
        /// `sqlparser::ast::Statement` 的顶层 variant 更细，尤其区分 INSERT、
        /// SHOW/LIST 和 ALTER TABLE 子类型，因此不能合并成通用 CRUD 枚举。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u16)]
        pub enum SqlType {
            $(
                #[doc = concat!("对应 Java `SQLType.", $java_name, "`。")]
                $variant,
            )+
        }

        impl SqlType {
            /// Java 声明顺序中的全部 SQL 类型。
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// 返回 Java 枚举常量名称。
            #[must_use]
            pub const fn java_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $java_name,)+
                }
            }

            /// 返回 Java `Enum#ordinal()`。
            #[must_use]
            pub const fn ordinal(self) -> u16 {
                self as u16
            }

            /// 按 Java 枚举常量名称严格解析。
            #[must_use]
            pub fn value_of(name: &str) -> Option<Self> {
                match name {
                    $($java_name => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

define_sql_types! {
    Select => "SELECT",
    Update => "UPDATE",
    InsertSelect => "INSERT_SELECT",
    InsertIntoSelect => "INSERT_INTO_SELECT",
    InsertOverwriteSelect => "INSERT_OVERWRITE_SELECT",
    InsertValues => "INSERT_VALUES",
    InsertIntoValues => "INSERT_INTO_VALUES",
    InsertOverwriteValues => "INSERT_OVERWRITE_VALUES",
    Insert => "INSERT",
    InsertInto => "INSERT_INTO",
    InsertOverwrite => "INSERT_OVERWRITE",
    InsertMulti => "INSERT_MULTI",
    Delete => "DELETE",
    Merge => "MERGE",
    Create => "CREATE",
    Alter => "ALTER",
    Drop => "DROP",
    Truncate => "TRUNCATE",
    Replace => "REPLACE",
    Analyze => "ANALYZE",
    Explain => "EXPLAIN",
    Show => "SHOW",
    ShowTables => "SHOW_TABLES",
    ShowUsers => "SHOW_USERS",
    ShowPartitions => "SHOW_PARTITIONS",
    ShowCatalogs => "SHOW_CATALOGS",
    ShowFunctions => "SHOW_FUNCTIONS",
    ShowRole => "SHOW_ROLE",
    ShowRoles => "SHOW_ROLES",
    ShowPackage => "SHOW_PACKAGE",
    ShowPackages => "SHOW_PACKAGES",
    ShowChangelogs => "SHOW_CHANGELOGS",
    ShowAcl => "SHOW_ACL",
    ShowRecyclebin => "SHOW_RECYCLEBIN",
    ShowVariables => "SHOW_VARIABLES",
    ShowHistory => "SHOW_HISTORY",
    ShowGrant => "SHOW_GRANT",
    ShowGrants => "SHOW_GRANTS",
    ShowCreateTable => "SHOW_CREATE_TABLE",
    ShowStatistic => "SHOW_STATISTIC",
    ShowStatisticList => "SHOW_STATISTIC_LIST",
    ShowLabel => "SHOW_LABEL",
    Desc => "DESC",
    Set => "SET",
    SetProject => "SET_PROJECT",
    SetLabel => "SET_LABEL",
    DumpData => "DUMP_DATA",
    List => "LIST",
    ListUsers => "LIST_USERS",
    ListTables => "LIST_TABLES",
    ListRoles => "LIST_ROLES",
    ListTenantRoles => "LIST_TENANT_ROLES",
    ListTrustedprojects => "LIST_TRUSTEDPROJECTS",
    ListAccountproviders => "LIST_ACCOUNTPROVIDERS",
    ListTemporaryOutput => "LIST_TEMPORARY_OUTPUT",
    Who => "WHO",
    Grant => "GRANT",
    Revoke => "REVOKE",
    Commit => "COMMIT",
    Rollback => "ROLLBACK",
    Use => "USE",
    Kill => "KILL",
    Msck => "MSCK",
    AddUser => "ADD_USER",
    RemoveUser => "REMOVE_USER",
    RemoveResource => "REMOVE_RESOURCE",
    CreateUser => "CREATE_USER",
    CreateTable => "CREATE_TABLE",
    CreateTableAsSelect => "CREATE_TABLE_AS_SELECT",
    CreateView => "CREATE_VIEW",
    CreateFunction => "CREATE_FUNCTION",
    CreateRole => "CREATE_ROLE",
    CreatePackage => "CREATE_PACKAGE",
    DropUser => "DROP_USER",
    DropTable => "DROP_TABLE",
    DropView => "DROP_VIEW",
    DropMaterializedView => "DROP_MATERIALIZED_VIEW",
    DropFunction => "DROP_FUNCTION",
    DropRole => "DROP_ROLE",
    DropResource => "DROP_RESOURCE",
    AlterUser => "ALTER_USER",
    AlterTable => "ALTER_TABLE",
    AlterView => "ALTER_VIEW",
    Read => "READ",
    AddTable => "ADD_TABLE",
    AddFunction => "ADD_FUNCTION",
    AddResource => "ADD_RESOURCE",
    AddTrustedproject => "ADD_TRUSTEDPROJECT",
    AddVolume => "ADD_VOLUME",
    AddStatistic => "ADD_STATISTIC",
    AddAccountprovider => "ADD_ACCOUNTPROVIDER",
    TunnelDownload => "TUNNEL_DOWNLOAD",
    Upload => "UPLOAD",
    Whoami => "WHOAMI",
    Script => "SCRIPT",
    Count => "COUNT",
    Add => "ADD",
    Clone => "CLONE",
    Load => "LOAD",
    Install => "INSTALL",
    Unload => "UNLOAD",
    Allow => "ALLOW",
    Purge => "PURGE",
    Restore => "RESTORE",
    Exstore => "EXSTORE",
    Undo => "UNDO",
    Remove => "REMOVE",
    Empty => "EMPTY",
    AlterTableAddPartition => "ALTER_TABLE_ADD_PARTITION",
    AlterTableMergePartition => "ALTER_TABLE_MERGE_PARTITION",
    AlterTableDropPartition => "ALTER_TABLE_DROP_PARTITION",
    AlterTableRenamePartition => "ALTER_TABLE_RENAME_PARTITION",
    AlterTableSetLifecycle => "ALTER_TABLE_SET_LIFECYCLE",
    AlterTableEnableLifecycle => "ALTER_TABLE_ENABLE_LIFECYCLE",
    AlterTableDisableLifecycle => "ALTER_TABLE_DISABLE_LIFECYCLE",
    AlterTableRename => "ALTER_TABLE_RENAME",
    AlterTableAddColumn => "ALTER_TABLE_ADD_COLUMN",
    AlterTableRenameColumn => "ALTER_TABLE_RENAME_COLUMN",
    AlterTableAlterColumn => "ALTER_TABLE_ALTER_COLUMN",
    AlterTableSetTblproperties => "ALTER_TABLE_SET_TBLPROPERTIES",
    AlterTableSetComment => "ALTER_TABLE_SET_COMMENT",
    AlterTableTouch => "ALTER_TABLE_TOUCH",
    AlterTableChangeOwner => "ALTER_TABLE_CHANGE_OWNER",
    Multi => "MULTI",
    With => "WITH",
    SetUnknown => "SET_UNKNOWN",
    Unknown => "UNKNOWN",
    Error => "ERROR",
}
