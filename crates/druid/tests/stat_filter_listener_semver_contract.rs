use druid::core::DruidError;
use druid::stats::StatFilterContextListener;

/// 旧版下游监听器只实现原有事件时，新增连接事件仍须保持源码兼容。
struct LegacyListener;

impl StatFilterContextListener for LegacyListener {
    fn add_update_count(&self, _: i32) -> Result<(), DruidError> {
        Ok(())
    }
    fn add_fetch_row_count(&self, _: i32) -> Result<(), DruidError> {
        Ok(())
    }
    fn execute_before(&self, _: &str, _: bool) -> Result<(), DruidError> {
        Ok(())
    }
    fn execute_after(
        &self,
        _: Option<&str>,
        _: i64,
        _: Option<&DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }
    fn commit(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn rollback(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn pool_connect(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn pool_close(&self, _: i64) -> Result<(), DruidError> {
        Ok(())
    }
    fn result_set_open(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn result_set_close(&self, _: i64) -> Result<(), DruidError> {
        Ok(())
    }
    fn clob_open(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn blob_open(&self) -> Result<(), DruidError> {
        Ok(())
    }
}

#[test]
fn legacy_listener_remains_implementable_and_new_events_are_noops() {
    let listener: &dyn StatFilterContextListener = &LegacyListener;
    assert!(listener.connection_open().is_ok());
    assert!(listener.connection_close(17).is_ok());
}
