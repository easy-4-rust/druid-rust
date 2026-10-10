from pathlib import Path
import hashlib,json,subprocess,shutil
b=Path(__file__).parent
w=Path('F:/codex_build_cache/d177')
e=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
run=b/'task7-candidate-verification403'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
frozen=json.loads((run/'frozen-inputs.json').read_text())
relative='crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs'
source=w/relative
assert sha(source)==frozen[relative]
original=source.read_bytes()
d=b/'task7-format-draft406'
d.mkdir(exist_ok=False)
draft=d/'sqlx_connection_actor.rs'
draft.write_bytes(original)
rustfmt='C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustfmt.exe'
records=[]
for name,args,expected in [('red-check',['--check'],1),('format',[],0),('green-check',['--check'],0)]:
    command=[rustfmt,'--edition','2021','--config','skip_children=true',*args,str(draft)]
    with (d/(name+'.log')).open('wb') as out:
        result=subprocess.run(command,stdout=out,stderr=subprocess.STDOUT)
    assert result.returncode==expected,(name,result.returncode)
    records.append(dict(name=name,command=command,exit_code=result.returncode,log_sha256=sha(d/(name+'.log'))))
before=b'use tokio::sync::{mpsc, oneshot, watch};\nuse tokio::sync::Mutex;'
after=b'use tokio::sync::Mutex;\nuse tokio::sync::{mpsc, oneshot, watch};'
desired=draft.read_bytes()
assert original.count(before)==1
assert desired==original.replace(before,after,1),'unexpected formatting changes'
assert desired.replace(after,before,1)==original
assert all(sha(w/p)==h for p,h in frozen.items())
record=dict(scope='SINGLE_IMPORT_ORDER_DRAFT_NOT_APPLIED_TO_FROZEN_CANDIDATE',path=relative,
    candidate_preimage_sha256=sha(source),desired_sha256=sha(draft),commands=records,
    reverse_transform_byte_identical=True,frozen_inputs_unchanged=1278,
    production_applied=False,full_workspace_format_after_apply_not_verified=True)
(d/'summary.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
(d/'original.rs.txt').write_bytes(original)
formal=e/'docs/verification/rdbc-first/format406'
assert not formal.exists()
formal.mkdir()
for name in ['red-check.log','format.log','green-check.log','summary.json','original.rs.txt']:
    shutil.copyfile(d/name,formal/name)
shutil.copyfile(draft,formal/'desired.rs.txt')
shutil.copyfile(Path(__file__),formal/'prepare_format406.py')
shutil.copyfile(run/'format-check.log',formal/'original-workspace-format-failure.log')
(formal/'README.md').write_text('# 格式修复候选 406\n\n原完整格式检查失败，仅 SQLx actor 的两个 use 声明顺序不符合 rustfmt。\n本目录保存原源码、格式修正源码及 RED/GREEN 日志；原冻结候选未修改。\n逆变换完整字节一致，没有修改导入内容或业务逻辑。\n必须在当前完整验收终态后应用，并重新执行完整格式检查及受影响验收。\n',encoding='utf-8')
with (e/'.gitignore').open('a',encoding='utf-8',newline='') as out:
    out.write('!/docs/verification/rdbc-first/format406/**/*.json\n!/docs/verification/rdbc-first/format406/**/*.log\n')
with (e/'.gitattributes').open('a',encoding='utf-8',newline='') as out:
    out.write('/docs/verification/rdbc-first/format406/** -text whitespace=cr-at-eol,-blank-at-eof\n')
note=f'\n\n2026-10-09 当前403严格/doc/fmt阶段及修复草稿406：strict全workspace/allfeatures/alltargets/-D warnings实际exit0，日志SHA d540c44fe749ec63c79f57e08f2788dfb0ad957d8aa4d5c48920511778dba4ae；doc/no-deps exit0，日志SHA f0ed830f5eed9f5cf99b082ffaf4a3805e291a44a3f0e4be360dbff34e79706e。完整fmt实际exit1，仅sqlx_connection_actor.rs两use顺序，原日志保留。冻结外同一文件原rustfmt RED exit1->格式化->GREEN exit0；仅调换两个导入顺序，逆变换逐字节相等，1278冻结输入不变。草稿desired SHA {sha(draft)}，普通源码/日志在docs/verification/rdbc-first/format406/；未apply，不宣称完整fmt或C1/C2通过。原session56719完整C1/C2继续，正式候选未集成，18顶层OPEN。\n'
for path in ['.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/progress.md','docs/verification/rdbc-first/status.md','docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md']:
    with (e/path).open('a',encoding='utf-8',newline='') as out:out.write(note)
print(json.dumps(record))
