"""Reuse original recorder with terminal summary; no WMI liveness query after exit."""
from pathlib import Path
b=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
source=(b/'task6-current-workspace-serial2-record.py').read_text(encoding='utf-8')
lines=source.splitlines(keepends=True)
matches=[i for i,line in enumerate(lines)if line.startswith('payload=subprocess.check_output')]
assert len(matches)==1
lines[matches[0]]="assert serial['terminal']; process=None\n"
exec(compile(''.join(lines),str(b/'task6-current-workspace-serial2-record.py'),'exec'))
