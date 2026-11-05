"""New PI12-only ownership and preserved-behavior checks; allocated execution only."""
from pathlib import Path
import json
import re
import subprocess as sp

root = Path(__file__).resolve().parents[4]
base = '8b47e0e40e8182d2401565fbb079533efffb41e7'
world = 'frontend/src/app/WorldShell.tsx'
notice = 'frontend/src/app/pre-world-policy/PolicyNotice.tsx'
def original(path):
    return sp.check_output(['git', 'show', base + ':' + path], cwd=root, text=True)
def current(path):
    return (root / path).read_text()
def feedback(source):
    return source[source.index('    <Feedback'):source.index('    </Feedback>') + len('    </Feedback>')]
before, after = original(world), current(world)
added = "import { PreWorldFrame } from './pre-world-policy/parade-ink/PreWorldFrame'\n"
assert after.count(added) == 1
after_without_import = after.replace(added, '')
marker = "  if (result.status !== 'ready') return "
assert before.split(marker)[0] == after_without_import.split(marker)[0], 'All bootstrap logic before fallback is byte-exact'
ready = '  return <WorldShell scope='
assert before.split(ready)[1] == after_without_import.split(ready)[1], 'Ready return and ALL downstream World/PI08 source are byte-exact'
assert feedback(before) == feedback(after), 'Fallback feedback copy and predicate are exact'
for fragment in [
    "{result.status === 'loading' ? 'Opening your event' : {\n      unauthenticated: 'Sign in to continue', denied: 'Access denied', 'not-found': 'Event not found',\n      gone: 'This event is no longer available', 'rate-limited': 'Please wait before trying again', unavailable: 'Your event is unavailable',\n    }[result.reason]}",
    "{result.status === 'error' && <Button onClick={() => setAttempt(n => n + 1)}>Try again</Button>}",
    '<Link className="shell-link" to="/">Back to home</Link>',
]:
    assert fragment in before and fragment in after, fragment

before_notice, after_notice = original(notice), current(notice)
added_notice = "import { PreWorldFrame } from './parade-ink/PreWorldFrame'\n"
assert after_notice.replace(added_notice, '').split('  return ')[0] == before_notice.split('  return ')[0], 'PolicyView/title/pending/maintenance/version predicates are byte-exact'
assert feedback(before_notice) == feedback(after_notice), 'Every policy feedback string and minimum version expression is byte-exact'
for fragment in [
    '{required && <Button onClick={reload}>Reload app</Button>}',
    '{!pending && <Button variant="secondary" onClick={retry}>Check again</Button>}',
    '<Link className="shell-link" to="/">Back to home</Link>',
]:
    assert fragment in before_notice and fragment in after_notice, fragment
frame = current('frontend/src/app/pre-world-policy/parade-ink/PreWorldFrame.tsx')
assert '<Link className="pi-pre-world__brand" to="/">MCR Concert World</Link>' in frame
assert not re.search(r'fetch\(|localStorage|sessionStorage|usePolicy|createPolicy', frame)
css = current('frontend/src/app/pre-world-policy/parade-ink/pre-world.css')
assert not re.search(r'--(?:pi|mcr|concert)-[\w-]+\s*:', css), 'No forked shared tokens'
assert not re.search(r'border-(?:left|inline-start)\s*:|::(?:before|after)', css), 'No decorative rails or left bars'

exact = {world, notice, 'docs/mcr/frontend/parade-ink-pre-world-policy.md'}
prefixes = ('frontend/src/app/pre-world-policy/parade-ink/', 'frontend/src/app/pre-world-policy/__tests__/parade-ink/',
            'frontend/e2e/visual/parade-ink-pre-world-policy/', 'docs/mcr/evidence/parade-ink-pre-world-policy/')
changed = sp.check_output(['git', 'diff', '--name-only', base, '--'], cwd=root, text=True).splitlines()
for path in changed:
    assert path in exact or path.startswith(prefixes), ('Outside exact Root grant', path)
print(json.dumps({'result': 'PASS', 'groups': 6, 'base': base,
    'preserved': ['bootstrap logic', 'ready/downstream World', 'status/reason/copy/actions', 'policy predicates/minimum/copy/actions', 'pure view/single tokens/no left bars', 'exact owned paths']}))
