import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import bootstrap_smoke as smoke


class ArtifactSelection(unittest.TestCase):
    def test_missing_and_ambiguous_artifacts_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'build.jsonl'
            path.write_text('{}\n')
            with self.assertRaises(smoke.migration.SmokeError): smoke.build_artifacts(path)
            rows=[{'reason':'compiler-artifact','executable':str(Path(directory)/name),
                   'target':{'name':'concert-service','kind':['bin']},'profile':{'test':False}}
                  for name in ['one','two']]
            path.write_text('\n'.join(json.dumps(row) for row in [*rows, {'reason':'build-finished','success':True}]))
            with patch.object(smoke.migration,'executable',side_effect=lambda path:path):
                with self.assertRaises(smoke.migration.SmokeError): smoke.build_artifacts(path)

    def test_only_exact_package_and_target_are_selected(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'build.jsonl'
            rows=[{'reason':'compiler-artifact','executable':str(Path(directory)/'service'),
                   'target':{'name':'concert-service','kind':['bin']},'profile':{'test':False}},
                  {'reason':'compiler-artifact','executable':str(Path(directory)/'test'),
                   'target':{'name':'bootstrap','kind':['test']},'profile':{'test':True},
                   'manifest_path':str(smoke.ROOT/'backend/services/concert-service/Cargo.toml')},
                  {'reason':'compiler-artifact','executable':'/foreign/test',
                   'target':{'name':'bootstrap','kind':['test']},'profile':{'test':True},
                   'manifest_path':'/repo/backend/crates/concert-application/Cargo.toml'}]
            path.write_text('\n'.join(json.dumps(row) for row in [*rows, {'reason':'build-finished','success':True}]))
            with patch.object(smoke.migration,'executable',side_effect=lambda path:path):
                actual=smoke.build_artifacts(path)
            self.assertEqual(actual,{'service':(Path(directory)/'service').resolve(),'tests':(Path(directory)/'test').resolve()})

    def test_native_zero_test_success_is_refused(self):
        from types import SimpleNamespace
        cluster=SimpleNamespace(env={},root=Path('/tmp/private'),token='token',identity=(1,2),url=lambda _: 'private-url')
        with patch.object(smoke.migration,'execute',return_value=SimpleNamespace(returncode=0,stdout='test result: ok. 0 passed; 0 failed; 0 ignored;')):
            with self.assertRaises(smoke.migration.SmokeError): smoke.run_test(cluster,Path('/test'),'missing',{'commands':{}})


if __name__=='__main__': unittest.main()
