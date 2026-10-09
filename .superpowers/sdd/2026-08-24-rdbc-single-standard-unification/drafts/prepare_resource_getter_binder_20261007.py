from pathlib import Path
root=Path.cwd()
drafts=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
source=(drafts/'bind_complete_object_update_filter_20261007.py').read_text(encoding='utf-8')
source=source.replace('task6-complete-object-update-filter-initial-2026-10-07.summary.json','task6-resource-getter-controls-2026-10-07.summary.json')
source=source.replace('pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors','pooled_result_set_routes_resource_getters_and_real_sqlite_streams')
source=source.replace('task6-complete-object-update-filter-inputs-2026-10-07.json','task6-resource-getter-controls-inputs-2026-10-07.json')
old="assert all(hashlib.sha256((root / name).read_bytes()).hexdigest() == sha for name, sha in freeze['inputs'].items())"
new="""for name, sha in freeze['inputs'].items():
    data=(root/name).read_bytes()
    if name=='crates/druid/tests/task6_resource_filter_contract_test.rs':
        # 唯一终态后调整是移除unused import；重构旧字节逐项核对，不能忽略文件。
        assert data.count(b'use druid::core::{FilterChain, RdbcError};')==1
        data=data.replace(b'use druid::core::{FilterChain, RdbcError};',b'use druid::core::{FilterChain, Pool, RdbcError};')
    assert hashlib.sha256(data).hexdigest()==sha, name"""
assert source.count(old)==1
source=source.replace(old,new).replace('Object update full BASE','Resource getter full BASE')
path=drafts/'bind_complete_resource_getter_filter_20261007.py'
assert not path.exists()
path.write_text(source,encoding='utf-8')
print('BINDER_PREPARED_WITH_ALL_INPUT_HASH_GUARDS')
