from pathlib import Path
import re
import json
import hashlib

root = Path.cwd()
evidence = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
source = (evidence / 'drafts/complete_scalar_update_base_original_20261007.rs.txt').read_text(encoding='utf-8')
name = 'pooled_result_set_routes_scalar_updates_and_preserves_sqlite_capability_errors'
target = root / 'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs'
current = target.read_text(encoding='utf-8')
assert 'async fn task6_' + name not in current
source = source.replace('async fn ' + name, 'async fn task6_' + name)
source = source.replace('() {\n', '''() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use scalar_update_short_circuit_filter::ScalarUpdateShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
''', 1)
pattern = r'    let physical = factory.create\(\).await.unwrap\(\);\n    let mut connection = DruidPooledConnection::with_context\(.*?\.unwrap\(\);\n'
matches = list(re.finditer(pattern, source, re.S))
assert len(matches) == 2
for match, context_id, context_name, chain in reversed(list(zip(matches, [87,88], ['sqlite-scalar-update-short-circuit','sqlite-scalar-update-pass-through'], ['filter_chain','pass_chain']))):
    replacement = f'''    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory, {context_id}, "{context_name}", Arc::new({chain}),
    ).await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();
'''
    source = source[:match.start()] + replacement + source[match.end():]
source = source.replace('PassThroughResultSetFilter', 'FilterAdapter::new()')
source = source.replace('&mut connection, 1', 'ColumnIndex::new(1).unwrap()')
source = source.replace('&mut connection, ', '')
source = source.replace('result_set.close_with_connection(&mut connection).unwrap();', 'result_set.close().await.unwrap();\n    statement.close().await.unwrap();\n    connection.close().await.unwrap();')
source = source.replace('!result_set.is_closed()', '!result_set.is_closed().unwrap()')
# 原长度断言保留；额外逐项校验操作身份与参数，避免仅以总数证明完整性。
events = [('Null','null'), ('Boolean','boolean'), ('Byte','byte'), ('Short','short'), ('Int','int'), ('Long','long'), ('Float','float'), ('Double','double'), ('BigDecimal','big_decimal'), ('String','string'), ('Bytes','bytes'), ('Date','date'), ('Time','time'), ('Timestamp','timestamp')]
values = [('',''), ('true','false'), ('-8','8'), ('-16','16'), ('-32','32'), ('-64','64'), ('1.25','-1.25'), ('2.5','-2.5'), ('Some(BigDecimal::from_str("12.340").unwrap())','None::<BigDecimal>'), ('Some("index".to_string())','None::<String>'), ('Some(vec![0u8, 255])','None::<Vec<u8>>'), ('Some(date)','None::<NaiveDate>'), ('Some(time)','None::<NaiveTime>'), ('Some(date.and_time(time))','None::<druid_rdbc::types::RdbcNaiveDateTime>')]
expected = []
for (_, method), (iv, lv) in zip(events, values):
    if method == 'null':
        expected.extend(['"result_set_update_null:1".to_owned()', '"result_set_update_null_by_label:value".to_owned()'])
    else:
        expected.extend([f'format!("result_set_update_{method}:1:{{:?}}", {iv})', f'format!("result_set_update_{method}_by_label:value:{{:?}}", {lv})'])
source = source.replace('assert_eq!(calls(&call_log).len(), 28);', 'assert_eq!(calls(&call_log).len(), 28);\n    assert_eq!(calls(&call_log), vec![\n' + ',\n'.join(expected) + '\n]);')
fixture = '''use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use std::sync::{Arc, Mutex};

/// 完整保留十四类标量下标和标签更新的原事件、参数与短路错误。
/// 对应 Java: Filter#resultSet_updateNull/updateBoolean/updateByte/updateShort/updateInt/updateLong/updateFloat/updateDouble/updateBigDecimal/updateString/updateBytes/updateDate/updateTime/updateTimestamp。
pub(super) struct ScalarUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}
impl ScalarUpdateShortCircuitFilter {
    fn record(&self, event: String) {
        self.calls.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(event);
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for ScalarUpdateShortCircuitFilter {
'''
for label in [False, True]:
    method = 'standard_result_set_update' + ('_by_label' if label else '')
    arg = 'label: &str' if label else 'column: ColumnIndex'
    fixture += f'fn {method}(&self, chain: &mut StandardResultSetFilterChain<\'_>, {arg}, update: ResultSetUpdate) -> Result<(), DruidError> {{\nmatch update {{\n'
    for variant, opname in events:
        prefix = f'result_set_update_{opname}' + ('_by_label:{label}' if label else ':{}')
        if variant == 'Null':
            fmt = f'format!("{prefix}"' + ('' if label else ', column.get()') + ')'
            fixture += f'ResultSetUpdate::Null => {{ self.record({fmt}); Ok(()) }},\n'
        else:
            fmt = f'format!("{prefix}:{{value:?}}"' + ('' if label else ', column.get()') + ')'
            result = 'if label == "fail" { Err(DruidError::DriverError("filtered scalar update failure".to_owned())) } else { Ok(()) }' if label else 'Ok(())'
            fixture += f'ResultSetUpdate::{variant}(value) => {{ self.record({fmt}); {result} }},\n'
    fixture += 'other => chain.result_set_update_by_label(label, other),\n' if label else 'other => chain.result_set_update(column, other),\n'
    fixture += '}\n}\n'
fixture += '}\n'
fixture_path = target.parent / 'diagnostic_tests/scalar_update_short_circuit_filter.rs'
assert not fixture_path.exists()
fixture_path.write_text(fixture, encoding='utf-8')
target.write_text(current + '\nmod scalar_update_short_circuit_filter;\n\n/// 恢复完整原标量更新短路、精确错误与真实SQLite能力合同。\n' + source, encoding='utf-8')
print('Complete scalar update contract and faithful fixture restored; no production changes')
