from pathlib import Path
from copy import deepcopy
import hashlib,json,tempfile,unittest
from integration_preflight387 import IntegrationRefused,validate_preflight
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
class PreflightTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp=tempfile.TemporaryDirectory(dir='F:/codex_tmp');cls.root=Path(cls.tmp.name);cls.w=cls.root/'candidate';cls.e=cls.root/'formal';cls.w.mkdir();cls.e.mkdir()
        cls.frozen={}
        for i in range(1277):
            p=cls.w/f'source_{i}.rs';p.write_bytes(b'owned candidate');cls.frozen[p.name]=sha(p)
        (cls.e/'source_0.rs').write_bytes(b'user original')
        cls.snapshot=dict(candidate_frozen_count=1277,selected_source_count=1257,files=[dict(path=f'source_{i}.rs',candidate_sha256=cls.frozen[f'source_{i}.rs'],e_preimage_sha256=sha(cls.e/'source_0.rs') if i==0 else None) for i in range(1257)])
        cls.logs=cls.root/'logs';cls.logs.mkdir();cls.commands=[]
        common=['--workspace','--all-features','--locked','--offline','-j1']
        specs=[('workspace-complete',['cargo','test',*common,'--no-fail-fast']),('workspace-strict',['cargo','clippy',*common,'--all-targets','--','-D','warnings']),('workspace-doc',['cargo','doc',*common,'--no-deps']),('format-check',['cargo','fmt','--all','--','--check']),('c1-complete-runtime',['python','-X','utf8','scripts/verify_task6_method_parity.py','--repository-root',str(cls.w)])]
        for name,cmd in specs:
            p=cls.logs/(name+'.log');p.write_text('TASK6_METHOD_ROWS=479\nTASK6_METHOD_EVIDENCE_UNIQUE=442\nTASK6_METHOD_EVIDENCE_EXECUTED=yes\n' if name=='c1-complete-runtime' else 'completed original scope\n',encoding='utf-8');cls.commands.append(dict(name=name,command=cmd,exit_code=0,changed_inputs=[],log=str(p),sha256=sha(p)))
        cls.base=dict(terminal=True,exit_code=0,workspace=str(cls.w),frozen_input_count=1277,original_c1_c2_and_timeouts_preserved=True,commands=cls.commands)
    @classmethod
    def tearDownClass(cls):cls.tmp.cleanup()
    def setUp(self):self.s=deepcopy(self.base);self.f=deepcopy(self.frozen);self.p=deepcopy(self.snapshot)
    def run_gate(self):return validate_preflight(self.w,self.e,self.f,self.p,self.s)
    def rejected(self):
        with self.assertRaises(IntegrationRefused):self.run_gate()
    def test_valid_preview_preserves_user_bytes_and_does_not_apply(self):
        before=sha(self.e/'source_0.rs');result=self.run_gate();self.assertEqual(result['state'],'MERGE_REVIEW_PRECONDITIONS_MET_NOT_RELEASE');self.assertEqual(sha(self.e/'source_0.rs'),before);self.assertFalse((self.e/'source_1.rs').exists())
    def test_running_summary_rejected(self):self.s['terminal']=False;self.rejected()
    def test_failed_gate_rejected(self):self.s['commands'][0]['exit_code']=101;self.rejected()
    def test_missing_gate_rejected(self):self.s['commands'].pop();self.rejected()
    def test_duplicate_gate_rejected(self):self.s['commands'][1]=deepcopy(self.s['commands'][0]);self.rejected()
    def test_narrowed_workspace_rejected(self):self.s['commands'][0]['command'].remove('--all-features');self.rejected()
    def test_strictness_removed_rejected(self):self.s['commands'][1]['command'][-1]='allow';self.rejected()
    def test_no_execute_c1_rejected(self):self.s['commands'][-1]['command'].append('--no-execute');self.rejected()
    def test_log_substitution_rejected(self):self.s['commands'][0]['sha256']='0'*64;self.rejected()
    def test_candidate_hash_drift_rejected(self):self.f['source_0.rs']='0'*64;self.rejected()
    def test_formal_preimage_drift_rejected(self):self.p['files'][0]['e_preimage_sha256']='0'*64;self.rejected()
    def test_occupied_new_path_rejected(self):self.p['files'][0]['e_preimage_sha256']=None;self.rejected()
    def test_path_escape_rejected(self):self.p['files'][0]['path']='../outside.rs';self.rejected()
    def test_duplicate_snapshot_path_rejected(self):self.p['files'][1]=deepcopy(self.p['files'][0]);self.rejected()
    def test_source_denominator_reduced_rejected(self):self.f.pop('source_1276.rs');self.rejected()
    def test_snapshot_candidate_hash_mismatch_rejected(self):self.p['files'][0]['candidate_sha256']='0'*64;self.rejected()
    def test_preservation_flag_missing_rejected(self):self.s['original_c1_c2_and_timeouts_preserved']=False;self.rejected()
    def test_wrong_workspace_rejected(self):self.s['workspace']=str(self.e);self.rejected()
    def test_reduced_selected_denominator_rejected(self):self.p['files'].pop();self.p['selected_source_count']=1256;self.rejected()
    def test_final_exit_failed_rejected(self):self.s['exit_code']=1;self.rejected()
    def test_c1_source_only_log_rejected_even_with_valid_hash(self):
        record=self.s['commands'][-1];path=self.root/'source-only.log';path.write_text('TASK6_METHOD_ROWS=479\nTASK6_METHOD_EVIDENCE_UNIQUE=442\nTASK6_METHOD_EVIDENCE_EXECUTED=no\n',encoding='utf-8');record['log']=str(path);record['sha256']=sha(path)
        with self.assertRaisesRegex(IntegrationRefused,'C1 runtime'):self.run_gate()
if __name__=='__main__':unittest.main(verbosity=2)
