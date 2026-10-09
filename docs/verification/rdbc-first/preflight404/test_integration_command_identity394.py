from copy import deepcopy
import unittest
from test_integration_preflight388 import PreflightTests
from integration_preflight393 import IntegrationRefused, validate_preflight

CARGO='C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/cargo.exe'
PYTHON='C:/Espressif/tools/python/python.exe'

class CommandIdentityTests(PreflightTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.base=deepcopy(cls.base)
        for record in cls.base['commands']:
            record['command'][0]=PYTHON if record['name']=='c1-complete-runtime' else CARGO

    def run_gate(self):
        return validate_preflight(self.w,self.e,self.f,self.p,self.s)

    def test_substituted_cargo_executable_rejected(self):
        self.s['commands'][0]['command'][0]='F:/unrelated/cargo.exe'
        with self.assertRaisesRegex(IntegrationRefused,'executable identity'):self.run_gate()

    def test_substituted_python_executable_rejected(self):
        self.s['commands'][-1]['command'][0]='F:/unrelated/python.exe'
        with self.assertRaisesRegex(IntegrationRefused,'executable identity'):self.run_gate()

if __name__=='__main__':unittest.main(verbosity=2)
