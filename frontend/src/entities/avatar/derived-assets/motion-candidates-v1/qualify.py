"""Reproduce and qualify only the assigned AR03 candidate art; no installs/adoption."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[6]
TOOLS = Path(__file__).resolve().parent
PUBLIC = ROOT / 'frontend/public/world/avatars/derived-v1/motion-candidates-v1'
EVIDENCE = ROOT / 'docs/mcr/art/evidence/motion-candidates-v1'
BASE = 'a3526254a716e14e3134de551c3b277379a1096f'
NODE = os.environ.get('MCR_ART_NODE') or shutil.which('node')
PLAYWRIGHT = Path(os.environ['MCR_ART_PLAYWRIGHT']).absolute()
OXLINT = PLAYWRIGHT.parents[2] / '.bin/oxlint'
checks = []

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def save(name, value):
    (EVIDENCE / name).write_text(json.dumps(value, indent=2) + '\n')

def run(args, label):
    result = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True, timeout=240)
    output = (result.stdout + result.stderr).replace(str(ROOT), '<checkout>').replace(str(PLAYWRIGHT), '<retained-playwright>').replace(str(NODE), '<node24>')
    (EVIDENCE / (label + '.log')).write_text(output)
    checks.append({'name': label, 'command': [str(a).replace(str(ROOT), '<checkout>').replace(str(NODE), 'node').replace(str(OXLINT), '<retained-bin>/oxlint') for a in args], 'exit': result.returncode})
    print(label + ': exit ' + str(result.returncode), flush=True)
    assert result.returncode == 0, output[-3000:]

run([NODE, '--version'], 'node-version')
assert (EVIDENCE / 'node-version.log').read_text().strip() == 'v24.22.2'
before = {p.name: sha(p) for p in PUBLIC.iterdir() if p.is_file()}
for stem in ['produce', 'produce-rig', 'produce-attachments', 'produce-eyes', 'produce-garments', 'produce-directions', 'produce-components', 'produce-combined']:
    run([NODE, TOOLS / (stem + '.mjs')], stem + '-repeat')
after = {p.name: sha(p) for p in PUBLIC.iterdir() if p.is_file()}
assert before == after, {'changed': [p for p in after if before.get(p) != after[p]]}
checks.append({'name': 'eight-generator-byte-reproduction', 'exit': 0, 'publicFiles': len(after), 'allBytesIdentical': True})
for script in sorted(TOOLS.glob('*.mjs')):
    run([NODE, '--check', script], 'syntax-' + script.stem)
run([OXLINT, TOOLS], 'scoped-lint')
for stem in ['verify', 'verify-gait', 'verify-eyes', 'verify-garments', 'verify-directions', 'verify-components']:
    run([NODE, TOOLS / (stem + '.mjs')], stem)
run(['python3', 'scripts/mcr/check-plan.py'], 'planning-provenance')
run([NODE, 'scripts/mcr/check-ui-origin.mjs'], 'ui-origin')
unchanged = ['docs/source', 'frontend/public/world/avatars/supplied-v1', 'frontend/src/entities/avatar/character-source-pack.json', 'frontend/src/entities/avatar/derived-assets/hair-layout.json', 'frontend/src/entities/avatar/derived-assets/static-package.json', 'frontend/public/world/avatars/derived-v1/static-release.json']
run(['git', 'diff', '--exit-code', BASE, '--', *unchanged], 'original-static-preservation')
inputs = ['MCR_IMPLEMENTATION_PLAN_V2_EN.md', 'docs/mcr/character-art.md', 'frontend/src/entities/avatar/character-source-pack.json', 'frontend/src/entities/avatar/derived-assets/hair-layout.json', 'frontend/src/entities/avatar/derived-assets/mask-presets.mjs', 'frontend/src/entities/avatar/derived-assets/source-layout.json', 'frontend/src/entities/avatar/derived-assets/static-package.json'] + [f'frontend/public/world/themes/{s}.png' for s in ['revenge', 'parade', 'danger']]
pins = {}
for name in inputs:
    path = ROOT / name
    pins[name] = {'sha256': sha(path), 'bytes': path.stat().st_size, 'qualifiedGitBlob': subprocess.check_output(['git', 'rev-parse', 'HEAD:' + name], cwd=ROOT, text=True).strip()}
package = json.loads((PLAYWRIGHT.parent / 'package.json').read_text())
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
save('source-pins.json', {'assignmentBase': BASE, 'qualifiedHead': head, 'inputs': pins, 'dependencies': {'node': '24.22.2', 'playwright': package['version'], 'chrome': json.loads((EVIDENCE / 'wardrobe-validation.json').read_text())['browser']}, 'authoredSources': json.loads((EVIDENCE / 'component-source-inputs.json').read_text()), 'codeProvenance': 'Newly authored rendering/tooling; original mask approach adapted from mask-presets.mjs; read-only selected hair windows. Built-in imagegen for explicitly authored reconstruction; saved exact prompts and immutable outputs.'})
for name in ['failure.jpg', 'gait-failure.jpg', 'direction-failure.jpg', 'wardrobe-failure.jpg']:
    path = EVIDENCE / name
    if path.exists():
        path.unlink()
combined = json.loads((PUBLIC / 'combined-release-candidate.json').read_text())
save('checks.json', {'result': 'PASS', 'qualifiedHead': head, 'qualification': 'Candidate art reproduction, actual Canvas/browser/image/gait/network qualification, source preservation and planning/UI-origin checks. No application/Phaser/device/backend/multiuser/release acceptance.', 'runtimePayload': {'descriptorBytes': (PUBLIC / 'combined-release-candidate.json').stat().st_size, 'atlasBytes': combined['atlas']['bytes'], 'initialAvatarRequests': 2, 'interactionAvatarRequests': 0}, 'checks': checks})
save('delivery-pins.json', {'qualifiedHead': head, 'public': after, 'tools': {str(p.relative_to(ROOT)): sha(p) for p in TOOLS.iterdir() if p.is_file()}, 'evidenceInputs': {name: sha(EVIDENCE / name) for name in ['authored-reconstruction.json', 'component-source-inputs.json', 'authored-turnaround-prompt.txt', 'authored-body-prompt.txt', 'authored-hair-prompt.txt', 'authored-outfit-prompt.txt', 'authored-accessory-prompt.txt']}})
print('PASS: complete scoped art qualification', flush=True)
