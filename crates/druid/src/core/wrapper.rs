//! JDBC Wrapper 的 Rust 平台协议。

use super::{
    PhysicalCallableStatement, PhysicalConnection, PhysicalPreparedStatement, PhysicalResultSet,
    PhysicalResultSetMetaData, PhysicalStatement,
};
use std::any::{Any, TypeId};

/// Wrapper 解包结果。
///
/// Java 的 `unwrap` 可以返回具体类或 `java.sql.Connection` 接口。Rust 的
/// `Any` 只能下转为具体类型，因此显式区分具体对象与内部连接 SPI，避免把
/// `SqlxConnectionAdapter` 的具体类型判断冒充 `Connection.class` 语义。
pub enum Unwrapped<'a> {
    /// 具体运行时对象。
    Object(&'a dyn Any),
    /// `java.sql.Connection` 对应的内部 `PhysicalConnection` SPI。
    PhysicalConnection(&'a dyn PhysicalConnection),
    /// `java.sql.PreparedStatement` 对应的物理语句 SPI。
    PreparedStatement(&'a dyn PhysicalPreparedStatement),
    /// `java.sql.Statement` 对应的物理语句 SPI。
    Statement(&'a dyn PhysicalStatement),
    /// `java.sql.ResultSet` 对应的物理结果集 SPI。
    ResultSet(&'a dyn PhysicalResultSet),
    /// `java.sql.ResultSetMetaData` 对应的物理 metadata SPI。
    ResultSetMetaData(&'a dyn PhysicalResultSetMetaData),
    /// `java.sql.CallableStatement` 对应的物理调用语句 SPI。
    CallableStatement(&'a dyn PhysicalCallableStatement),
}

impl std::fmt::Debug for Unwrapped<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Object(_) => formatter.write_str("Unwrapped::Object"),
            Self::PhysicalConnection(_) => formatter.write_str("Unwrapped::PhysicalConnection"),
            Self::PreparedStatement(_) => formatter.write_str("Unwrapped::PreparedStatement"),
            Self::Statement(_) => formatter.write_str("Unwrapped::Statement"),
            Self::ResultSet(_) => formatter.write_str("Unwrapped::ResultSet"),
            Self::ResultSetMetaData(_) => formatter.write_str("Unwrapped::ResultSetMetaData"),
            Self::CallableStatement(_) => formatter.write_str("Unwrapped::CallableStatement"),
        }
    }
}

impl<'a> Unwrapped<'a> {
    /// 尝试把具体对象下转为 `T`。
    ///
    /// `PhysicalConnection` 接口结果不会被错误地下转为具体对象。
    pub fn downcast_ref<T: Any>(&self) -> Option<&'a T> {
        match self {
            Self::Object(value) => value.downcast_ref::<T>(),
            _ => None,
        }
    }

    /// 返回物理连接接口；具体对象结果返回 `None`。
    pub fn physical_connection(&self) -> Option<&'a dyn PhysicalConnection> {
        match self {
            Self::PhysicalConnection(connection) => Some(*connection),
            Self::Object(_)
            | Self::PreparedStatement(_)
            | Self::Statement(_)
            | Self::ResultSet(_)
            | Self::ResultSetMetaData(_)
            | Self::CallableStatement(_) => None,
        }
    }

    /// 返回物理 `PreparedStatement` 接口；其他结果返回 `None`。
    pub fn prepared_statement(&self) -> Option<&'a dyn PhysicalPreparedStatement> {
        match self {
            Self::PreparedStatement(statement) => Some(*statement),
            Self::Object(_)
            | Self::PhysicalConnection(_)
            | Self::Statement(_)
            | Self::ResultSet(_)
            | Self::ResultSetMetaData(_)
            | Self::CallableStatement(_) => None,
        }
    }

    /// 返回物理 `Statement` 接口；其他结果返回 `None`。
    pub fn statement(&self) -> Option<&'a dyn PhysicalStatement> {
        match self {
            Self::Statement(statement) => Some(*statement),
            Self::Object(_)
            | Self::PhysicalConnection(_)
            | Self::PreparedStatement(_)
            | Self::ResultSet(_)
            | Self::ResultSetMetaData(_)
            | Self::CallableStatement(_) => None,
        }
    }

    /// 返回物理 `ResultSet` 接口；其他结果返回 `None`。
    pub fn result_set(&self) -> Option<&'a dyn PhysicalResultSet> {
        match self {
            Self::ResultSet(result_set) => Some(*result_set),
            Self::Object(_)
            | Self::PhysicalConnection(_)
            | Self::PreparedStatement(_)
            | Self::Statement(_)
            | Self::ResultSetMetaData(_)
            | Self::CallableStatement(_) => None,
        }
    }

    /// 返回物理 `ResultSetMetaData` 接口；其他结果返回 `None`。
    pub fn result_set_meta_data(&self) -> Option<&'a dyn PhysicalResultSetMetaData> {
        match self {
            Self::ResultSetMetaData(meta_data) => Some(*meta_data),
            Self::Object(_)
            | Self::PhysicalConnection(_)
            | Self::PreparedStatement(_)
            | Self::Statement(_)
            | Self::ResultSet(_)
            | Self::CallableStatement(_) => None,
        }
    }

    /// 返回物理 `CallableStatement` 接口；其他结果返回 `None`。
    pub fn callable_statement(&self) -> Option<&'a dyn PhysicalCallableStatement> {
        match self {
            Self::CallableStatement(statement) => Some(*statement),
            Self::Object(_)
            | Self::PhysicalConnection(_)
            | Self::PreparedStatement(_)
            | Self::Statement(_)
            | Self::ResultSet(_)
            | Self::ResultSetMetaData(_) => None,
        }
    }
}

/// 可按运行时类型识别并解包底层对象的协议。
///
/// 对应 Java: `java.sql.Wrapper`。Java 使用 `Class<?>` 作为类型令牌，
/// Rust 使用 `Option<TypeId>`：`None` 精确保留 Java 传入 `null` 的语义。
/// [`WrapperExt`] 提供面向调用者的强类型便捷方法。
pub trait Wrapper: Any + Send {
    /// 返回当前对象的运行时类型视图。
    ///
    /// 返回值用于实现 Java `Class#isInstance` 对应的强类型判断。
    fn as_any(&self) -> &dyn Any;

    /// 判断当前对象是否是目标接口或父类型的实例。
    ///
    /// 默认实现等价于具体类匹配；需要暴露 Rust trait/interface 的对象应覆盖
    /// 本方法，保留 Java `Class#isInstance` 的可赋值判断。
    fn is_instance_of(&self, iface: TypeId) -> bool {
        self.as_any().type_id() == iface
    }

    /// 判断当前对象是否可解包为指定运行时类型。
    ///
    /// 参数 `iface` 对应 Java `Wrapper#isWrapperFor(Class<?>)`；`None`
    /// 对应 Java `null`，必须返回 `false`。
    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        iface.is_some_and(|iface| self.is_instance_of(iface))
    }

    /// 解包为指定运行时类型。
    ///
    /// 参数 `iface` 对应 Java `Wrapper#unwrap(Class<T>)`；无法匹配或传入
    /// `None` 时返回 `None`，与 Druid 的 `WrapperAdapter` 行为一致。
    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        self.is_wrapper_for(iface)
            .then(|| Unwrapped::Object(self.as_any()))
    }

    /// 标记当前包装对象是否对应 Druid `WrapperProxy`。
    ///
    /// `PoolableWrapper` 必须对代理对象跳过直接实例返回，改由代理自己的
    /// `FilterChain` 解包逻辑处理。
    fn is_wrapper_proxy(&self) -> bool {
        false
    }

    /// 返回 statement 包装器持有的底层连接。
    ///
    /// 对应 Java `DruidStatementConnection#getConnection()` 特殊分支；普通
    /// Wrapper 返回 `None`。
    fn statement_connection(&self) -> Option<&dyn Any> {
        None
    }
}

/// [`Wrapper`] 的强类型调用扩展。
pub trait WrapperExt: Wrapper {
    /// 判断当前对象是否可解包为 `T`。
    ///
    /// 返回值对应 Java `isWrapperFor(T.class)`。
    fn is_wrapper_for_type<T: ?Sized + 'static>(&self) -> bool {
        self.is_wrapper_for(Some(TypeId::of::<T>()))
    }

    /// 尝试把当前对象解包为 `T` 的只读引用。
    ///
    /// 返回值对应 Java `unwrap(T.class)`；类型不匹配时返回 `None`。
    fn unwrap_ref<T: Any>(&self) -> Option<&T> {
        self.unwrap(Some(TypeId::of::<T>()))?.downcast_ref::<T>()
    }
}

impl<T> WrapperExt for T where T: Wrapper + ?Sized {}
