from pathlib import Path
import subprocess, hashlib, json
root = Path(r'E:\workspaces\workspace-github\druid')
out = Path(r'D:\codex-build\druid-sorter-methods-20261101')
source = 'core/src/test/java/com/alibaba/druid/bvt/pool/ExceptionSorterTest.java'
raw = subprocess.check_output(['git', '-C', str(root), 'show', '33824c3dec1612711f9bb4e409319bcab2e4cd0e:' + source])
assert hashlib.sha256(raw).hexdigest() == 'df66b9a96faec30ea9de73ce60d0bab95dbf11f9c6cadeee74b1d944a21d4a27'
# 原冻结资产采用 Windows checkout CRLF；在独立 D 盘目录重建同一字节资产。
raw = raw.replace(b'\n', b'\r\n')
assert hashlib.sha256(raw).hexdigest() == 'cf1665f28399e9bb17d9e791651d4f5a3941bfe54c29cc5d93e1a7385cabedfd'
target = out / 'frozen-java' / source
target.parent.mkdir(parents=True, exist_ok=True)
target.write_bytes(raw)
classes = sorted((root/'core/src/test/java/com/alibaba/druid/bvt/pool/vendor').glob('*ExceptionSorterTest*.java'))
manifest = [{'path': str(p.relative_to(root)), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in classes + [root/source, root/'core/src/test/java/com/alibaba/druid/PoolTestCase.java']]
(out/'source-tests-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
print('source_test_files', len(classes), 'frozen_mock_sha_verified', True)
