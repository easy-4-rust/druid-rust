from pathlib import Path

b=Path(__file__).parent
source=(b/'integration_preflight393.py').read_text()
source=source.replace("require(Path(summary.get('workspace','')).resolve()==candidate.resolve(),'verification workspace mismatch')", "if 'workspace' in summary:\n        require(Path(summary['workspace']).resolve()==candidate.resolve(),'verification workspace mismatch')\n    require(summary.get('parent319_terminal_sha256')=='36a165c61a9c7f53f0d30fb8323c30539819237fce5a4ab61fcca02adb615469','parent terminal evidence mismatch')\n    require(digest(Path(__file__).parent/'verify_candidate403.py')=='bbe85794078ecd135397cda94b27c2106c0278775285def74a6fa27f2b97ce72','original runner provenance mismatch')")
source=source.replace("summary.get('original_c1_c2_and_timeouts_preserved')", "summary.get('original_c1_c2_timeouts_preserved')")
source=source.replace('1277','1278').replace('1257','1258')
source=source.replace("expected=[('workspace-complete'", "expected=[('wrapper-original-prebuild',['test','--offline','--locked','-p','druid-wrapper','--all-features','--lib','--','--show-output']),('workspace-complete'")
source=source.replace("require(len(commands)==5", "require(len(commands)==6")
assert "==1278" in source and "==1258" in source and "len(commands)==6" in source
assert source.count("parent terminal evidence mismatch")==1
target=b/'integration_preflight404.py'
assert not target.exists()
target.write_text(source,encoding='utf-8')
test=b/'test_integration_preflight404.py'
test.write_text(test.read_text().replace('from integration_preflight393 import','from integration_preflight404 import'),encoding='utf-8')
print('Created a separate six-command checker; original checker and live runner unchanged.')
