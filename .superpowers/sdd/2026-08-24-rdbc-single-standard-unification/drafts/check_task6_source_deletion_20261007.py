"""只读运行删除门禁的源码部分，不替代 API、rustdoc 或运行验收。"""
from pathlib import Path
import json, os, subprocess, sys
root = Path.cwd()
sys.path.insert(0, str(root / 'scripts'))
import verify_no_physical_standard as gate
env = dict(os.environ)
env['RUSTUP_AUTO_INSTALL'] = '0'
env['RUSTUP_TOOLCHAIN'] = '1.97.1'
env['CARGO_NET_OFFLINE'] = 'true'
cargo = 'C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/cargo.exe'
result = subprocess.run([cargo, 'metadata', '--locked', '--offline', '--no-deps', '--format-version', '1'], cwd=root, env=env, capture_output=True, text=True, check=True)
metadata = json.loads(result.stdout)
files = gate.verify_source_surface(root, metadata)
gate.verify_no_duplicate_trait_tree(root, metadata)
print('SOURCE_ONLY_PRODUCTION_FILES=' + str(len(files)))
print('SOURCE_ONLY_PHYSICAL_OR_DUPLICATE_TREE=PASS')
print('API_RUSTDOC_METHOD_RUNTIME_AND_REVIEW=NOT_VERIFIED_BY_THIS_CHECK')
