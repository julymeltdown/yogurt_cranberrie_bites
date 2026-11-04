#!/usr/bin/env python3
"""Use the actual granted generators for all four named OpenAPI outputs."""
import importlib.util
from pathlib import Path
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[4]
for name in ['generate-mcr-core.py','generate-mcr-domain.py']:
    subprocess.run([sys.executable,str(ROOT/'backend/scripts/docs'/name)],check=True)
# The retained domain CLI has one output variable; use its unchanged writer for
# the explicitly granted backend mirror as well, with the identical source graph.
p=ROOT/'backend/scripts/docs/generate-mcr-domain.py'
spec=importlib.util.spec_from_file_location('reviewed_domain_mirror',p)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
m.OUTPUT=ROOT/'backend/docs/api/mcr-domain.openapi.json'
m.main()
assert (ROOT/'contracts/generated/mcr-domain.openapi.json').read_bytes()==m.OUTPUT.read_bytes()
