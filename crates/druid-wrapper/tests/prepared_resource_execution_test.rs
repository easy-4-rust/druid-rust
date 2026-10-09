//! 验证资源参数在真实 adapter 执行边界读取，而非在绑定或批次快照时读取。

use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcInputStream, RdbcStreamLength};
use druid_rdbc::statement::{ParameterIndex, PreparedInputParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

/// 保存 Filter 收到的完整描述符，验证重试时逻辑批次与 driver 批次一致。
#[derive(Default)]
struct BatchDescriptorObserver {
    inputs: Mutex<Vec<Vec<Vec<PreparedInputParameter>>>>,
}

#[async_trait::async_trait]
impl druid::core::BeforeFilter for BatchDescriptorObserver {
    fn name(&self) -> &str {
        "resource-batch-observer"
    }
    async fn before(
        &self,
        _context: &mut druid::core::ExecContext<'_>,
    ) -> Result<(), druid::core::DruidError> {
        Ok(())
    }
    async fn before_batch(
        &self,
        context: &mut druid::core::BatchExecContext<'_>,
    ) -> Result<(), druid::core::DruidError> {
        self.inputs
            .lock()
            .unwrap()
            .push(context.prepared_parameter_sets.unwrap().to_vec());
        Ok(())
    }
}

/// 用明确的 Filter 等待点验证调用者在 raw 执行前取消。
struct BatchBeforeGate {
    enabled: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    batch_sizes: Mutex<Vec<usize>>,
}

#[async_trait::async_trait]
impl druid::core::BeforeFilter for BatchBeforeGate {
    fn name(&self) -> &str {
        "resource-batch-before-gate"
    }
    async fn before(
        &self,
        _context: &mut druid::core::ExecContext<'_>,
    ) -> Result<(), druid::core::DruidError> {
        Ok(())
    }
    async fn before_batch(
        &self,
        context: &mut druid::core::BatchExecContext<'_>,
    ) -> Result<(), druid::core::DruidError> {
        self.batch_sizes
            .lock()
            .unwrap()
            .push(context.prepared_parameter_sets.unwrap().len());
        if self.enabled.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        Ok(())
    }
}

#[tokio::test]
async fn cancellation_before_raw_execution_preserves_unread_batch_for_retry() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let gate = Arc::new(BatchBeforeGate {
            enabled: AtomicBool::new(true),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            batch_sizes: Mutex::new(Vec::new()),
        });
        let mut chain = druid::core::FilterChain::new();
        chain.add_before(gate.clone());
        let pool = druid::pool::DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .filter_chain(Arc::new(chain))
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        connection
            .exec("CREATE TABLE cancel_stream(value BLOB)", vec![])
            .await
            .unwrap();
        let mut insert = connection
            .prepare_statement(
                "INSERT INTO cancel_stream(value) VALUES (?1)",
                StatementOptions::default(),
            )
            .await
            .unwrap();
        let (stream, reads) = observed_stream(b"retry-once");
        insert
            .add_input_parameter_batch(vec![(
                ParameterIndex::new(1).unwrap(),
                PreparedInputParameter::BinaryStream {
                    stream: Some(stream),
                    length: RdbcStreamLength::Unspecified,
                },
            )])
            .unwrap();
        let mut execution = Box::pin(insert.execute_prepared_batch());
        tokio::select! {
            result = &mut execution => panic!("must stop before raw: {result:?}"),
            () = gate.entered.notified() => {}
        }
        drop(execution);
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        gate.enabled.store(false, Ordering::SeqCst);
        assert_eq!(insert.execute_prepared_batch().await.unwrap(), vec![1]);
        assert_eq!(
            insert.execute_prepared_batch().await.unwrap(),
            Vec::<i64>::new()
        );
        assert_eq!(*gate.batch_sizes.lock().unwrap(), vec![1, 1, 0]);
        assert_eq!(
            connection
                .fetch("SELECT value FROM cancel_stream", vec![])
                .await
                .unwrap()[0]
                .values,
            vec![Value::Bytes(b"retry-once".to_vec())]
        );
        insert.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}

/// 记录真实读取次数，允许不移动游标地观察绑定和批次快照。
struct ObservedReader {
    cursor: Cursor<Vec<u8>>,
    reads: Arc<AtomicUsize>,
}

#[tokio::test]
async fn partial_resource_batch_failure_consumes_the_attempt_once() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        let observer = Arc::new(BatchDescriptorObserver::default());
        let mut chain = druid::core::FilterChain::new();
        chain.add_before(observer.clone());
        let pool = druid::pool::DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .filter_chain(Arc::new(chain))
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        connection
            .exec("CREATE TABLE partial_stream(value BLOB)", vec![])
            .await
            .unwrap();
        let mut insert = connection
            .prepare_statement(
                "INSERT INTO partial_stream(value) VALUES (?1)",
                StatementOptions::default(),
            )
            .await
            .unwrap();
        let (first, first_reads) = observed_stream(b"first");
        let (last, last_reads) = observed_stream(b"never-executed");
        let inputs = vec![
            PreparedInputParameter::BinaryStream {
                stream: Some(first),
                length: RdbcStreamLength::Int(5),
            },
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes(b"x")),
                length: RdbcStreamLength::Int(2),
            },
            PreparedInputParameter::BinaryStream {
                stream: Some(last),
                length: RdbcStreamLength::Unspecified,
            },
        ];
        for input in &inputs {
            insert
                .add_input_parameter_batch(vec![(ParameterIndex::new(1).unwrap(), input.clone())])
                .unwrap();
        }
        let error = insert.execute_prepared_batch().await.unwrap_err();
        let RdbcError::BatchUpdateException {
            update_counts,
            cause,
        } = error
        else {
            panic!("must preserve batch failure")
        };
        assert_eq!(update_counts.large(), &[1]);
        assert!(matches!(*cause, RdbcError::DriverError(_)));
        assert!(first_reads.load(Ordering::SeqCst) > 0);
        assert_eq!(last_reads.load(Ordering::SeqCst), 0);
        assert_eq!(
            insert.execute_prepared_batch().await.unwrap(),
            Vec::<i64>::new()
        );
        let observed = observer.inputs.lock().unwrap().clone();
        assert_eq!(
            observed,
            vec![
                inputs
                    .into_iter()
                    .map(|input| vec![input])
                    .collect::<Vec<_>>(),
                Vec::new()
            ]
        );
        assert_eq!(
            connection
                .fetch("SELECT value FROM partial_stream", vec![])
                .await
                .unwrap()[0]
                .values,
            vec![Value::Bytes(b"first".to_vec())]
        );
        insert.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}

impl Read for ObservedReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.cursor.read(buffer)
    }
}

fn observed_stream(bytes: &[u8]) -> (RdbcInputStream, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let stream = RdbcInputStream::new(ObservedReader {
        cursor: Cursor::new(bytes.to_vec()),
        reads: Arc::clone(&reads),
    });
    (stream, reads)
}

#[tokio::test]
async fn binary_stream_binding_batch_snapshot_and_execution_preserve_cursor() {
    let factories: Vec<Arc<dyn ConnectionFactory>> = vec![
        Arc::new(
            ToastyConnectionFactory::new("sqlite::memory:")
                .await
                .unwrap(),
        ),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
    ];
    for factory in factories {
        // Toasty 与 SQLx SQLite 的 BASE 都在同步 setter 消费资源。
        let pool = druid::pool::DruidPool::builder()
            .factory(factory)
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        connection
            .exec("CREATE TABLE stream_item(value BLOB)", vec![])
            .await
            .unwrap();
        let mut insert = connection
            .prepare_statement(
                "INSERT INTO stream_item(value) VALUES (?1)",
                StatementOptions::default(),
            )
            .await
            .unwrap();
        let (first, first_reads) = observed_stream(b"first-tail");
        insert
            .set_input_parameter(
                ParameterIndex::new(1).unwrap(),
                PreparedInputParameter::BinaryStream {
                    stream: Some(first.clone()),
                    length: RdbcStreamLength::Long(5),
                },
            )
            .unwrap();
        assert_eq!(first_reads.load(Ordering::SeqCst), 1);
        insert.add_prepared_batch().unwrap();
        assert_eq!(first_reads.load(Ordering::SeqCst), 1);
        // 直接加入 rich 批次也不能物化；覆盖当前槽位不改变先前批次。
        let (second, second_reads) = observed_stream(b"second");
        insert
            .add_input_parameter_batch(vec![(
                ParameterIndex::new(1).unwrap(),
                PreparedInputParameter::BinaryStream {
                    stream: Some(second.clone()),
                    length: RdbcStreamLength::Unspecified,
                },
            )])
            .unwrap();
        insert.clear_parameters().unwrap();
        assert_eq!(second_reads.load(Ordering::SeqCst), 0);
        assert_eq!(insert.execute_prepared_batch().await.unwrap(), vec![1, 1]);
        assert!(first_reads.load(Ordering::SeqCst) > 0);
        assert!(second_reads.load(Ordering::SeqCst) > 0);
        assert_eq!(first.read_to_end().unwrap(), b"-tail");
        assert!(second.read_to_end().unwrap().is_empty());
        assert_eq!(
            connection
                .fetch("SELECT value FROM stream_item ORDER BY rowid", vec![])
                .await
                .unwrap()
                .into_iter()
                .map(|row| row.values)
                .collect::<Vec<_>>(),
            vec![
                vec![Value::Bytes(b"first".to_vec())],
                vec![Value::Bytes(b"second".to_vec())]
            ]
        );

        // clear batch 不新增读取；两个 SQLite adapter 都已在 setter 消费。
        let (cleared, cleared_reads) = observed_stream(b"cleared");
        insert
            .set_input_parameter(
                ParameterIndex::new(1).unwrap(),
                PreparedInputParameter::BinaryStream {
                    stream: Some(cleared),
                    length: RdbcStreamLength::Unspecified,
                },
            )
            .unwrap();
        let reads_after_binding = cleared_reads.load(Ordering::SeqCst);
        assert!(reads_after_binding > 0);
        insert.add_prepared_batch().unwrap();
        insert.clear_prepared_batch().unwrap();
        assert_eq!(
            insert.execute_prepared_batch().await.unwrap(),
            Vec::<i64>::new()
        );
        assert_eq!(cleared_reads.load(Ordering::SeqCst), reads_after_binding);

        let (negative, negative_reads) = observed_stream(b"negative");
        let bound = insert.set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(negative),
                length: RdbcStreamLength::Int(-1),
            },
        );
        assert!(matches!(bound, Err(RdbcError::InvalidArgument(_))));
        assert_eq!(negative_reads.load(Ordering::SeqCst), 0);
        let bound = insert.set_input_parameter(
            ParameterIndex::new(1).unwrap(),
            PreparedInputParameter::BinaryStream {
                stream: Some(RdbcInputStream::from_bytes(b"x")),
                length: RdbcStreamLength::Int(2),
            },
        );
        assert!(matches!(bound, Err(RdbcError::DriverError(_))));
        assert_eq!(
            connection
                .fetch("SELECT COUNT(*) FROM stream_item", vec![])
                .await
                .unwrap()[0]
                .values,
            vec![Value::Int(2)]
        );
        insert.close().await.unwrap();
        let (closed, closed_reads) = observed_stream(b"closed");
        assert!(matches!(
            insert.set_input_parameter(
                ParameterIndex::new(1).unwrap(),
                PreparedInputParameter::BinaryStream {
                    stream: Some(closed),
                    length: RdbcStreamLength::Unspecified,
                }
            ),
            Err(RdbcError::ConnectionDiscarded)
        ));
        assert_eq!(closed_reads.load(Ordering::SeqCst), 0);
        connection.close().await.unwrap();
        pool.close().await;
    }
}
