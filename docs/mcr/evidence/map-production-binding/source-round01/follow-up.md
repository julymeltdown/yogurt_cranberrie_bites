# Configuration-loader follow-up

After FULLRELEASE, Root's native preflight identified that Vite's default bundled config loader may write `.vite-temp` under borrowed `node_modules`, even when `cacheDir` points to private TMP. The canonical native driver was corrected in `4f5ce53e613aa018b0572bca0e4ff021a988ca79` to use `configLoader: 'runner'`. This is a source-only driver change; no new executable job or lease extension was performed. Root's fresh native allocation must syntax-check the updated driver.

Read-only inspection of `/Users/lhs/Desktop/mcr/frontend/node_modules/.vite-temp` found an empty directory with mtime 19:08 KST, consistent with this source round's Vitest config loading. The evidence cannot establish pristine read-only dependency use; transient config-bundle writes are likely. No retained config files were found. The developer did not delete or alter that shared directory after inspection. Historical argv, supervisor, exit codes and raw logs remain unchanged.

Future allocated source invocations must add `--configLoader runner` to Vitest, supported by the retained Vitest CLI. Native `createServer` must likewise use `configLoader: 'runner'`. The existing 82-test/type/lint outcomes stand, with this resource-isolation limitation recorded separately. No production code, source asset, lockfile or reference geometry changed in this follow-up.
