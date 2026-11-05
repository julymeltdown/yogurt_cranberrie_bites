"""PI12 exact-hunk protection. Execute only in a fresh SOURCE allocation."""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
BASE = '6414d3c9aa0d7a10224b3fed89f6f0e5e45d4997'
EXISTING = [
    'frontend/src/app/errors/RouteState.tsx', 'frontend/src/app/router.tsx',
    'frontend/src/app/access.tsx', 'frontend/src/app/AdminBoundary.tsx',
    'frontend/src/app/PublicBoundary.tsx', 'frontend/src/features/connection-status/index.tsx',
    'frontend/src/features/connection-status/status.css',
    'frontend/src/app/operations-composition/panel.tsx',
    'frontend/src/features/safety/panels.tsx', 'frontend/src/app/safety-entry/panels.tsx',
]
PREFIXES = (
    'frontend/src/app/errors/parade-ink/', 'frontend/src/app/__tests__/parade-ink-route-status/',
    'frontend/e2e/visual/parade-ink-route-status/', 'frontend/src/features/safety/parade-ink/report/',
    'frontend/src/features/safety/__tests__/parade-ink-report/', 'frontend/e2e/visual/parade-ink-report/',
    'docs/mcr/evidence/parade-ink-report-error-network/',
)
DOC = 'docs/mcr/frontend/parade-ink-report-error-network.md'


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def protected(text):
    text = re.sub(r"^import ['\"].*parade-ink/.*['\"]\n", '', text, flags=re.M)
    text = text.replace('  <div className="pi-report">\n', '').replace('  </div>\n </SafetyDialog>', ' </SafetyDialog>')
    text = text.replace('<div className="pi-report-entry"><SafetyPanel report /></div>', '<SafetyPanel report />')
    return re.sub(r'\sclassName="[^"]*"', '', text)


def main():
    changed = git('diff', '--name-only', BASE).splitlines()
    untracked = git('ls-files', '--others', '--exclude-standard').splitlines()
    for p in changed + untracked:
        if p == 'frontend/node_modules':
            link = ROOT / p
            assert link.is_symlink() and link.resolve() == (ROOT.parent.parent / 'mcr/frontend/node_modules').resolve(), 'Unapproved dependency link'
            continue
        assert p in EXISTING or p == DOC or p.startswith(PREFIXES), 'Out of scope: ' + p
    for p in EXISTING:
        if p.endswith('.css'):
            continue
        original = git('show', BASE + ':' + p)
        assert protected(original).strip() == protected((ROOT / p).read_text()).strip(), 'Protected logic/copy changed: ' + p
    for p in sorted(set(changed + untracked)):
        if not p.endswith('.css') or '/fixture-overlay/' in p:
            continue
        assert not re.search(r'border-left|border-inline-start|inset-inline-start', (ROOT / p).read_text()), 'Forbidden left accent: ' + p
    for ref, sha in [('codex/mcr-fe-FE17-original-completion', '8ca330d8966e4551ef0fade957e5ce907646108b'),
                     ('codex/mcr-fe-AR03-persistence-motion', 'defdc8fc429e658c7f53523304208056477c6a88'),
                     ('codex/mcr-fe-FE41-original-completion', '4d49d95e6f221cbcc8040f31b0499aa2151ad7ff'),
                     ('codex/mcr-fe-QA05-functional-recovery', '9929fbb488547341a0ecb2e7f2594bc6ef89dd93')]:
        assert git('rev-parse', ref) == sha
    assert 'f543d43a96848d47021eaa4b0736a27e548ebadc' in git('stash', 'list', '--format=%H')
    print('PASS: ten presentation leaves preserve all protected logic/copy; scoped paths and historical refs retained')


if __name__ == '__main__':
    main()
