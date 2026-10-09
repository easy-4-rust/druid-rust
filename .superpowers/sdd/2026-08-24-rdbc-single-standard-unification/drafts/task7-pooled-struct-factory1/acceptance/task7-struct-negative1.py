"""Compile exact-source negative controls for Struct's Send-only, no-free contract."""
import hashlib
import json
from pathlib import Path
import subprocess
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
candidate=base/'task7-struct-rdbc-candidate1';dest=base/'task7-struct-negative1';dest.mkdir()
positive=json.loads((candidate/'summary.json').read_text(encoding='utf-8'))
assert positive['terminal']
template=next(r['command'] for r in positive['commands'] if r['name']=='green-compile')
old=base/'task7-struct-contract1.rs';payload=old.read_text(encoding='utf-8')
sources={'not-sync':payload+'\nfn rejects_sync() {fn require<T:Sync>() {} require::<DriverStruct>();}\n',
         'no-free':payload.replace('impl Struct for DriverStruct {','impl Struct for DriverStruct {\n    async fn free(&mut self)->Result<(),RdbcError> {Err(RdbcError::feature_not_supported("Struct#free"))}',1)}
state={'production_applied':False,'positive_source_sha256':hashlib.sha256(old.read_bytes()).hexdigest(),'commands':[]}
for name,text in sources.items():
    path=dest/(name+'.rs');path.write_text(text,encoding='utf-8',newline='\n')
    command=template.copy();command[command.index(str(old))]=str(path);command[command.index('-o')+1]=str(dest/(name+'.rmeta'));command+=['--emit=metadata']
    # Hyphens in source filenames require an explicit valid crate name.
    command+=['--crate-name','task7_struct_'+name.replace('-','_')]
    log=dest/(name+'.log')
    with log.open('xb') as output:result=subprocess.run(command,stdout=output,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    diagnostic=log.read_text(encoding='utf-8');code='E0277' if name=='not-sync' else 'E0407'
    assert result.returncode!=0 and code in diagnostic
    if name=='not-sync':assert 'Cell<usize>' in diagnostic
    else:assert 'method `free` is not a member of trait `Struct`' in diagnostic
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'expected_diagnostic':code,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()})
    print(json.dumps({'name':name,'exit_code':result.returncode,'diagnostic':code}),flush=True)
state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
