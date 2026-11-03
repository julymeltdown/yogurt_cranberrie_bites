"""Evidence integrity checks; does not claim native database qualification."""
import hashlib
import tempfile
import unittest
from pathlib import Path
import mural_smoke as smoke

class EvidenceGuards(unittest.TestCase):
    def test_missing_stale_and_mismatched_build_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            binary=Path(directory)/'mural-test'
            binary.write_bytes(b'native fixture')
            sources={'owned.rs':'current'}
            valid={'exit_code':0,'source_sha256':sources,'binaries':{str(binary):hashlib.sha256(binary.read_bytes()).hexdigest()}}
            smoke.verify_build_receipt(valid,sources,[binary])
            for invalid in [{}, {**valid,'exit_code':1}, {**valid,'source_sha256':{'owned.rs':'stale'}}, {**valid,'binaries':{str(binary):'wrong'}}]:
                with self.assertRaises(RuntimeError): smoke.verify_build_receipt(invalid,sources,[binary])
            binary.write_bytes(b'changed after build')
            with self.assertRaises(RuntimeError): smoke.verify_build_receipt(valid,sources,[binary])

    def test_current_source_manifest_includes_effect_and_replay_producers(self):
        paths=smoke.source_paths()
        for suffix in ['src/adapters/out/repository/mural.rs','src/adapters/out/repository/receipt.rs','src/commands/mod.rs','tests/mural.rs','src/world/mural_subscription.rs']:
            self.assertTrue(any(path.endswith(suffix) for path in paths),suffix)
        self.assertTrue(all(len(value)==64 for value in paths.values()))

if __name__=='__main__': unittest.main()
