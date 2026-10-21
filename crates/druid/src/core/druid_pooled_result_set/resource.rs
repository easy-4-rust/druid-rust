use super::*;

impl StandardResultSetResource {
    pub(crate) fn new(
        result_set: Box<dyn ResultSet>,
        filter_chain: Option<Arc<FilterChain>>,
        filter_context: Arc<ResultSetFilterContext>,
        policy: Arc<super::super::standard_logical_policy::StandardLogicalPolicy>,
    ) -> Arc<Self> {
        Arc::new(Self {
            // 在接管前保存只读摘要；不锁定或推进 raw cursor，也不延长执行借用。
            known_row_count: result_set.known_row_count(),
            view: OnceLock::new(),
            result_set: tokio::sync::Mutex::new(Some(result_set)),
            opened: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            close_error: Mutex::new(None),
            filter_chain,
            filter_context,
            policy,
            meta_data_attributes: Arc::new(ProxyAttributes::default()),
            registration: OnceLock::new(),
        })
    }

    /// 接管 driver getter 返回的同一游标的新拥有型视图，不重建逻辑身份或触发 open。
    /// 对应 Java: Statement#getResultSet；参数为当前 raw 视图，返回交接结果。
    pub(crate) async fn adopt_current_view(
        &self,
        mut raw: Box<dyn ResultSet>,
    ) -> Result<(), RdbcError> {
        let mut current = self.result_set.lock().await;
        if self.closed.load(Ordering::Acquire) {
            // 已关闭游标不得复活；正常 driver 返回的是已关闭视图。
            // 若它仍开放，则释放这次交接的拥有型资源，并保留驱动错误分类。
            let closed = self
                .policy
                .classify(raw.is_closed(), self.filter_context.sql())?;
            if !closed {
                self.policy
                    .classify(raw.close().await, self.filter_context.sql())?;
            }
            return Ok(());
        }
        // 新视图已经拥有同一 driver cursor，替换旧视图不会执行 cursor close。
        // registry、共享游标状态、Filter context 与混合结果序列继续引用本资源。
        *current = Some(raw);
        Ok(())
    }

    pub(crate) async fn next_core(&self) -> Result<bool, RdbcError> {
        self.policy.ensure_open()?;
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::Other("result set is closed".to_owned()));
        }
        let mut result_set = self.result_set.lock().await;
        // 等待资源锁期间租约可能结束，优先拒绝旧租约，再报告游标关闭。
        self.policy.ensure_open()?;
        let raw = result_set
            .as_mut()
            .ok_or_else(|| RdbcError::Other("result set is closed".to_owned()))?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_next(raw.as_mut(), &self.filter_context)
                .await
                .map_err(RdbcError::from),
            None => raw.next().await,
        };
        self.policy.classify(result, self.filter_context.sql())
    }

    /// 返回本游标实际抓取峰值，供创建语句在级联关闭后汇总；无参数。
    /// 对应 Java: DruidPooledResultSet#close 的 fetchRowPeak 交接。
    pub(crate) fn fetch_row_count(&self) -> i32 {
        self.filter_context.fetch_row_count()
    }

    pub(crate) async fn close_core(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            self.unregister_resource();
            return Ok(());
        }
        self.closing.store(true, Ordering::Release);
        let mut result_set = self.result_set.lock().await;
        let Some(raw) = result_set.as_mut() else {
            self.closed.store(true, Ordering::Release);
            self.unregister_resource();
            return Ok(());
        };
        let opened = self.opened.load(Ordering::Acquire);
        let result = match (&self.filter_chain, opened) {
            (Some(filter_chain), true) => filter_chain
                .standard_result_set_close(raw.as_mut(), &self.filter_context)
                .await
                .map_err(RdbcError::from),
            (Some(_), false) | (None, _) => raw.close().await,
        };
        if let Err(error) = self.policy.classify(result, self.filter_context.sql()) {
            *self
                .close_error
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error.clone());
            return Err(error);
        }
        self.close_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        result_set.take();
        self.closed.store(true, Ordering::Release);
        if opened {
            self.filter_context.increment_close_count();
        }
        self.unregister_resource();
        Ok(())
    }

    pub(crate) fn registry_cleanup_error(&self) -> Option<RdbcError> {
        if let Some(error) = self
            .close_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            return Some(error);
        }
        let cleanup_requested = self.registration.get().is_some_and(
            super::super::standard_resource_registration::StandardResourceRegistration::cleanup_requested,
        );
        (cleanup_requested
            || self.closing.load(Ordering::Acquire)
            || self.closed.load(Ordering::Acquire))
        .then_some(RdbcError::ConnectionDiscarded)
    }

    pub(crate) fn open_meta_data_core(
        &self,
    ) -> Result<Arc<dyn druid_rdbc::metadata::ResultSetMetaData>, RdbcError> {
        let result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        let meta_data = self
            .policy
            .classify(raw.meta_data(), self.filter_context.sql())?;
        Ok(Arc::new(ResultSetMetaDataProxy::new(
            meta_data,
            Arc::clone(&self.meta_data_attributes),
        )))
    }

    pub(crate) fn value_core(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.read_value_core(column, false)
    }

    pub(crate) fn string_value_core(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.read_value_core(column, true)
    }

    // 两类读取共用资源锁、Filter 与错误分类；区别仅在驱动末端 getter。
    pub(crate) fn read_value_core(
        &self,
        column: ColumnIndex,
        string_read: bool,
    ) -> Result<Value, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) if string_read => filter_chain
                .standard_result_set_string_value(raw.as_mut(), &self.filter_context, column)
                .map_err(RdbcError::from),
            Some(filter_chain) => filter_chain
                .standard_result_set_value(raw.as_mut(), &self.filter_context, column)
                .map_err(RdbcError::from),
            None if string_read => raw.string_value(column),
            None => raw.value(column),
        };
        self.policy.classify(result, self.filter_context.sql())
    }

    pub(crate) fn meta_data_proxy_core(&self) -> Result<ResultSetMetaDataProxy, RdbcError> {
        let mut result_set = self
            .result_set
            .try_lock()
            .map_err(|_| RdbcError::Other("result set is busy".to_owned()))?;
        let raw = result_set.as_mut().ok_or(RdbcError::ConnectionDiscarded)?;
        let result = match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .standard_result_set_meta_data(raw.as_mut(), &self.filter_context)
                .map_err(RdbcError::from),
            None => raw.meta_data(),
        };
        let meta_data = self.policy.classify(result, self.filter_context.sql())?;
        Ok(ResultSetMetaDataProxy::new(
            meta_data,
            Arc::clone(&self.meta_data_attributes),
        ))
    }
}

#[async_trait::async_trait]
impl StandardResourceHandle for StandardResultSetResource {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        self.close_core().await
    }

    fn is_resource_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn registration_slot(
        &self,
    ) -> &OnceLock<super::super::standard_resource_registration::StandardResourceRegistration> {
        &self.registration
    }
}
