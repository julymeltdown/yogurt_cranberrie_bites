"""Artifact provenance boundaries only; no compiler or positive authority fixtures."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("operations_live_smoke", Path(__file__).with_name("operations_live_smoke.py"))
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

class ArtifactBoundary(unittest.TestCase):
    def artifact(self, name, binary):
        return {"reason":"compiler-artifact", "executable":str(binary),
                "manifest_path":str(MODULE.ROOT/"backend/services/concert-service/Cargo.toml"),
                "profile":{"test":True,"debuginfo":0},"target":{"name":name,"kind":["test"]}}

    def test_missing_duplicate_or_wrong_manifest_never_freezes(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);binary=root/"fixture";binary.write_bytes(b"fixture, never executed")
            complete=[self.artifact(name,binary) for name in MODULE.TARGETS]
            wrong=[dict(a) for a in complete];wrong[0]["manifest_path"]="/foreign/Cargo.toml"
            for artifacts in [complete[:1],complete+[complete[0]],wrong]:
                path=root/"cargo.json";path.write_text("\n".join(json.dumps(a) for a in artifacts))
                with self.assertRaises(ValueError):MODULE.freeze([path])

    def test_source_inventory_or_copied_binary_changes_refuse_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);binary=root/"fixture";binary.write_bytes(b"fixture, never executed")
            path=root/"cargo.json";path.write_text("\n".join(json.dumps(self.artifact(name,binary)) for name in MODULE.TARGETS))
            # Own the output directory explicitly so unit fixtures leave no files.
            frozen=root/"mcr-operations-live-frozen-unit";frozen.mkdir(mode=0o700)
            with patch.object(MODULE.tempfile,"mkdtemp",return_value=str(frozen)),patch.object(MODULE,"source_pins",return_value={"source":"original"}):
                pin=MODULE.freeze([path]);MODULE.verify(pin)
                with patch.object(MODULE,"source_pins",return_value={"source":"changed"}):
                    with self.assertRaises(ValueError):MODULE.verify(pin)
                (frozen/MODULE.TARGETS[0]).write_bytes(b"changed artifact")
                with self.assertRaises(ValueError):MODULE.verify(pin)

    def test_symlink_artifact_never_freezes(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);binary=root/"fixture";binary.write_bytes(b"fixture")
            link=root/"alias";link.symlink_to(binary)
            path=root/"cargo.json";path.write_text("\n".join(json.dumps(self.artifact(name,link)) for name in MODULE.TARGETS))
            frozen=root/"mcr-operations-live-frozen-unit";frozen.mkdir(mode=0o700)
            with patch.object(MODULE.tempfile,"mkdtemp",return_value=str(frozen)),patch.object(MODULE,"source_pins",return_value={}):
                with self.assertRaises(ValueError):MODULE.freeze([path])

if __name__=="__main__":unittest.main()
