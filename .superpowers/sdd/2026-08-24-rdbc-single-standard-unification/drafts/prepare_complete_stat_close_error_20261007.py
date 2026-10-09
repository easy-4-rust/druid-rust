from pathlib import Path
root = Path.cwd()
draft = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
original = (draft / 'complete_stat_close_error_base_original_20261007.rs.txt').read_text(encoding='utf-8')
body = original.replace('let physical = PhysicalResultSetProbe::new(call_log, true);',
    'let observations = Arc::new(Mutex::new(Vec::new()));\n    let mut physical = StatCloseErrorResultSetProbe::new(Arc::clone(&call_log), Arc::clone(&collector), Arc::clone(&observations));')
body = body.replace('let mut chain = FilterChain::new();', 'let mut chain = StatCloseErrorChainHarness::new();')
body = body.replace('chain.result_set_close(&physical, &context),', 'chain.result_set_close(&mut physical, &context).await,')
body = body.rstrip()
assert body.endswith('}')
body = body[:-1] + '''    assert_eq!(*call_log.lock().unwrap(), ["physical:close"]);
    assert_eq!(*observations.lock().unwrap(), ["1:0:3:1"]);
    assert!(physical.is_closed().unwrap());
}
'''
imports = '''
    use crate::core::{DruidError, RdbcError, ResultSetFilterContext};
    use crate::stats::{StatFilter, StatsCollector};
    use std::sync::Mutex;
    use std::time::Duration;
    use stat_close_error_chain_harness::StatCloseErrorChainHarness;
    use stat_close_error_result_set_probe::StatCloseErrorResultSetProbe;
'''
body = body.replace('{', '{' + imports, 1)
name = 'stat_filter_commits_close_statistics_before_downstream_close_error'
target = root / 'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs'
text = target.read_text(encoding='utf-8')
assert 'async fn task6_' + name not in text
text += '\nmod stat_close_error_chain_harness;\nmod stat_close_error_result_set_probe;\n\n/// 保留原全部断言，并在下游关闭入口验证统计提交先于精确错误。\n#[tokio::test]\nasync fn task6_' + name + '() ' + body
target.write_bytes(text.encode('utf-8'))
old = (draft / 'bind_complete_stream_update_defaults_20261007.py').read_text(encoding='utf-8')
old = old.replace('result_set_stream_update_filter_defaults_preserve_all_twenty_two_overloads', name).replace('stream-update-defaults', 'stat-close-error').replace('Stream update defaults', 'Stat close error')
dest = draft / 'bind_complete_stat_close_error_20261007.py'
assert not dest.exists()
dest.write_bytes(old.encode('utf-8'))
old = (draft / 'audit_complete_stream_update_defaults_source_floor_20261007.py').read_text(encoding='utf-8')
dest = draft / 'audit_complete_stat_close_error_source_floor_20261007.py'
assert not dest.exists()
dest.write_bytes(old.replace('stream-update-defaults', 'stat-close-error').encode('utf-8'))
print('Full original case restored; evidence binding scripts prepared, not executed')
