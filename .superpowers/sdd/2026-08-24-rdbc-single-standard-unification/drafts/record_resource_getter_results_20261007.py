from pathlib import Path
import json,hashlib,re
root=Path.cwd()
e=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
for stem,session,exit_code,expected in [('interface-red',18518,101,(0,0)),('runtime-red',59755,101,(131,1)),('routing-green',68875,0,(132,0)),('controls',98890,0,(135,0))]:
    path=e/f'task6-resource-getter-{stem}-2026-10-07.log'
    text=path.read_text(encoding='utf-8-sig')
    blocks=re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
    passed=sum(int(x[0]) for x in blocks);failed=sum(int(x[1]) for x in blocks)
    assert (passed,failed)==expected
    data={'state':'TERMINAL_PASSED' if exit_code==0 else 'TERMINAL_FAILED','exit_code':exit_code,'session_id':session,'toolchain':'1.97.1','log':path.relative_to(root).as_posix(),'log_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'passed':passed,'failed':failed,'ignored':sum(int(x[2]) for x in blocks),'result_blocks':len(blocks),'scope':'Druid lib + external resource consumer' if stem=='controls' else 'complete Druid lib; interface-red compiled no tests'}
    if stem=='controls':
        freeze=json.loads((e/'task6-resource-getter-controls-inputs-2026-10-07.json').read_text())
        # 已移除一个外部测试unused import；终态核对输出BUILD_INPUT_CHANGES []已在该修改前取得。
        data['unchanged_frozen_inputs_at_terminal']=1063
        data['post_terminal_change']='remove unused Pool import from external test only; rerun required'
    (e/f'task6-resource-getter-{stem}-2026-10-07.summary.json').write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8')
    print(stem,passed,failed,data['log_sha256'])
