from copy import deepcopy
from pathlib import Path
import unittest

from test_integration_command_identity394 import CommandIdentityTests, CARGO
from integration_preflight404 import IntegrationRefused, validate_preflight

class Candidate403Tests(CommandIdentityTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.frozen = deepcopy(cls.frozen)
        p = cls.w / 'scripts/windows_bounded_process.py'
        p.parent.mkdir()
        p.write_bytes(b'new bounded process helper')
        import hashlib
        cls.frozen['scripts/windows_bounded_process.py'] = hashlib.sha256(p.read_bytes()).hexdigest()
        cls.snapshot = deepcopy(cls.snapshot)
        cls.snapshot['candidate_frozen_count'] = 1278
        cls.snapshot['selected_source_count'] = 1258
        cls.snapshot['files'].append(dict(path='scripts/windows_bounded_process.py', candidate_sha256=cls.frozen['scripts/windows_bounded_process.py'], e_preimage_sha256=None))
        cls.base = deepcopy(cls.base)
        cls.base['frozen_input_count'] = 1278
        cls.base['original_c1_c2_timeouts_preserved'] = True
        cls.base['parent319_terminal_sha256'] = '36a165c61a9c7f53f0d30fb8323c30539819237fce5a4ab61fcca02adb615469'
        log = cls.logs / 'wrapper-original-prebuild.log'
        log.write_text('original wrapper prebuild complete\n', encoding='utf-8')
        cls.base['commands'].insert(0, dict(name='wrapper-original-prebuild', command=[CARGO,'test','--offline','--locked','-p','druid-wrapper','--all-features','--lib','--','--show-output'],exit_code=0,changed_inputs=[],log=str(log),sha256=hashlib.sha256(log.read_bytes()).hexdigest()))

    def run_gate(self):
        return validate_preflight(self.w,self.e,self.f,self.p,self.s)

    def test_strictness_removed_rejected(self):
        self.s['commands'][2]['command'][-1]='allow'
        self.rejected()

    def test_narrowed_workspace_rejected(self):
        self.s['commands'][1]['command'].remove('--all-features')
        self.rejected()

    def test_preservation_flag_missing_rejected(self):
        self.s['original_c1_c2_timeouts_preserved']=False
        self.rejected()

    def test_new_helper_omitted_rejected(self):
        self.f.pop('scripts/windows_bounded_process.py')
        self.rejected()

    def test_prebuild_cannot_replace_complete_c1(self):
        self.s['commands'].pop()
        self.rejected()

    def test_wrong_parent_evidence_rejected(self):
        self.s['parent319_terminal_sha256']='0'*64
        self.rejected()

    def test_missing_legacy_workspace_metadata_accepted_with_exact_c1_root(self):
        self.s.pop('workspace')
        self.assertEqual(self.run_gate()['state'],'MERGE_REVIEW_PRECONDITIONS_MET_NOT_RELEASE')

    def test_wrong_c1_root_rejected_without_legacy_metadata(self):
        self.s.pop('workspace')
        self.s['commands'][-1]['command'][-1]=str(self.e)
        self.rejected()

    def test_prebuild_scope_substitution_rejected(self):
        self.s['commands'][0]['command'].remove('--all-features')
        self.rejected()

if __name__=='__main__':unittest.main(verbosity=2)
