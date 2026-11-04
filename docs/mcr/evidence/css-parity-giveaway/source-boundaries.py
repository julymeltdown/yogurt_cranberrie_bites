"""Owner boundary evidence; run only inside the named output allocation."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

repo = Path(__file__).resolve().parents[4]
base = '54320d71c4ae0bff3524d1b500cea772e0ba42e0'
owned_css = {
    'frontend/src/features/gift-discovery/ui/gifts.css',
    'frontend/src/features/gift-discovery/ui/reference-gifts.css',
}


def git(*args):
    return subprocess.check_output(['git', *args], cwd=repo)


changed = set(git('diff', '--name-only', base).decode().splitlines())
changed.update(git('ls-files', '--others', '--exclude-standard').decode().splitlines())
temporary_paths = []
link = repo / 'frontend/node_modules'
if link.is_symlink() and str(link.readlink()) == '/Users/lhs/Desktop/mcr/frontend/node_modules':
    changed.discard('frontend/node_modules')
    temporary_paths.append('frontend/node_modules: authorized own ephemeral read-only dependency link')
allowed = lambda p: p in owned_css or p == 'docs/mcr/frontend/css-parity-giveaway.md' or p.startswith('docs/mcr/evidence/css-parity-giveaway/')
assert all(allowed(p) for p in changed), sorted(p for p in changed if not allowed(p))
checks = []
for path in sorted(owned_css):
    old = git('show', base + ':' + path).decode()
    new = (repo / path).read_text()
    # Exact decorative paint preservation, including values and declaration order.
    paints = lambda s: re.findall(r'\bbackground(?:-[\w-]+)?\s*:[^;}]*', re.sub(r'/\*.*?\*/', '', s, flags=re.S))
    assert paints(old) == paints(new), path + ': background paint changed'
    assert not re.search(r'\b(?:border-left|border-inline-start)\s*:', new), path + ': left rail'
    assert not re.search(r'(?:[;{]\s*)content\s*:', new), path + ': generated product text'
    checks.append({'path': path, 'sha256': hashlib.sha256(new.encode()).hexdigest(), 'decorativePaintPreserved': True, 'noGeneratedText': True, 'noLeftRails': True})
assert not git('diff', base, '--', 'docs/source'), 'Source archive changed'
subprocess.run(['git', 'diff', '--check', base], cwd=repo, check=True)
print(json.dumps({'result': 'PASS', 'base': base, 'head': git('rev-parse', 'HEAD').decode().strip(), 'changed': sorted(changed), 'checks': checks, 'temporaryPaths': temporary_paths, 'sourceArchivePreserved': True, 'nonCssProductFilesPreserved': True}, indent=2))
