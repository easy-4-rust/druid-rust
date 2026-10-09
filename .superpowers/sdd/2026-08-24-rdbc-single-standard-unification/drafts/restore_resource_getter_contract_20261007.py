from pathlib import Path
import re

root = Path.cwd()
draft = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/complete_resource_getter_base_original_20261007.rs.txt'
target = root / 'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs'
source = target.read_text(encoding='utf-8')
assert 'mod resource_short_circuit_filter;' not in source
test = draft.read_text(encoding='utf-8')
test = test.replace('async fn pooled_result_set_routes_resource_getters_and_real_sqlite_streams()', 'async fn task6_pooled_result_set_routes_resource_getters_and_real_sqlite_streams()')
imports = '''
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
    use druid_rdbc::result_set::ResultSetExt;
    use resource_short_circuit_filter::ResourceShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> { log.lock().unwrap().clone() }
'''
test = test.replace('    let call_log =', imports + '    let call_log =', 1)
start = test.index('    let physical = factory')
end = test.index('    macro_rules!', start)
test = test[:start] + '''    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory, 76, "sqlite-resource-filter", Arc::new(filter_chain),
    ).await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

''' + test[end:]
start = test.index('    let physical = factory.create()')
end = test.index('    assert!(result_set.next', start)
test = test[:start] + '''    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory, 77, "sqlite-resource-pass-through", Arc::new(pass_chain),
    ).await;
    let mut result_set = statement.execute_query("SELECT X'010203' AS payload, '你好' AS text_value").await.unwrap();
''' + test[end:]
test = test.replace('Arc::new(PassThroughResultSetFilter)', 'Arc::new(FilterAdapter::new())')
test = test.replace('(&mut connection, $position)', '(ColumnIndex::new($position).unwrap())').replace('(&mut connection, $name)', '($name)')
test = re.sub(r'\(&mut connection, (\d+)\)', r'(ColumnIndex::new(\1).unwrap())', test)
test = re.sub(r'\(&mut connection, ("[^"]*")\)', r'(\1)', test)
test = test.replace('result_set.next(&mut connection).unwrap()', 'result_set.next().await.unwrap()')
test = test.replace('result_set.close_with_connection(&mut connection).unwrap();', 'result_set.close().await.unwrap();\n    statement.close().await.unwrap();\n    connection.close().await.unwrap();')
expected = []
for pos, stem, label in [(1,'ref','ref'),(2,'blob','blob'),(3,'clob','clob'),(4,'array','array'),(5,'url','url'),(6,'row_id','row_id'),(7,'n_clob','n_clob'),(8,'sql_xml','sql_xml'),(9,'ascii_stream','ascii'),(10,'unicode_stream','unicode'),(11,'binary_stream','binary'),(12,'character_stream','character'),(13,'n_character_stream','n_character')]:
    expected.extend([f'result_set_get_{stem}:{pos}', f'result_set_get_{stem}_by_label:{label}'])
expected.append('result_set_get_sql_xml_by_label:fail')
assertion = '    assert_eq!(calls(&call_log), [\n' + ''.join('        "' + item + '",\n' for item in expected) + '    ]);\n'
test = test.replace('    assert_eq!(calls(&call_log).len(), 27);', '    assert_eq!(calls(&call_log).len(), 27);\n' + assertion)
assert '&mut connection,' not in test
target.write_text(source + '\nmod resource_short_circuit_filter;\n\n/// 完整固定BASE资源getter、共享流游标、精确错误与真实SQLite读取合同。\n' + test, encoding='utf-8', newline='\n')
print('RESTORED_FULL_RESOURCE_GETTER_TEST_WITH_ORDERED_27_EVENTS')
