//! `DruidPooledConnection` 标准合同中的重量级方法体。
//!
//! 仅为把 `impl Connection` 压到 800 行以内；语义与拆分前逐行一致。

use super::*;

/// 构造受 Filter/资源注册表治理的 DatabaseMetaData。
///
/// 对应 Java: `DruidPooledConnection#getMetaData`。
pub(crate) async fn database_meta_data(
    conn: &mut DruidPooledConnection,
) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
    let filter_chain = conn.filter_chain.clone();
    let connection_id = conn.id;
    let raw_result = match &filter_chain {
        Some(filter_chain) => filter_chain
            .standard_connection_meta_data(conn.standard_mut()?, connection_id)
            .await
            .map_err(RdbcError::from),
        None => conn.standard_mut()?.database_meta_data().await,
    };
    let raw = conn.classify_result(raw_result)?;
    let resources = conn
        .holder
        .as_ref()
        .ok_or(RdbcError::ConnectionDiscarded)?
        .standard_resource_registry();
    let policy = conn
        .standard_policy
        .as_ref()
        .ok_or(RdbcError::ConnectionDiscarded)?
        .clone();
    Ok(Box::new(DatabaseMetaDataProxyImpl::new_standard(
        raw,
        conn.id,
        Arc::clone(&conn.lease_active),
        Arc::clone(&conn.result_set_id_seed),
        resources,
        policy,
        conn.filter_chain.clone(),
        conn.stats_collector.clone(),
    )))
}

/// 生成键 prepare 的完整合同；对应 Java `DruidPooledConnection#prepareStatement(sql, generatedKeys)`。
pub(crate) async fn prepare_statement_with_generated_keys(
    conn: &mut DruidPooledConnection,
    sql: &str,
    options: druid_rdbc::statement::StatementOptions,
    generated_keys: druid_rdbc::statement::GeneratedKeys,
) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
    let effective_sql =
        DruidExecutionFilterPolicy::prepare_statement_sql(conn.filter_chain.as_deref(), sql)?;
    // 对应 Java prepareStatement 重载：请求模式与列顺序均参与私有缓存身份。
    let method_type = match &generated_keys {
        druid_rdbc::statement::GeneratedKeys::None => PreparedStatementMethodType::M1,
        druid_rdbc::statement::GeneratedKeys::ReturnGeneratedKeys => {
            PreparedStatementMethodType::M6
        }
        druid_rdbc::statement::GeneratedKeys::ColumnIndexes(_) => PreparedStatementMethodType::M4,
        druid_rdbc::statement::GeneratedKeys::ColumnNames(_) => PreparedStatementMethodType::M5,
        _ => {
            return Err(RdbcError::UnsupportedOperation {
                operation: "connection_prepare_statement_generated_keys",
            })
        }
    };
    let key = PreparedStatementKey::standard(
        &effective_sql,
        conn.current_catalog(),
        method_type,
        options,
        generated_keys.clone(),
    )?;
    let cache = conn
        .holder
        .as_ref()
        .ok_or(RdbcError::ConnectionDiscarded)?
        .standard_statement_pool();
    let cache_stats = Arc::clone(
        conn.holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .prepared_statement_stats(),
    );
    let resources = conn
        .holder
        .as_ref()
        .ok_or(RdbcError::ConnectionDiscarded)?
        .standard_resource_registry();
    let policy = conn
        .standard_policy
        .as_ref()
        .ok_or(RdbcError::ConnectionDiscarded)?
        .clone();
    let admission = resources.begin_admission(policy.resource_generation())?;
    let (cached, cache_enabled) = {
        let mut cache = cache.lock().await;
        let enabled = cache.is_enabled();
        (cache.take(&key), enabled)
    };
    let cache_hit = cached.is_some();
    let statement = if let Some(statement) = cached {
        cache_stats.record_cache_checkout();
        cache_stats.record_hit();
        statement
    } else {
        let result = conn
            .standard_mut()?
            .prepare_statement_with_generated_keys(&effective_sql, options, generated_keys)
            .await;
        let statement = conn.classify_result_with_sql(result, Some(&effective_sql))?;
        cache_stats.record_prepare();
        if cache_enabled {
            cache_stats.record_miss();
        }
        statement
    };
    let statement_id = conn.next_statement_id();
    let statement = DruidPooledPreparedStatement::new_standard(
        statement,
        &effective_sql,
        options,
        key,
        cache,
        cache_stats,
        cache_hit,
        resources,
        &admission,
        policy,
        conn.id,
        statement_id,
        Arc::clone(&conn.result_set_id_seed),
        Arc::clone(&conn.metadata_id_seed),
        Arc::clone(&conn.lease_active),
        conn.filter_chain.clone(),
        conn.stats_collector.clone(),
    )
    .await?;
    let query_timeout = conn.effective_query_timeout();
    if query_timeout > 0 {
        let result = statement.configure_standard_query_timeout(query_timeout);
        conn.classify_result_with_sql(result, Some(&effective_sql))?;
    }
    let event_result = DruidExecutionFilterPolicy::complete_statement_creation(
        conn.filter_chain.as_deref(),
        conn.stats_collector.as_ref(),
        conn.id,
        statement_id,
        &super::super::StatementEvent::PrepareStatement(effective_sql),
    )
    .await;
    conn.classify_result(event_result)?;
    Ok(Box::new(statement))
}

/// 连接有效性检查；对应 Java `DruidPooledConnection#isValid`。
pub(crate) async fn is_valid(
    conn: &mut DruidPooledConnection,
    timeout_seconds: u32,
) -> Result<bool, RdbcError> {
    let event = ConnectionEvent::IsValid;
    let started_at = Instant::now();
    conn.before_connection_event(&event).await?;
    let result = conn.standard_mut()?.is_valid(timeout_seconds).await;
    let valid = conn.classify_result(result)?;
    conn.after_connection_event(&event, started_at).await?;
    Ok(valid)
}

/// 连接存活探测；对应 Java `DruidPooledConnection#ping`。
pub(crate) async fn ping(conn: &mut DruidPooledConnection) -> Result<(), RdbcError> {
    let result = conn.standard_mut()?.ping().await;
    conn.classify_result(result)
}
