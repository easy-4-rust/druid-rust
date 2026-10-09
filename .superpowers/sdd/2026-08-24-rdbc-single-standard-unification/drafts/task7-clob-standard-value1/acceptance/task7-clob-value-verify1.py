"""Verify full Clob candidate against forced lock contention and single-source mutations."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');rdbc=base/'task7-clob-value-rdbc-candidate1';dest=base/'task7-clob-value-druid-candidate1';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
original=json.loads((dest/'summary.json').read_text(encoding='utf-8'));assert original['terminal'];state={'terminal':False,'commands':[],'production_applied':False}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)});(dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-clob-value-consumer1.rs';source=base/'task7-pooled-clob-value-consumer2.rs';text=prior.read_text(encoding='utf-8')
lock=(base/'task7-blob-value-lock-order2.rs').read_text(encoding='utf-8').replace('task7_blob_value_blocked_first_locks_preserve_global_order','blocked_first_locks_preserve_global_order').replace('blob_value_probe','probe').replace('blob_value(','clob(').replace('ResourceOperations::blob()','ResourceOperations::clob()').replace('position_blob','position_clob')
assert text.rstrip().endswith('}');text=text.rstrip()[:-1]+lock+'\n}\n';source.write_text(text,encoding='utf-8',newline='\n')
consumer=next(row['command'] for row in original['commands'] if row['name']=='consumer-compile').copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer2.exe')
assert run('consumer2-compile',consumer)==0;assert run('consumer2-runtime',[str(dest/'consumer2.exe'),'--test-threads=1'])==0
for name,cfg,code in [('negative-sync','task7_require_clob_value_sync','E0277'),('negative-old-access','task7_require_old_clob_access','E0432')]:
    command=consumer.copy();command[command.index('-o')+1]=str(dest/(name+'.exe'));command+=['--cfg',cfg];assert run(name,command)!=0;assert code in (dest/(name+'.log')).read_text(encoding='utf-8')
for variant,test in [('capability','all_capability_combinations_and_freed_precedence'),('self-alias','self_pattern_preserves_start_and_no_snapshot'),('lock-order','blocked_first_locks_preserve_global_order')]:
    root=base/('task7-clob-value-'+variant+'-mutation1');root.mkdir();rr=root/'rdbc';dd=root/'druid';shutil.copytree(rdbc/'src',rr/'src');shutil.copytree(dest/'src',dd/'src');shutil.copytree(dest/'resources',dd/'resources')
    p=rr/'src/resource/rdbc_clob.rs';text=p.read_text(encoding='utf-8')
    if variant=='capability':
        needle='.lock(ResourceOperations::READ, "Clob#length")';assert text.count(needle)==1;text=text.replace(needle,'.lock(ResourceOperations::empty(), "Clob#length")')
    elif variant=='self-alias':
        start=text.index('            // 单次独占借用');end=text.index('\n        }',start);text=text[:start]+'            return Ok(Some(1));'+text[end:]
    else:
        needle='if self.resource_id() < pattern.resource_id() {';assert text.count(needle)==1;text=text.replace(needle,'if true {')
    p.write_text(text,encoding='utf-8',newline='\n')
    library=next(row['command'] for row in original['commands'] if row['name']=='full-rdbc-compile').copy();library[library.index(str(rdbc/'src/lib.rs'))]=str(rr/'src/lib.rs');library[library.index('--out-dir')+1]=str(rr)
    for i,arg in enumerate(library):
        if arg.startswith('metadata='):library[i]='metadata=task7_clob_'+variant.replace('-','_')
    assert run(variant+'-rdbc-compile',library)==0
    library=next(row['command'] for row in original['commands'] if row['name']=='full-druid-compile').copy();library[library.index(str(dest/'src/lib.rs'))]=str(dd/'src/lib.rs');library[library.index('-o')+1]=str(dd/'libdruid.rlib')
    for i,arg in enumerate(library):
        if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rr/'libdruid_rdbc.rlib')
        if arg.startswith('metadata='):library[i]='metadata=task7_clob_druid_'+variant.replace('-','_')
    env=os.environ.copy();env.update(original['druid_environment']);env['CARGO_MANIFEST_DIR']=str(dd)
    assert run(variant+'-druid-compile',library,env)==0
    command=consumer.copy();command[command.index('-o')+1]=str(root/'consumer.exe')
    for i,arg in enumerate(command):
        if arg.startswith('druid_rdbc='):command[i]='druid_rdbc='+str(rr/'libdruid_rdbc.rlib')
        if arg.startswith('druid='):command[i]='druid='+str(dd/'libdruid.rlib')
    command+=['-L','dependency='+str(rr)];assert run(variant+'-consumer-compile',command)==0
    assert run(variant+'-runtime',[str(root/'consumer.exe'),'clob_value_contract::'+test,'--exact','--test-threads=1'])!=0
    state[variant]={'root':str(root),'changed_source':'src/resource/rdbc_clob.rs','sources':{p.relative_to(root).as_posix():sha(p) for p in root.rglob('*.rs')},'rlib_sha256':sha(dd/'libdruid.rlib'),'rdbc_rlib_sha256':sha(rr/'libdruid_rdbc.rlib'),'exe_sha256':sha(root/'consumer.exe')}
state['consumer_sha256']=sha(source);state['exe_sha256']=sha(dest/'consumer2.exe');state['terminal']=True
(dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
