mod pooled_flow {
    use super::*;
    use druid::core::{DruidConnectionHolder,DruidPooledConnection};
    use druid_rdbc::connection::Connection;
    use druid_rdbc::statement::{Statement,StatementOptions};
    use std::time::Duration;

    #[derive(Debug)]
    struct RefConnection {value:RdbcRef,closed:bool,closed_statements:Arc<AtomicUsize>}
    druid_rdbc::impl_standard_wrapper!(RefConnection => Connection);
    #[async_trait::async_trait]
    impl Connection for RefConnection {
        async fn create_statement(&mut self,_:StatementOptions)->Result<Box<dyn Statement>,RdbcError> {
            if self.closed {return Err(RdbcError::ConnectionDiscarded);}
            Ok(Box::new(RefStatement {value:self.value.clone(),closed:false,closed_statements:self.closed_statements.clone()}))
        }
        async fn close(&mut self)->Result<(),RdbcError> {self.closed=true;Ok(())}
        fn is_closed(&self)->Result<bool,RdbcError> {Ok(self.closed)}
    }
    #[derive(Debug)]
    struct RefStatement {value:RdbcRef,closed:bool,closed_statements:Arc<AtomicUsize>}
    druid_rdbc::impl_standard_wrapper!(RefStatement => Statement);
    #[async_trait::async_trait]
    impl Statement for RefStatement {
        async fn execute_query(&mut self,_:&str)->Result<Box<dyn ResultSet>,RdbcError> {
            if self.closed {return Err(RdbcError::ConnectionDiscarded);}
            Ok(Box::new(RefCursor {value:self.value.clone(),closed:false,reads:Mutex::new(vec![]),null:AtomicBool::new(false),fail:AtomicBool::new(false)}))
        }
        async fn close(&mut self)->Result<(),RdbcError> {
            if !self.closed {self.closed=true;self.closed_statements.fetch_add(1,Ordering::SeqCst);}Ok(())
        }
        fn is_closed(&self)->Result<bool,RdbcError> {Ok(self.closed)}
    }
    async fn pooled(value:RdbcRef,closed_statements:Arc<AtomicUsize>,discards:Arc<AtomicUsize>)->DruidPooledConnection {
        let holder=DruidConnectionHolder::with_standard_connection(Box::new(RefConnection {value,closed:false,closed_statements}),77,Duration::ZERO,0).await.unwrap();
        DruidPooledConnection::with_standard_holder_and_disposition_observer(holder,"ref-contract".into(),None,false,false,
            Box::new(move |_| {discards.fetch_add(1,Ordering::SeqCst);}),Box::new(move |_,_|true))
    }
    #[tokio::test]
    async fn task7_pooled_result_set_ref_keeps_standard_driver_identity_after_parent_close() {
        let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());
        let closed=Arc::new(AtomicUsize::new(0));let discards=Arc::new(AtomicUsize::new(0));let mut connection=pooled(value.clone(),closed.clone(),discards.clone()).await;
        let mut statement=connection.create_statement(StatementOptions::default()).await.unwrap();let mut cursor=statement.execute_query("select ref_value").await.unwrap();assert!(cursor.next().await.unwrap());
        let first=cursor.reference(ColumnIndex::new(1).unwrap()).unwrap().unwrap();let alias=cursor.reference_by_label("OriginalLabel").unwrap().unwrap();assert_eq!(first,value);assert_eq!(first,alias);
        assert_eq!(probe.calls.load(Ordering::SeqCst),0);assert_eq!(probe.attributes.load(Ordering::SeqCst),0);
        cursor.close().await.unwrap();assert!(cursor.reference_by_label("closed").is_err());statement.close().await.unwrap();connection.close().await.unwrap();
        assert_eq!(closed.load(Ordering::SeqCst),1);assert_eq!(first.base_type_name().await.unwrap(),"schema.Target");
        let RdbcObject::Struct(structure)=alias.object().await.unwrap() else {panic!("Struct")};assert_eq!(structure.attributes().await.unwrap()[0],Value::Int(7).into());
        assert_eq!(discards.load(Ordering::SeqCst),0);
    }
    #[tokio::test]
    async fn task7_pooled_returned_ref_preserves_raw_method_errors_without_extra_owner_policy() {
        let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());let discards=Arc::new(AtomicUsize::new(0));
        let mut connection=pooled(value,Arc::new(AtomicUsize::new(0)),discards.clone()).await;let mut statement=connection.create_statement(StatementOptions::default()).await.unwrap();
        let cursor=statement.execute_query("select ref_value").await.unwrap();let result=cursor.reference(ColumnIndex::new(1).unwrap()).unwrap().unwrap();
        probe.fail.store(true,Ordering::SeqCst);assert_eq!(result.object().await.unwrap_err(),failure());assert_eq!(discards.load(Ordering::SeqCst),0);assert!(!connection.is_closed().unwrap());
        probe.fail.store(false,Ordering::SeqCst);assert_eq!(result.base_type_name().await.unwrap(),"schema.Target");connection.close().await.unwrap();
    }
}
