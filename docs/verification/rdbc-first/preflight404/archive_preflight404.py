from pathlib import Path
import hashlib,json,zipfile,shutil
b=Path(__file__).parent
d=b/'task7-integration-preflight404'
e=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
names=['integration_preflight387.py','integration_preflight393.py','integration_preflight404.py','test_integration_preflight388.py','test_integration_command_identity394.py','test_integration_preflight404.py','prepare_integration_preflight404.py','record_preflight404.py','archive_preflight404.py','verify_candidate403.py']
files={name:b/name for name in names}
files.update({f'evidence/{p.name}':p for p in d.iterdir() if p.is_file()})
manifest={name:sha(p) for name,p in files.items()}
z=b/'task7-integration-preflight404.zip'
assert not z.exists()
with zipfile.ZipFile(z,'w',compression=zipfile.ZIP_DEFLATED) as archive:
    for name,p in files.items():archive.write(p,name)
    archive.writestr('archive_manifest.json',json.dumps(manifest,indent=2)+'\n')
with zipfile.ZipFile(z) as archive:
    assert all(hashlib.sha256(archive.read(name)).hexdigest()==h for name,h in manifest.items())
target=e/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'/z.name
assert not target.exists()
shutil.copyfile(z,target)
assert sha(target)==sha(z)
snapshot=json.loads((d/'snapshot.json').read_text())
note=f'\n\n2026-10-09 当前403集成前检查404：旧393检查器针对新1278输入实际拒绝分母不匹配；独立新检查器绑定原403执行器SHA、319终态来源、新六条精确命令与1258集成路径，保留完整C1/C2、479/442/executed=yes、逐日志SHA和原文件保护。原21项、身份扩展23项与新29项共73测试PASS/exit0。1257正式原文件hash及1278候选冻结输入逐项不变，新helper正式路径未占用；当前状态{snapshot["counts"]}。五个402变更路径需按当前desired SHA集成；原383静态审阅不得冒充更新后的行为认证。真实403预览仍REFUSED：verification still running。仅外部检查器/保护清单更新，未集成/退休正式源码，不认证LLVM/API或发布；18顶层OPEN。zip404 SHA {sha(z)}全部条目及E镜像核验。\n'
for relative in ['.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/progress.md','docs/verification/rdbc-first/status.md','docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md']:
    with (e/relative).open('a',encoding='utf-8',newline='') as out:out.write(note)
print(json.dumps(dict(archive=str(target),sha256=sha(z),bytes=z.stat().st_size,counts=snapshot['counts'],production_applied=False)))
