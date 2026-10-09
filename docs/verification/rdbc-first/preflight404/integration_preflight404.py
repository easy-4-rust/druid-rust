from pathlib import Path
import hashlib,re

from integration_preflight387 import IntegrationRefused

def validate_preflight(candidate: Path, formal: Path, frozen: dict, snapshot: dict, summary: dict):
    """只读检查合并前条件；不写文件，也不授权退休或发布。"""
    def require(condition,message):
        if not condition:raise IntegrationRefused(message)
    def digest(path):
        require(path.is_file(),f'missing source or evidence: {path}')
        return hashlib.sha256(path.read_bytes()).hexdigest()
    def contained(root,relative):
        require(isinstance(relative,str) and '\\' not in relative,'noncanonical relative path')
        parts=Path(relative).parts
        require(parts and not Path(relative).is_absolute() and '..' not in parts,'path escapes workspace')
        result=root/relative
        require(result.resolve().is_relative_to(root.resolve()),'resolved path escapes workspace')
        return result
    require(summary.get('terminal') is True,'verification still running')
    require(summary.get('exit_code')==0,'complete verification failed')
    if 'workspace' in summary:
        require(Path(summary['workspace']).resolve()==candidate.resolve(),'verification workspace mismatch')
    require(summary.get('parent319_terminal_sha256')=='36a165c61a9c7f53f0d30fb8323c30539819237fce5a4ab61fcca02adb615469','parent terminal evidence mismatch')
    require(digest(Path(__file__).parent/'verify_candidate403.py')=='bbe85794078ecd135397cda94b27c2106c0278775285def74a6fa27f2b97ce72','original runner provenance mismatch')
    require(summary.get('original_c1_c2_timeouts_preserved') is True,'original scope preservation missing')
    require(len(frozen)==summary.get('frozen_input_count')==snapshot.get('candidate_frozen_count')==1278,'frozen denominator mismatch')
    common=['--workspace','--all-features','--locked','--offline','-j1']
    expected=[('wrapper-original-prebuild',['test','--offline','--locked','-p','druid-wrapper','--all-features','--lib','--','--show-output']),('workspace-complete',['test',*common,'--no-fail-fast']),('workspace-strict',['clippy',*common,'--all-targets','--','-D','warnings']),('workspace-doc',['doc',*common,'--no-deps']),('format-check',['fmt','--all','--','--check']),('c1-complete-runtime',['-X','utf8','scripts/verify_task6_method_parity.py','--repository-root',str(candidate)])]
    commands=summary.get('commands',[]);require(len(commands)==6,'verification command denominator mismatch')
    for record,(name,arguments) in zip(commands,expected):
        require(record.get('name')==name,'verification command identity/order mismatch')
        command=record.get('command',[])
        expected_executable=Path('C:/Espressif/tools/python/python.exe' if name=='c1-complete-runtime' else 'C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/cargo.exe')
        require(command and Path(command[0]).resolve()==expected_executable.resolve(),'verification executable identity mismatch')
        require(command[1:]==arguments,'verification command scope mismatch')
        require(record.get('exit_code')==0 and record.get('changed_inputs')==[],'verification command failed or inputs drifted')
        log=Path(record.get('log',''));require(digest(log)==record.get('sha256'),'verification log hash mismatch')
        if name=='c1-complete-runtime':
            source=log.read_text(encoding='utf-8')
            for key,value in [('TASK6_METHOD_ROWS','479'),('TASK6_METHOD_EVIDENCE_UNIQUE','442'),('TASK6_METHOD_EVIDENCE_EXECUTED','yes')]:
                values=re.findall(r'^'+key+r'=(.*)$',source,re.M)
                require(values and values[-1]==value,'C1 runtime identity/denominator proof missing')
    for relative,expected_hash in frozen.items():require(digest(contained(candidate,relative))==expected_hash,'candidate source drift')
    rows=snapshot.get('files',[]);require(len(rows)==snapshot.get('selected_source_count')==1258,'selected integration denominator mismatch')
    seen=set()
    for row in rows:
        relative=row.get('path');require(relative not in seen,'duplicate integration path');seen.add(relative)
        require(relative in frozen and row.get('candidate_sha256')==frozen[relative],'integration desired hash mismatch')
        path=contained(formal,relative);preimage=row.get('e_preimage_sha256')
        if preimage is None:require(not path.exists(),'new path already occupied')
        else:require(digest(path)==preimage,'formal preimage drift')
    return dict(state='MERGE_REVIEW_PRECONDITIONS_MET_NOT_RELEASE',frozen_inputs=1278,selected_paths=1258,production_applied=False,retirement_authorized=False,compiler_api_or_release_certified=False)
