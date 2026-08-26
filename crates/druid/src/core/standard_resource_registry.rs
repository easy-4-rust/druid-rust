//! 单 logical lease 的 standard resource close registry。

use super::standard_resource_handle::StandardResourceHandle;
use super::RdbcError;
use std::sync::{Arc, Mutex};

/// 保存 holder-owned async close commands，避免连接先于子 handle 回池。
#[derive(Default)]
pub(crate) struct StandardResourceRegistry {
    resources: Mutex<Vec<Arc<dyn StandardResourceHandle>>>,
}

impl StandardResourceRegistry {
    pub(crate) fn register<T>(&self, resource: Arc<T>)
    where
        T: StandardResourceHandle + 'static,
    {
        self.resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(resource);
    }

    pub(crate) fn has_open_resources(&self) -> bool {
        self.resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|resource| !resource.is_resource_closed())
    }

    pub(crate) async fn close_all(&self) -> Result<(), RdbcError> {
        let resources = {
            let mut resources = self
                .resources
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *resources)
        };
        let mut failed = Vec::new();
        let mut first_error = None;
        for resource in resources.into_iter().rev() {
            if let Err(error) = resource.close_resource().await {
                first_error.get_or_insert(error);
                failed.push(resource);
            }
        }
        if !failed.is_empty() {
            self.resources
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend(failed.into_iter().rev());
        }
        first_error.map_or(Ok(()), Err)
    }
}
