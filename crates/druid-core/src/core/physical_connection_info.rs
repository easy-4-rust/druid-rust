//! 对应 Java：
//! `com.alibaba.druid.pool.DruidAbstractDataSource.PhysicalConnectionInfo`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidAbstractDataSource.java`。

use super::PhysicalConnection;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 物理连接创建各阶段的结果。
///
/// Java 对象把 raw connection、connect/init/validate 的单调时钟时间点以及
/// 会话/全局变量一起交给 `DruidConnectionHolder`。Rust 使用 `Instant`
/// 保留相同的阶段顺序和耗时语义，不暴露没有跨进程意义的 JVM nanoTime 数值。
pub struct PhysicalConnectionInfo {
    connection: Option<Box<dyn PhysicalConnection>>,
    connect_started_at: Instant,
    connected_at: Instant,
    initialized_at: Instant,
    validated_at: Instant,
    variables: Option<HashMap<String, Value>>,
    global_variables: Option<HashMap<String, Value>>,
    create_task_id: u64,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for PhysicalConnectionInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PhysicalConnectionInfo")
            .field("has_connection", &self.connection.is_some())
            .field("connect_span", &self.connect_span())
            .field("initialize_span", &self.initialize_span())
            .field("validate_span", &self.validate_span())
            .field("variables", &self.variables)
            .field("global_variables", &self.global_variables)
            .field("create_task_id", &self.create_task_id)
            .finish()
    }
}

impl PhysicalConnectionInfo {
    /// 从一次已经完成的 raw connection 创建结果构造阶段对象。
    ///
    /// # 参数
    /// - `connection`：新建且尚未进入任何连接池的物理连接。
    /// - `connect_started_at`：调用驱动创建连接前记录的单调时钟。
    #[must_use]
    pub fn connected(connection: Box<dyn PhysicalConnection>, connect_started_at: Instant) -> Self {
        let connected_at = Instant::now();
        Self {
            connection: Some(connection),
            connect_started_at,
            connected_at,
            initialized_at: connected_at,
            validated_at: connected_at,
            variables: None,
            global_variables: None,
            create_task_id: 0,
        }
    }

    /// 返回物理连接；连接被移交给 holder 后返回 `None`。
    #[must_use]
    pub fn physical_connection(&self) -> Option<&dyn PhysicalConnection> {
        self.connection.as_deref()
    }

    /// 返回可变物理连接所有权容器，供 factory 的 validate/close 生命周期方法使用。
    pub fn physical_connection_box_mut(&mut self) -> Option<&mut Box<dyn PhysicalConnection>> {
        self.connection.as_mut()
    }

    /// 将物理连接所有权移交给 holder。
    pub fn take_physical_connection(&mut self) -> Option<Box<dyn PhysicalConnection>> {
        self.connection.take()
    }

    /// 标记默认属性和初始化 SQL 已完成。
    pub fn mark_initialized(&mut self) {
        self.initialized_at = Instant::now();
        if self.validated_at < self.initialized_at {
            self.validated_at = self.initialized_at;
        }
    }

    /// 标记连接有效性校验已完成。
    pub fn mark_validated(&mut self) {
        self.validated_at = Instant::now();
    }

    /// 返回驱动连接阶段耗时。
    #[must_use]
    pub fn connect_span(&self) -> Duration {
        self.connected_at
            .saturating_duration_since(self.connect_started_at)
    }

    /// 返回连接成功到初始化完成的耗时。
    #[must_use]
    pub fn initialize_span(&self) -> Duration {
        self.initialized_at
            .saturating_duration_since(self.connected_at)
    }

    /// 返回初始化完成到验证完成的耗时。
    #[must_use]
    pub fn validate_span(&self) -> Duration {
        self.validated_at
            .saturating_duration_since(self.initialized_at)
    }

    /// 返回从开始连接到验证完成的总耗时。
    #[must_use]
    pub fn total_span(&self) -> Duration {
        self.validated_at
            .saturating_duration_since(self.connect_started_at)
    }

    /// 返回会话变量。
    #[must_use]
    pub fn variables(&self) -> Option<&HashMap<String, Value>> {
        self.variables.as_ref()
    }

    /// 设置会话变量；`None` 保留 Java 未启用变量初始化的状态。
    pub fn set_variables(&mut self, variables: Option<HashMap<String, Value>>) {
        self.variables = variables;
    }

    /// 取走会话变量并移交给 holder。
    pub fn take_variables(&mut self) -> Option<HashMap<String, Value>> {
        self.variables.take()
    }

    /// 返回全局变量。
    #[must_use]
    pub fn global_variables(&self) -> Option<&HashMap<String, Value>> {
        self.global_variables.as_ref()
    }

    /// 设置全局变量；`None` 保留 Java 未启用变量初始化的状态。
    pub fn set_global_variables(&mut self, variables: Option<HashMap<String, Value>>) {
        self.global_variables = variables;
    }

    /// 取走全局变量并移交给 holder。
    pub fn take_global_variables(&mut self) -> Option<HashMap<String, Value>> {
        self.global_variables.take()
    }

    /// 返回连接创建任务 ID。
    #[must_use]
    pub const fn create_task_id(&self) -> u64 {
        self.create_task_id
    }

    /// 设置连接创建任务 ID。
    pub fn set_create_task_id(&mut self, create_task_id: u64) {
        self.create_task_id = create_task_id;
    }
}
