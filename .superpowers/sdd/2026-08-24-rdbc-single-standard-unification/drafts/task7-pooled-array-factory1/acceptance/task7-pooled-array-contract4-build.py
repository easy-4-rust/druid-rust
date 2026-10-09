"""Use Result status in panic text without adding Debug to canonical ResultSet."""
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
source=(base/'task7-pooled-array-consumer3.rs').read_text(encoding='utf-8')
old='panic!("cursor should block: {result:?}")'
assert source.count(old)==1
with (base/'task7-pooled-array-consumer4.rs').open('x',encoding='utf-8',newline='\n') as output:
    output.write(source.replace(old,'panic!("cursor should block, completed_ok={}",result.is_ok())'))
runner=(base/'task7-pooled-array-contract3-run.py').read_text(encoding='utf-8')
runner=runner.replace('task7-pooled-array-consumer3.rs','task7-pooled-array-consumer4.rs').replace('contract3.summary.json','contract4.summary.json')
runner=runner.replace("'before3'","'before4'").replace("'after3'","'after4'")
with (base/'task7-pooled-array-contract4-run.py').open('x',encoding='utf-8',newline='\n') as output: output.write(runner)
print('Kept ResultSet canonical bounds unchanged; retained consumer3 E0277 setup log.')
