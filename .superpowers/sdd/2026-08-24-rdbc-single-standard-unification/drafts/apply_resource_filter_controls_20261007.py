from pathlib import Path
root=Path.cwd()
drafts=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
base=root/'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests'
paths=[(drafts/'resource_read_probe_20261007.rs.txt',base/'resource_read_probe.rs'),(drafts/'ordered_resource_filter_20261007.rs.txt',base/'ordered_resource_filter.rs'),(drafts/'ordered_resource_filter_20261007.rs.txt',root/'crates/druid/tests/resource_filter_support/ordered_resource_filter.rs'),(drafts/'external_resource_filter_contract_20261007.rs.txt',root/'crates/druid/tests/task6_resource_filter_contract_test.rs')]
assert all(not dest.exists() for src,dest in paths)
for src,dest in paths:
    text=src.read_text(encoding='utf-8')
    if dest.parent==base: text=text.replace('use druid::core::','use crate::core::')
    dest.parent.mkdir(parents=True,exist_ok=True)
    dest.write_text(text,encoding='utf-8',newline='\n')
mod=root/'crates/druid/tests/resource_filter_support/mod.rs'
assert not mod.exists()
mod.write_text('mod ordered_resource_filter;\npub use ordered_resource_filter::OrderedResourceFilter;\n',encoding='utf-8')
target=base.with_suffix('.rs')
source=target.read_text(encoding='utf-8')
assert 'mod ordered_resource_filter;' not in source
target.write_text(source+(drafts/'resource_filter_controls_20261007.rs.txt').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
print('APPLIED_PRIVATE_AND_EXTERNAL_RESOURCE_CONTROLS')
