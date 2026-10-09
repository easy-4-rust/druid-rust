#[tokio::test]
async fn task7_blob_value_target_capability_precedes_closed_pattern_for_both_orders(){
    // 构造两种身份排序，避免只覆盖恰好先锁目标的分支。
    for target_first in [true,false]{
        let target_probe=blob_value_probe(ResourceOperations::READ|ResourceOperations::FREE);
        let (target,pattern)=if target_first {
            (blob_value(target_probe.clone()),blob_value(blob_value_probe(ResourceOperations::blob())))
        }else{
            let pattern=blob_value(blob_value_probe(ResourceOperations::blob()));(blob_value(target_probe.clone()),pattern)
        };
        pattern.free().await.unwrap();
        let error=target.position_blob(&pattern,1).await.unwrap_err();
        assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");assert!(target_probe.calls.lock().unwrap().is_empty());
        target.free().await.unwrap();let error=target.position_blob(&pattern,1).await.unwrap_err();assert_eq!(error.class_name(),RdbcError::rdbc_resource_closed("Blob").class_name());
    }
}
