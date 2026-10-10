//! 完整 RowSet 标准执行、事件、原始行、provider 写回和取消的行为合同。
//! 不把入口可导入、mock callback 或本地行状态当作真实数据库认证。
use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::extensions::{
    RowSet, RowSetEvent, RowSetInternal, RowSetListener, RowSetMetaData, RowSetMetaDataBuffer,
    RowSetReader, RowSetWriter, SharedRowSet, SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter, ResultSetConcurrency};
use druid_rdbc::types::{RdbcCharacterLength, RdbcObject, RdbcReader};
use std::collections::VecDeque;
use std::future::{pending, poll_fn, Future};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}
fn parameter(index: usize) -> ParameterIndex {
    ParameterIndex::new(index).unwrap()
}
fn values(id: i32, text: &str) -> Vec<RdbcObject> {
    vec![RdbcObject::Integer(id), RdbcObject::String(text.into())]
}
fn metadata() -> SharedRowSetMetaData {
    let mut meta = RowSetMetaDataBuffer::new();
    meta.set_column_count(2).unwrap();
    meta.set_column_label(column(1), Some("id".into())).unwrap();
    meta.set_column_label(column(2), Some("value".into()))
        .unwrap();
    SharedRowSetMetaData::new(Box::new(meta))
}
fn provider_error() -> RdbcError {
    RdbcError::SqlException(Box::new(
        SqlException::new(977, Some("40001".into()), None).with_class_name("example.RowSetFailure"),
    ))
}
fn assert_provider_error(error: RdbcError) {
    let RdbcError::SqlException(error) = error else {
        panic!("original SQL exception required")
    };
    assert_eq!(error.error_code(), 977);
    assert_eq!(error.sql_state(), Some("40001"));
    assert_eq!(error.message(), None);
    assert_eq!(error.class_name(), "example.RowSetFailure");
}
fn assert_sql_error<T>(value: Result<T, RdbcError>) {
    assert!(matches!(value, Err(RdbcError::SqlException(_))));
}

#[derive(Clone, Copy)]
enum ReadMode {
    Load,
    Fail,
    Recursive,
    Pending,
}
struct Reader {
    rows: Vec<Vec<RdbcObject>>,
    meta: SharedRowSetMetaData,
    mode: ReadMode,
    entered: Arc<AtomicBool>,
    seen_params: Arc<Mutex<Vec<PreparedInputParameter>>>,
    seen_connection: Arc<Mutex<Option<String>>>,
}
#[async_trait::async_trait]
impl RowSetReader for Reader {
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        *self.seen_params.lock().unwrap() = caller.params()?;
        let connection_name = match caller.connection()? {
            Some(connection) => Some(connection.driver_name()?),
            None => None,
        };
        *self.seen_connection.lock().unwrap() = connection_name;
        caller.set_meta_data(self.meta.clone())?;
        let row_set = caller.row_set();
        row_set.move_to_insert_row()?;
        for row in &self.rows {
            for (offset, value) in row.iter().enumerate() {
                row_set.update_value(column(offset + 1), ResultSetUpdate::Object(value.clone()))?;
            }
            row_set.insert_row()?;
        }
        row_set.move_to_current_row()?;
        self.entered.store(true, Ordering::Release);
        match self.mode {
            ReadMode::Load => Ok(()),
            ReadMode::Fail => Err(provider_error()),
            ReadMode::Recursive => row_set.execute().await,
            ReadMode::Pending => pending().await,
        }
    }
}
type ReaderInstallation = (
    SharedRowSetMetaData,
    Arc<AtomicBool>,
    Arc<Mutex<Vec<PreparedInputParameter>>>,
    Arc<Mutex<Option<String>>>,
);
fn install_reader(
    source: &mut SharedRowSet,
    rows: Vec<Vec<RdbcObject>>,
    mode: ReadMode,
) -> ReaderInstallation {
    let meta = metadata();
    let entered = Arc::new(AtomicBool::new(false));
    let seen_params = Arc::new(Mutex::new(Vec::new()));
    let seen_connection = Arc::new(Mutex::new(None));
    source
        .set_reader(Some(Box::new(Reader {
            rows,
            meta: meta.clone(),
            mode,
            entered: Arc::clone(&entered),
            seen_params: Arc::clone(&seen_params),
            seen_connection: Arc::clone(&seen_connection),
        })))
        .unwrap();
    (meta, entered, seen_params, seen_connection)
}

type Events = Arc<Mutex<Vec<(char, SharedRowSet)>>>;
struct Listener {
    events: Events,
}
impl Listener {
    fn record(&self, kind: char, event: &RowSetEvent) {
        let mut source = event.source();
        assert!(
            source.known_row_count().is_some(),
            "callback must observe a usable source outside locks"
        );
        if kind == 'S' {
            source.set_command(Some("changed inside listener")).unwrap();
        }
        self.events.lock().unwrap().push((kind, source));
    }
}
impl RowSetListener for Listener {
    fn row_set_changed(&self, event: &RowSetEvent) {
        self.record('S', event);
    }
    fn row_changed(&self, event: &RowSetEvent) {
        self.record('R', event);
    }
    fn cursor_moved(&self, event: &RowSetEvent) {
        self.record('C', event);
    }
}
fn listen(source: &mut SharedRowSet) -> (Arc<dyn RowSetListener>, Events) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let listener: Arc<dyn RowSetListener> = Arc::new(Listener {
        events: Arc::clone(&events),
    });
    source.add_row_set_listener(Arc::clone(&listener));
    (listener, events)
}

enum WriteOutcome {
    Conflict,
    Error,
    Success,
}
struct Writer {
    outcomes: Arc<Mutex<VecDeque<WriteOutcome>>>,
    originals: Arc<Mutex<Vec<Vec<RdbcObject>>>>,
}
#[async_trait::async_trait]
impl RowSetWriter for Writer {
    async fn write_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<bool, RdbcError> {
        let mut original = caller.original()?;
        assert_eq!(original.row()?, 0);
        let mut originals = Vec::new();
        while original.next().await? {
            originals.push(original.object(column(1))?);
        }
        self.originals.lock().unwrap().push(originals);
        match self
            .outcomes
            .lock()
            .unwrap()
            .pop_front()
            .expect("writer outcome")
        {
            WriteOutcome::Conflict => Ok(false),
            WriteOutcome::Error => Err(provider_error()),
            WriteOutcome::Success => Ok(true),
        }
    }
}

#[tokio::test]
async fn reader_emits_one_complete_event_with_same_source_and_supports_reentrant_listener() {
    let mut source = SharedRowSet::new();
    let (listener, events) = listen(&mut source);
    let (_, entered, _, _) = install_reader(
        &mut source,
        vec![values(1, "first"), values(2, "second")],
        ReadMode::Load,
    );
    let standard: &mut dyn RowSet = &mut source;
    standard.execute().await.unwrap();
    assert!(entered.load(Ordering::Acquire));
    assert_eq!(source.known_row_count(), Some(2));
    assert_eq!(source.row().unwrap(), 0);
    assert_eq!(source.command(), Some("changed inside listener".into()));
    assert_eq!(
        events.lock().unwrap().len(),
        1,
        "reader population must not emit row/cursor events"
    );
    assert_eq!(events.lock().unwrap()[0].0, 'S');
    assert_eq!(events.lock().unwrap()[0].1, source);
    assert!(source.next().await.unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(1));
    assert_eq!(events.lock().unwrap().last().unwrap().0, 'C');
    source.remove_row_set_listener(&listener);
    events.lock().unwrap().clear();
}

#[tokio::test]
async fn shared_metadata_remains_live_and_original_result_set_has_independent_before_first_cursor()
{
    let mut source = SharedRowSet::new();
    let (mut meta, _, _, _) = install_reader(
        &mut source,
        vec![values(1, "first"), values(2, "second")],
        ReadMode::Load,
    );
    source.execute().await.unwrap();
    assert_eq!(source.row_set_meta_data().unwrap(), meta);
    meta.set_column_label(column(2), Some("renamed".into()))
        .unwrap();
    assert_eq!(
        source.meta_data().unwrap().column_label(column(2)).unwrap(),
        "renamed"
    );
    assert!(source.next().await.unwrap());
    let mut original = source.original().unwrap();
    assert_eq!(original.row().unwrap(), 0);
    assert!(original.next().await.unwrap());
    assert_eq!(
        original.object(column(2)).unwrap(),
        RdbcObject::String("first".into())
    );
    assert_eq!(source.row().unwrap(), 1);
    assert!(original.next().await.unwrap());
    assert_eq!(source.row().unwrap(), 1);
    original.close().await.unwrap();
    assert!(!source.is_closed().unwrap());
}

#[tokio::test]
async fn public_setters_forward_exact_parameter_families_and_named_resolution_uses_current_command()
{
    let mut source = SharedRowSet::new();
    source.set_command(Some("select :精确参数")).unwrap();
    source.set_int(parameter(1), 11).unwrap();
    source
        .set_named_parameter(Some(" 精确参数 "), PreparedInputParameter::Int(12))
        .unwrap();
    source
        .set_named_parameter_resolver(Some(Arc::new(|command, name| {
            assert_eq!(command, Some("select :精确参数"));
            assert_eq!(name, " 精确参数 ");
            Ok(parameter(1))
        })))
        .unwrap();
    let reader = RdbcReader::from_utf16(vec![0xd800, 0x0062]);
    let descriptor = PreparedInputParameter::NCharacterStream {
        reader: Some(reader.clone()),
        length: RdbcCharacterLength::Long(-3),
    };
    source
        .set_parameter(parameter(2), descriptor.clone())
        .unwrap();
    let (_, _, seen, _) = install_reader(&mut source, vec![values(1, "first")], ReadMode::Load);
    source.execute().await.unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        vec![PreparedInputParameter::Int(12), descriptor]
    );
    let mut units = [0; 2];
    assert_eq!(reader.read_utf16(&mut units).unwrap(), 2);
    assert_eq!(
        units,
        [0xd800, 0x0062],
        "binding and provider params must not consume the reader"
    );
    source.clear_parameters().unwrap();
    assert!(source.params().unwrap().is_empty());
}

#[tokio::test]
async fn recursive_execute_is_rejected_and_original_provider_error_does_not_publish_partial_rows() {
    let mut source = SharedRowSet::new();
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    install_reader(&mut source, vec![values(99, "partial")], ReadMode::Fail);
    assert_provider_error(source.execute().await.unwrap_err());
    assert_eq!(source.known_row_count(), Some(1));
    assert!(source.next().await.unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(1));
    install_reader(
        &mut source,
        vec![values(100, "recursive")],
        ReadMode::Recursive,
    );
    assert_sql_error(source.execute().await);
    install_reader(&mut source, vec![values(2, "recovered")], ReadMode::Load);
    source.execute().await.unwrap();
    assert!(source.next().await.unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(2));
}

#[tokio::test]
async fn cancellation_after_real_provider_poll_restores_baseline_and_allows_next_execute() {
    let mut source = SharedRowSet::new();
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    let (_, entered, _, _) =
        install_reader(&mut source, vec![values(99, "partial")], ReadMode::Pending);
    let mut future = Box::pin(source.execute());
    poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(
        entered.load(Ordering::Acquire),
        "must cancel after read_data actually entered"
    );
    drop(future);
    assert_eq!(source.known_row_count(), Some(1));
    install_reader(&mut source, vec![values(2, "next")], ReadMode::Load);
    source.execute().await.unwrap();
    assert!(source.next().await.unwrap());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(2));
}

#[tokio::test]
async fn writer_conflict_error_and_success_preserve_then_accept_actual_original_rows() {
    let mut source = SharedRowSet::new();
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    source.set_read_only(false).unwrap();
    source.set_concurrency(1008).unwrap();
    assert!(source.next().await.unwrap());
    source
        .update_value(column(1), ResultSetUpdate::Int(7))
        .unwrap();
    source.update_row().unwrap();
    let outcomes = Arc::new(Mutex::new(VecDeque::from([
        WriteOutcome::Conflict,
        WriteOutcome::Error,
        WriteOutcome::Success,
    ])));
    let originals = Arc::new(Mutex::new(Vec::new()));
    source
        .set_writer(Some(Box::new(Writer {
            outcomes,
            originals: Arc::clone(&originals),
        })))
        .unwrap();
    assert!(!source.accept_changes().await.unwrap());
    assert_provider_error(source.accept_changes().await.unwrap_err());
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(7));
    assert!(source.accept_changes().await.unwrap());
    assert_eq!(
        *originals.lock().unwrap(),
        vec![vec![RdbcObject::Integer(1)]; 3]
    );
    let mut original = source.original().unwrap();
    assert!(original.next().await.unwrap());
    assert_eq!(original.object(column(1)).unwrap(), RdbcObject::Integer(7));
}

struct CallerConnection {
    closed: Arc<AtomicBool>,
    close_calls: Arc<AtomicUsize>,
}
druid_rdbc::impl_standard_wrapper!(CallerConnection => Connection);
#[async_trait::async_trait]
impl Connection for CallerConnection {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.close_calls.fetch_add(1, Ordering::AcqRel);
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("caller-owned".into())
    }
}
#[tokio::test]
async fn borrowed_caller_connection_is_exposed_to_reader_and_never_closed_by_row_set() {
    let closed = Arc::new(AtomicBool::new(false));
    let closes = Arc::new(AtomicUsize::new(0));
    let mut connection = CallerConnection {
        closed: Arc::clone(&closed),
        close_calls: Arc::clone(&closes),
    };
    let mut source = SharedRowSet::new();
    let (_, _, _, seen_connection) =
        install_reader(&mut source, vec![values(1, "row")], ReadMode::Load);
    source
        .execute_with_connection(&mut connection)
        .await
        .unwrap();
    assert_eq!(
        *seen_connection.lock().unwrap(),
        Some("caller-owned".into())
    );
    source.close().await.unwrap();
    source.close().await.unwrap();
    drop(source);
    assert!(!connection.is_closed().unwrap());
    assert_eq!(closes.load(Ordering::Acquire), 0);
    connection.close().await.unwrap();
    assert_eq!(closes.load(Ordering::Acquire), 1);
}

#[test]
fn event_rejects_null_source_and_listener_removal_uses_same_registration_identity() {
    assert!(matches!(
        RowSetEvent::new(None),
        Err(RdbcError::InvalidArgument(_))
    ));
    let source = SharedRowSet::new();
    let event = RowSetEvent::new(Some(source.clone())).unwrap();
    assert_eq!(event.source(), source);
}

#[tokio::test]
async fn removed_listener_is_not_notified_and_read_only_updates_fail_without_state_changes() {
    let mut source = SharedRowSet::new();
    let (listener, events) = listen(&mut source);
    source.remove_row_set_listener(&listener);
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    assert!(events.lock().unwrap().is_empty());
    assert_eq!(
        source.concurrency().unwrap(),
        ResultSetConcurrency::ReadOnly
    );
    assert!(source.next().await.unwrap());
    assert_sql_error(source.update_value(column(1), ResultSetUpdate::Int(9)));
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(1));
    source.close().await.unwrap();
    assert!(source.next().await.is_err());
    assert!(source.execute().await.is_err());
}

#[tokio::test]
async fn limits_are_applied_to_published_rows_and_bytes_instead_of_only_stored_properties() {
    let mut source = SharedRowSet::new();
    source.set_max_rows(1).unwrap();
    source.set_max_field_size(3).unwrap();
    install_reader(
        &mut source,
        vec![
            vec![RdbcObject::Integer(1), RdbcObject::Bytes(vec![1, 2, 3, 4])],
            values(2, "excess"),
        ],
        ReadMode::Load,
    );
    source.execute().await.unwrap();
    assert_eq!(source.known_row_count(), Some(1));
    assert!(source.next().await.unwrap());
    assert_eq!(
        source.object(column(2)).unwrap(),
        RdbcObject::Bytes(vec![1, 2, 3])
    );
    assert!(!source.next().await.unwrap());
}

#[tokio::test]
async fn query_timeout_cancels_provider_without_publishing_partial_rows_or_leaving_busy_state() {
    let mut source = SharedRowSet::new();
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    source.set_query_timeout(1).unwrap();
    let (_, entered, _, _) =
        install_reader(&mut source, vec![values(99, "partial")], ReadMode::Pending);
    let error = source.execute().await.unwrap_err();
    assert!(entered.load(Ordering::Acquire));
    let RdbcError::SqlException(error) = error else {
        panic!("SQL timeout required")
    };
    assert_eq!(error.class_name(), "java.sql.SQLTimeoutException");
    assert_eq!(source.known_row_count(), Some(1));
    source.set_query_timeout(0).unwrap();
    install_reader(&mut source, vec![values(2, "recovered")], ReadMode::Load);
    source.execute().await.unwrap();
}

// 补充合同草稿：须在原177终态后，保留原11合同并建立新冻结输入再运行。

#[tokio::test]
async fn pending_reader_never_exposes_partial_rows_to_an_external_clone() {
    let mut source = SharedRowSet::new();
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    assert!(source.next().await.unwrap());
    let observer = source.clone();
    let (_, entered, _, _) = install_reader(
        &mut source,
        vec![values(99, "partial"), values(100, "partial")],
        ReadMode::Pending,
    );
    let mut future = Box::pin(source.execute());
    poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(entered.load(Ordering::Acquire));
    assert_eq!(observer.known_row_count(), Some(1));
    assert_eq!(observer.row().unwrap(), 1);
    assert_eq!(observer.object(column(1)).unwrap(), RdbcObject::Integer(1));
    drop(future);
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(1));
}

struct PendingWriter {
    entered: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl RowSetWriter for PendingWriter {
    async fn write_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<bool, RdbcError> {
        let mut original = caller.original()?;
        assert!(original.next().await?);
        assert_eq!(original.object(column(1))?, RdbcObject::Integer(1));
        self.entered.store(true, Ordering::Release);
        pending().await
    }
}

#[tokio::test]
async fn cancellation_after_real_writer_poll_preserves_updates_and_original_baseline() {
    let mut source = SharedRowSet::new();
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    source.set_read_only(false).unwrap();
    source.set_concurrency(1008).unwrap();
    assert!(source.next().await.unwrap());
    source
        .update_value(column(1), ResultSetUpdate::Int(7))
        .unwrap();
    source.update_row().unwrap();
    let entered = Arc::new(AtomicBool::new(false));
    source
        .set_writer(Some(Box::new(PendingWriter {
            entered: Arc::clone(&entered),
        })))
        .unwrap();
    let mut future = Box::pin(source.accept_changes());
    poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(entered.load(Ordering::Acquire));
    drop(future);
    assert_eq!(source.object(column(1)).unwrap(), RdbcObject::Integer(7));
    let mut original = source.original().unwrap();
    assert!(original.next().await.unwrap());
    assert_eq!(original.object(column(1)).unwrap(), RdbcObject::Integer(1));
    let originals = Arc::new(Mutex::new(Vec::new()));
    source
        .set_writer(Some(Box::new(Writer {
            outcomes: Arc::new(Mutex::new(VecDeque::from([WriteOutcome::Success]))),
            originals: Arc::clone(&originals),
        })))
        .unwrap();
    assert!(source.accept_changes().await.unwrap());
    assert_eq!(
        *originals.lock().unwrap(),
        vec![vec![RdbcObject::Integer(1)]]
    );
}

#[tokio::test]
async fn original_row_rejects_non_row_cursor_and_preserves_deleted_original() {
    let mut source = SharedRowSet::new();
    source.set_type(1004).unwrap();
    install_reader(&mut source, vec![values(1, "baseline")], ReadMode::Load);
    source.execute().await.unwrap();
    assert_sql_error(source.original_row());
    assert!(source.next().await.unwrap());
    let mut original = source.original_row().unwrap();
    assert_eq!(original.row().unwrap(), 0);
    assert!(original.next().await.unwrap());
    assert_eq!(original.object(column(1)).unwrap(), RdbcObject::Integer(1));
    assert!(!original.next().await.unwrap());
    source.set_read_only(false).unwrap();
    source.set_concurrency(1008).unwrap();
    source.delete_row().unwrap();
    let mut deleted = source.original().unwrap();
    assert!(deleted.next().await.unwrap());
    assert_eq!(deleted.object(column(1)).unwrap(), RdbcObject::Integer(1));
    source.move_to_insert_row().unwrap();
    assert_sql_error(source.original_row());
    source
        .update_value(column(1), ResultSetUpdate::Int(2))
        .unwrap();
    source
        .update_value(column(2), ResultSetUpdate::String(Some("inserted".into())))
        .unwrap();
    source.insert_row().unwrap();
    source.move_to_current_row().unwrap();
    source.after_last().await.unwrap();
    assert_sql_error(source.original_row());
}
