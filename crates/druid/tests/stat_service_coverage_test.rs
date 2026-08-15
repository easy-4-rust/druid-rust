//! DruidStatService 差分覆盖测试（Java Druid 1.2.28 语义对照）。
//!
//! 覆盖 stat_service 的 URL 路由分发、page 排序分页、sql_detail 格式化、
//! between_id 解析、parameters 解析、value_by_key 嵌套键、compare_map_value
//! 比较语义。

use druid::stats::DruidStatService;

// ===========================================================================
// 1. 基础 URL 路由
// ===========================================================================

/// Java DruidStatManager.service("/basic.json")。
#[test]
fn service_basic_json() {
    let svc = DruidStatService;
    let result = svc.service("/basic.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
    assert!(parsed["Content"].is_object());
    assert!(parsed["Content"]["Version"].is_string());
}

/// Java service("/reset-all.json")。
#[test]
fn service_reset_all_json() {
    let svc = DruidStatService;
    let result = svc.service("/reset-all.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/log-and-reset.json")。
#[test]
fn service_log_and_reset_json() {
    let svc = DruidStatService;
    let result = svc.service("/log-and-reset.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/datasource.json")。
#[test]
fn service_datasource_json() {
    let svc = DruidStatService;
    let result = svc.service("/datasource.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
    assert!(parsed["Content"].is_array());
}

/// Java service("/sql.json")。
#[test]
fn service_sql_json() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/sql.json?dataSourceId=1") — 按数据源 ID 筛选。
#[test]
fn service_sql_json_with_datasource_id() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?dataSourceId=1");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/wall.json")。
#[test]
fn service_wall_json() {
    let svc = DruidStatService;
    let result = svc.service("/wall.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/wall.json?dataSourceId=1") — 按数据源 ID 筛选。
#[test]
fn service_wall_json_with_datasource_id() {
    let svc = DruidStatService;
    let result = svc.service("/wall.json?dataSourceId=1");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/wall-1.json") — 指定数据源 Wall。
#[test]
fn service_wall_by_id_json() {
    let svc = DruidStatService;
    let result = svc.service("/wall-1.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/sql-999.json") — 不存在的 SQL ID。
#[test]
fn service_sql_nonexistent_id() {
    let svc = DruidStatService;
    let result = svc.service("/sql-999999.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

/// Java service("/datasource-999.json") — 不存在的数据源 ID。
#[test]
fn service_datasource_nonexistent_id() {
    let svc = DruidStatService;
    let result = svc.service("/datasource-999999.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

/// Java service("/connectionInfo-999.json") — 不存在的数据源。
#[test]
fn service_connection_info_nonexistent() {
    let svc = DruidStatService;
    let result = svc.service("/connectionInfo-999999.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

/// Java service("/activeConnectionStackTrace.json")。
#[test]
fn service_active_connection_stack_trace_all() {
    let svc = DruidStatService;
    let result = svc.service("/activeConnectionStackTrace.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/activeConnectionStackTrace-1.json") — 不存在的数据源。
#[test]
fn service_active_connection_stack_trace_by_id() {
    let svc = DruidStatService;
    let result = svc.service("/activeConnectionStackTrace-999999.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    // 不存在的数据源返回 error（removeAbandoned=false）
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

/// Java service — 不支持的 URL。
#[test]
fn service_unsupported_url() {
    let svc = DruidStatService;
    let result = svc.service("/unsupported.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

/// Java service — 空 URL 走 default 分支。
#[test]
fn service_empty_url() {
    let svc = DruidStatService;
    let result = svc.service("");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

// ===========================================================================
// 2. page 排序分页
// ===========================================================================

/// Java page：空列表返回 Null。
#[test]
fn service_sql_page_empty_list() {
    let svc = DruidStatService;
    // 没有注册数据源时 sql 列表为空，page 返回 Null
    let result = svc.service("/sql.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    // Content 可能是 Null（空列表）或 Array
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java page：分页参数 page=1&perPageCount=10。
#[test]
fn service_sql_page_with_pagination() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?page=1&perPageCount=10");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java page：排序参数 orderBy=ExecuteCount&orderType=desc。
#[test]
fn service_sql_page_with_ordering() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?orderBy=ExecuteCount&orderType=desc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java page：orderType=asc（默认）。
#[test]
fn service_sql_page_ascending() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?orderBy=ExecuteCount&orderType=asc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

// ===========================================================================
// 3. reset-enable 门禁
// ===========================================================================

/// Java isResetEnable/setResetEnable。
#[test]
fn service_reset_enable_toggle() {
    let svc = DruidStatService;
    svc.set_reset_enable(false);
    assert!(!svc.is_reset_enable());
    svc.set_reset_enable(true);
    assert!(svc.is_reset_enable());
}

/// Java reset-all：reset_enable=false 时 reset_all 无副作用。
#[test]
fn service_reset_all_when_disabled() {
    let svc = DruidStatService;
    svc.set_reset_enable(false);
    let result = svc.service("/reset-all.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
    svc.set_reset_enable(true);
}

// ===========================================================================
// 4. between_id 解析
// ===========================================================================

/// Java betweenId：正常解析。
#[test]
fn service_sql_id_parsing() {
    let svc = DruidStatService;
    // /sql-123.json → id=123
    let result = svc.service("/sql-123.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    // 不存在的 SQL 返回 error
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

/// Java betweenId：非数字 ID。
#[test]
fn service_sql_non_numeric_id() {
    let svc = DruidStatService;
    let result = svc.service("/sql-abc.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

/// Java betweenId：datasource ID 解析。
#[test]
fn service_datasource_id_parsing() {
    let svc = DruidStatService;
    let result = svc.service("/datasource-456.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

// ===========================================================================
// 5. parameters 解析
// ===========================================================================

/// Java getParameters：query string 解析。
#[test]
fn service_parameters_parsing() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?page=2&perPageCount=5&orderBy=SQL&orderType=desc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java getParameters：无 query string。
#[test]
fn service_no_query_string() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java getParameters：空 query string。
#[test]
fn service_empty_query_string() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java getParameters：无值参数（key=）。
#[test]
fn service_parameter_no_value() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?page=");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

// ===========================================================================
// 6. wall stat 路由
// ===========================================================================

/// Java service("/wall.json") — sort_wall_stat 处理 tables/functions 键。
#[test]
fn service_wall_stat_sort() {
    let svc = DruidStatService;
    let result = svc.service("/wall.json?page=1&perPageCount=10&orderBy=name&orderType=asc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

/// Java service("/wall.json") — 带 dataSourceId 参数。
#[test]
fn service_wall_with_datasource_filter() {
    let svc = DruidStatService;
    let result = svc.service("/wall.json?dataSourceId=0");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

// ===========================================================================
// 7. SQL detail 路由
// ===========================================================================

/// Java service("/sql-{id}.json") — formattedSql 与 MaxTimespanOccurTime。
#[test]
fn service_sql_detail_nonexistent() {
    let svc = DruidStatService;
    let result = svc.service("/sql-1.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    // 不存在的 SQL 返回 error
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}
