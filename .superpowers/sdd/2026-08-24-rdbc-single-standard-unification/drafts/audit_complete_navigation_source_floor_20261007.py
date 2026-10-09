from pathlib import Path
import sys,dataclasses,json,collections
root=Path.cwd().resolve()
sys.path.insert(0,str(root/'scripts'))
import verify_task6_deleted_test_parity as v
bases=v.derive_base_tests(root)
rows=v.parse_ledger(root/'docs/verification/rdbc-first/task6-deleted-test-migration.tsv')
base_by={b.identity:b for b in bases}
targets=v.current_tests(root,{x.target_source_path for x in rows})
failed=[]
for row in rows:
    b=base_by[row.base_identity]
    frozen=(b.source_path,b.function,b.identity,b.body_sha256,b.behavioral_theme,b.key_assertions,b.key_calls)
    assert frozen==tuple(getattr(row,f.name) for f in dataclasses.fields(v.LedgerRow)[:7])
    try:
        target=targets[row.target_source_path+'#'+row.target_function]
        case=v.disposition_case_id(row)
        if case is None:
            v.verify_observation_floor(b,target)
        else:
            v.verify_parameterized_observation_floor(b,v.parameterized_case_body(target,case))
    except v.VerificationError as e:
        failed.append({'base_identity':row.base_identity,'reason':str(e)})
output={'base':len(bases),'ledger':len(rows),'passed':len(rows)-len(failed),'failed':len(failed),
'exact':sum(x.disposition==v.EXACT for x in rows),'parameterized':sum(v.disposition_case_id(x) is not None for x in rows),
'failures':failed}
path=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-complete-navigation-filter-source-floor-audit-2026-10-07.json'
path.write_bytes((json.dumps(output,ensure_ascii=False,indent=2)+'\n').encode())
print({k:value for k,value in output.items() if k!='failures'})
print('Frozen first seven fields verified for all rows')
