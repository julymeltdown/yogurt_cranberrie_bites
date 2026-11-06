# Task 3 - CI workflow removal

Commit: 501710d29 `ci: remove contract-only workflow pending full gate`
Base: 8fc507fe9

## Staged scope
```
.github/workflows/mcr-contracts.yml | 35 -----------------------------------
1 file changed, 35 deletions(-)
```

`git show --stat HEAD` at this commit lists exactly the one deletion. No replacement workflow was added; `.github/workflows/` is now empty.

## Commit body
```
The contract-only workflow is removed rather than left red. CI restoration, covering
plan/guards, frontend, cargo, generated-state drift and release checks, is deferred to
task 48 of the finish-project plan.
```

## Checks
| Command | Exit |
| --- | --- |
| `git show --stat --oneline HEAD` | 0 - one deleted file |
| `git status --porcelain --untracked-files=no` | 0 - empty output (tracked tree clean) |

## Push
`git fetch origin` immediately before push reported `origin/master = 9185c99223bbf7437fff4346a7683dbb4187dfb0`; `git merge-base --is-ancestor origin/master HEAD` succeeded, so the push was a fast-forward. `git push origin master` exited 0 (`9185c9922..501710d29 master -> master`); post-push `origin/master` read back `501710d293e5309e862997a353714e5753a76da0`.
