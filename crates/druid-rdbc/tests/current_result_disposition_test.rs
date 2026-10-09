use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{CurrentResultDisposition, Statement};
use std::sync::{Arc, Mutex};

/// 观察 typed 处置调用边界。对应 Java：Statement#getMoreResults(int)。
struct DispositionProbe {
    calls: Arc<Mutex<Vec<CurrentResultDisposition>>>,
    unsupported: bool,
}

druid_rdbc::impl_standard_wrapper!(DispositionProbe => Statement);

#[async_trait::async_trait]
impl Statement for DispositionProbe {
    async fn get_more_results(
        &mut self,
        disposition: CurrentResultDisposition,
    ) -> Result<bool, RdbcError> {
        self.calls.lock().unwrap().push(disposition);
        if self.unsupported {
            Err(RdbcError::UnsupportedOperation {
                operation: "probe_typed_disposition",
            })
        } else {
            Ok(false)
        }
    }
}

#[tokio::test]
async fn integer_disposition_rejects_invalid_values_without_calling_typed_driver() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut statement = DispositionProbe {
        calls: calls.clone(),
        unsupported: false,
    };
    for current in [i32::MIN, -1, 0, 4, 999, i32::MAX] {
        assert!(matches!(
            statement.get_more_results_current(current).await,
            Err(RdbcError::InvalidArgument(_))
        ));
        assert!(calls.lock().unwrap().is_empty());
    }
    for current in [1, 2, 3] {
        assert!(!statement.get_more_results_current(current).await.unwrap());
    }
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            CurrentResultDisposition::CloseCurrent,
            CurrentResultDisposition::KeepCurrent,
            CurrentResultDisposition::CloseAll,
        ]
    );
}

#[tokio::test]
async fn integer_disposition_preserves_typed_driver_error_and_default_is_distinct() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut statement = DispositionProbe {
        calls: calls.clone(),
        unsupported: true,
    };
    assert!(matches!(
        statement.get_more_results_current(1).await,
        Err(RdbcError::UnsupportedOperation {
            operation: "probe_typed_disposition"
        })
    ));
    assert!(matches!(
        statement
            .get_more_results(CurrentResultDisposition::Default)
            .await,
        Err(RdbcError::UnsupportedOperation {
            operation: "probe_typed_disposition"
        })
    ));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            CurrentResultDisposition::CloseCurrent,
            CurrentResultDisposition::Default
        ]
    );
}
