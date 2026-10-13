use super::DruidPooledCallableStatement;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::CurrentResultDisposition;
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl DruidPooledCallableStatement {
    /// 结束逻辑调用语句，清理参数与属性后复用或关闭底层句柄。
    /// 对应 Java: com.alibaba.druid.pool.DruidPooledCallableStatement#close。
    pub(super) async fn close_logical(&mut self) -> Result<(), RdbcError> {
        let shared = Arc::clone(&self.shared);
        let _close_guard = shared.close_lock.lock().await;
        let cache_owner = shared
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let cache_key = shared
            .key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if self.statement_base.is_closed() {
            return Ok(());
        }
        let resource = Arc::clone(&self.shared.resource);
        let mut cancellation_gate = resource.cancellation_gate.lock().await;
        *cancellation_gate = false;
        // cached raw 可继续存活，但本 logical statement 的子游标必须先全部结束。
        self.statement_base
            .inner
            .dispose_result_sets(CurrentResultDisposition::CloseAll, None)
            .await?;
        if let Err(error) = self
            .shared
            .initial_properties
            .restore(&mut self.statement_base)
        {
            self.shared.cacheable.store(false, Ordering::Release);
            return Err(error);
        }
        let policy = Arc::clone(&self.statement_base.inner.policy);
        let mut statement = resource.statement.lock().await;
        let Some(raw) = statement.as_mut() else {
            resource.closed.store(true, Ordering::Release);
            return self.statement_base.inner.finish_logical_close();
        };

        let cache_enabled = match cache_owner.as_ref() {
            Some(cache) => cache.lock().await.is_enabled(),
            None => false,
        };
        let cleanup_error = match policy.classify(raw.clear_parameters(), Some(&self.shared.sql)) {
            Ok(()) => match policy.classify(raw.clear_prepared_batch(), Some(&self.shared.sql)) {
                Ok(()) => policy
                    .classify(raw.clear_batch(), Some(&self.shared.sql))
                    .err(),
                Err(error) => Some(error),
            },
            Err(error) => Some(error),
        };
        if let Some(error) = cleanup_error {
            let mut raw = statement.take().ok_or(RdbcError::ConnectionDiscarded)?;
            let close_result = policy.classify(raw.close().await, Some(&self.shared.sql));
            if close_result.is_ok() {
                if self.shared.cache_entry || cache_enabled {
                    self.shared.cache_stats.record_checked_out_cache_delete();
                } else {
                    self.shared.cache_stats.record_close();
                }
            } else if let Some(cache) = cache_owner.as_ref() {
                cache.lock().await.retain_failed_close(vec![raw]);
            }
            resource.closed.store(true, Ordering::Release);
            self.shared
                .key
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            self.shared
                .cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            drop(statement);
            // 返回原清理错误时不能发布逻辑关闭成功；registry 留待下一次 close 完成通知。
            return Err(error);
        }

        let raw_closed = raw.is_closed()?;
        if let (Some(key), Some(cache)) = (cache_key.as_ref(), cache_owner.clone()) {
            if self
                .statement_base
                .inner
                .lease_active
                .load(Ordering::Acquire)
                && cache_enabled
                && self.shared.cacheable.load(Ordering::Acquire)
                && !raw_closed
            {
                let raw = statement.take().ok_or(RdbcError::ConnectionDiscarded)?;
                resource.closed.store(true, Ordering::Release);
                drop(statement);
                let removed = cache.lock().await.put_callable(key.clone(), raw);
                self.shared.cache_stats.record_cache_insert();
                self.shared
                    .key
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                self.shared
                    .cache
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                let mut first_error = None;
                let mut failed = Vec::new();
                for mut evicted in removed {
                    if let Err(error) = evicted.close().await {
                        first_error.get_or_insert(error);
                        failed.push(evicted);
                    }
                    self.shared.cache_stats.record_cache_delete();
                }
                if !failed.is_empty() {
                    cache.lock().await.retain_failed_close(failed);
                }
                self.statement_base.inner.finish_logical_close()?;
                return first_error.map_or(Ok(()), Err);
            }
        }

        if !raw_closed {
            raw.close().await?;
            if self.shared.cache_entry {
                self.shared.cache_stats.record_checked_out_cache_delete();
            } else if cache_enabled && !self.shared.cacheable.load(Ordering::Acquire) {
                // Java 对从未入缓存的执行失败语句也递减；已借出条目此前已递减，不能混用。
                self.shared.cache_stats.record_cache_delete();
            } else if cache_enabled
                && !self
                    .statement_base
                    .inner
                    .lease_active
                    .load(Ordering::Acquire)
            {
                self.shared.cache_stats.record_checked_out_cache_delete();
            } else {
                self.shared.cache_stats.record_close();
            }
        }
        statement.take();
        resource.closed.store(true, Ordering::Release);
        self.shared
            .key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        self.shared
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        drop(statement);
        self.statement_base.inner.finish_logical_close()
    }
}
