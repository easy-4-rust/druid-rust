use std::cell::Cell;
use druid_rdbc::resource::{RdbcResourceAccess, ResourceOperations};
#[derive(Debug)]
struct LocalResource { _state: Cell<usize> }
fn assert_send<T: Send + 'static>() {}
fn main() { assert_send::<LocalResource>(); }
#[async_trait::async_trait]
impl RdbcResourceAccess for LocalResource {
    fn capabilities(&self) -> ResourceOperations { ResourceOperations::READ }
}
