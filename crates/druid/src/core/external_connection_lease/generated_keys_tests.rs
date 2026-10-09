//! 外部池租约必须调用底层的类型化准备入口，不得退回普通 prepare。

use super::ExternalConnectionLease;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, PreparedStatement, StatementOptions};
use std::sync::{Arc, Mutex};

struct RequestDriver(Arc<Mutex<Vec<(String, StatementOptions, GeneratedKeys)>>>);
druid_rdbc::impl_standard_wrapper!(RequestDriver => Connection);

#[async_trait::async_trait]
impl Connection for RequestDriver {
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: StatementOptions,
        keys: GeneratedKeys,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.0.lock().unwrap().push((sql.to_owned(), options, keys));
        Err(RdbcError::DriverError("observed raw prepare".into()))
    }
}

#[tokio::test]
async fn external_lease_forwards_every_generated_keys_mode_and_preserves_driver_error() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let raw: Box<dyn Connection> = Box::new(RequestDriver(requests.clone()));
    let mut lease = ExternalConnectionLease::new(Box::new(raw));
    let modes = vec![
        GeneratedKeys::None,
        GeneratedKeys::ReturnGeneratedKeys,
        GeneratedKeys::ColumnIndexes(vec![
            ColumnIndex::new(2).unwrap(),
            ColumnIndex::new(1).unwrap(),
        ]),
        GeneratedKeys::ColumnNames(vec!["value".into(), "id".into()]),
    ];
    for keys in &modes {
        let result = lease
            .prepare_statement_with_generated_keys(
                "INSERT",
                StatementOptions::default(),
                keys.clone(),
            )
            .await;
        assert!(
            matches!(result, Err(RdbcError::DriverError(ref message)) if message == "observed raw prepare")
        );
    }
    assert_eq!(
        *requests.lock().unwrap(),
        modes
            .into_iter()
            .map(|keys| ("INSERT".to_owned(), StatementOptions::default(), keys))
            .collect::<Vec<_>>()
    );
}
