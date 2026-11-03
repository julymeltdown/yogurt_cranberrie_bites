"""End-to-end boundary checks using disposable local Git repositories only."""
from pathlib import Path
import errno
import os
import subprocess
import sys
import tempfile
import unittest


GUARD = Path(__file__).resolve().parents[1] / "check-boundaries.py"


class BoundariesTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mcr-boundaries-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = {
            **os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_AUTHOR_NAME": "MCR QA", "GIT_AUTHOR_EMAIL": "qa@example.invalid",
            "GIT_COMMITTER_NAME": "MCR QA", "GIT_COMMITTER_EMAIL": "qa@example.invalid",
        }
        self.git("init", "-q")
        self.write("frontend/src/features/item.ts", "export const item = 1;\n")
        self.write("backend/crates/domain/src/item.rs", "pub const ITEM: u8 = 1;\n")
        self.write("frontend/pnpm-lock.yaml", "lockfileVersion: '9.0'\n")
        self.write("docs/source/fixture.md", "Original source fixture.\n")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").stdout.strip()

    def git(self, *args):
        result = subprocess.run(
            ["git", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", *args],
            cwd=self.root, env=self.env, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def write(self, path, content):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(content, encoding="utf-8")

    def commit(self):
        self.git("add", "--all")
        self.git("commit", "-qm", "fixture")

    def run_guard(self, lane="frontend", *extra, base=None, cwd=None):
        return subprocess.run(
            [sys.executable, str(GUARD), "--base=" + (base or self.base), "--lane", lane, *extra],
            cwd=cwd or self.root, env=self.env, text=True, capture_output=True,
        )

    def assert_guard(self, code, lane="frontend", *extra, contains=None, **kwargs):
        result = self.run_guard(lane, *extra, **kwargs)
        output = result.stdout + result.stderr
        self.assertEqual(result.returncode, code, output)
        if contains:
            self.assertIn(contains, output)
        return output

    def test_clean_diff(self):
        self.assert_guard(0, contains="0 changes")

    def test_allowed_frontend_code(self):
        self.write("frontend/src/features/item.ts", "import { x } from './local'; export { x };\n")
        self.assert_guard(0)

    def test_allowed_backend_code(self):
        self.write("backend/crates/domain/src/item.rs", "use crate::local;\n")
        self.assert_guard(0, "backend")

    def test_opposite_lane_unstaged(self):
        self.write("backend/crates/domain/src/item.rs", "pub const ITEM: u8 = 2;\n")
        self.assert_guard(1, contains="opposite-lane write")

    def test_opposite_lane_staged(self):
        self.write("frontend/src/features/item.ts", "export const item = 2;\n")
        self.git("add", "--", "frontend/src/features/item.ts")
        self.assert_guard(1, "backend", contains="opposite-lane write")

    def test_opposite_lane_committed_since_base(self):
        self.write("backend/crates/domain/src/item.rs", "pub const ITEM: u8 = 3;\n")
        self.commit()
        self.assert_guard(1, contains="opposite-lane write")

    def test_staged_violation_cannot_be_hidden_by_unstaged_reversal(self):
        path = "backend/crates/domain/src/item.rs"
        self.write(path, "pub const ITEM: u8 = 3;\n")
        self.git("add", "--", path)
        self.write(path, "pub const ITEM: u8 = 1;\n")
        self.assert_guard(1, contains="index, M")

    def test_staged_import_is_scanned_even_when_working_tree_is_clean(self):
        path = "frontend/src/features/item.ts"
        self.write(path, 'import "../../../backend/secret";\n')
        self.git("add", "--", path)
        self.write(path, "export const item = 1;\n")
        self.assert_guard(1, contains="index:")

    def test_untracked_opposite_lane(self):
        self.write("backend/crates/domain/src/new.rs", "pub struct New;\n")
        self.assert_guard(1, contains="opposite-lane write")

    def test_shared_lockfile(self):
        self.write("frontend/pnpm-lock.yaml", "lockfileVersion: 'changed'\n")
        self.assert_guard(1, contains="manifest/lock")

    def test_integrator_owned_paths(self):
        paths = (
            "frontend/package.json", "backend/Cargo.toml", "backend/crates/new/Cargo.toml",
            "backend/proto/auth.proto", "backend/docs/api/openapi.json",
            "frontend/src/shared/contracts/http.ts", "frontend/src/generated/api.ts",
            "frontend/src/app/router.tsx", "frontend/src/main.tsx",
            "frontend/src/routes/index.tsx",
            "backend/services/api-gateway/src/main.rs",
            "backend/services/concert-service/src/adapters/in/http/mod.rs",
            "backend/services/api-gateway/src/adapters/in/http/routes/index.rs",
            "backend/infra/local.yaml", "frontend/.github/workflows/test.yml",
            "backend/.gitlab-ci.yml", "backend/docker/init.sql",
            "backend/scripts/check.sh", "frontend/vite.config.ts", "backend/.env.local",
            "frontend/Dockerfile", "backend/docker-compose.yml", "frontend/AGENTS.md",
        )
        for path in paths:
            with self.subTest(path=path):
                self.write(path, "fixture\n")
                self.assert_guard(1, path.split("/")[0], contains="integrator-owned")
                (self.root / path).unlink()

    def test_root_scripts_and_source_docs_denied(self):
        self.write("scripts/mcr/extra.py", "print('extra')\n")
        self.write("docs/source/fixture.md", "changed\n")
        output = self.assert_guard(1)
        self.assertIn("scripts/mcr/extra.py", output)
        self.assertIn("docs/source/fixture.md", output)

    def test_allowed_exact_docs_do_not_scan_prose(self):
        for path in ("docs/mcr/qa/assigned.md", "frontend/docs/assigned.md"):
            self.write(path, 'Example: import x from "../../backend/server";\n')
        self.assert_guard(0, "frontend", "--allow-doc", "docs/mcr/qa/assigned.md",
                          "--allow-doc", "frontend/docs/assigned.md")

    def test_docs_need_assignment(self):
        self.write("frontend/docs/unassigned.md", "Notes\n")
        self.assert_guard(1, contains="documentation requires an exact")

    def test_doc_assignment_is_not_a_prefix(self):
        self.write("docs/mcr/qa/assigned.md", "Notes\n")
        self.write("docs/mcr/qa/assigned.md.extra", "Notes\n")
        self.assert_guard(1, "frontend", "--allow-doc", "docs/mcr/qa/assigned.md",
                          contains="assigned.md.extra")

    def test_invalid_docs_cannot_override_boundaries(self):
        for path in ("backend/docs/notes.md", "docs/source/fixture.md", "frontend/package.json",
                     "frontend/src/generated/notes.md", "frontend/AGENTS.md", "docs/mcr/",
                     "docs/mcr/*.md", "docs/mcr/../secret.md", "/tmp/notes.md"):
            with self.subTest(path=path):
                self.assert_guard(2, "frontend", "--allow-doc", path)

    def test_rename_from_opposite_lane_checks_source(self):
        self.git("mv", "backend/crates/domain/src/item.rs", "frontend/src/features/moved.rs")
        output = self.assert_guard(1, contains="opposite-lane write")
        self.assertIn("R100", output)

    def test_rename_to_opposite_lane_checks_destination(self):
        self.git("mv", "frontend/src/features/item.ts", "backend/crates/domain/src/moved.ts")
        self.assert_guard(1, contains="opposite-lane write")

    def test_normal_rename_allowed(self):
        self.git("mv", "frontend/src/features/item.ts", "frontend/src/features/moved.ts")
        self.assert_guard(0)

    def test_renamed_content_is_scanned(self):
        self.git("mv", "frontend/src/features/item.ts", "frontend/src/features/moved.ts")
        self.write("frontend/src/features/moved.ts", 'import "../../../backend/crates/secret";\n')
        self.assert_guard(1, contains="cross-implementation")

    def test_deletions(self):
        (self.root / "frontend/src/features/item.ts").unlink()
        self.assert_guard(0)
        (self.root / "backend/crates/domain/src/item.rs").unlink()
        self.assert_guard(1, contains="opposite-lane write")

    def test_deleted_lockfile_denied(self):
        (self.root / "frontend/pnpm-lock.yaml").unlink()
        self.assert_guard(1, contains="manifest/lock")

    def test_frontend_static_import_forms(self):
        forms = (
            'import x from "../../../backend/server";',
            'import type { X }\nfrom "../../../backend/server";',
            'export { x } from "@backend/server";',
            'export * from "/backend/server";',
            'import "backend/server";',
            'const x = require("../../../backend/server");',
            'const x = require.resolve("../../../backend/server");',
            'const x = import(`../../../backend/server`);',
            'import x = require("@mcr/backend/server");',
            'import /* comment */ x from /* comment */ "../../../backend/server";',
        )
        for form in forms:
            with self.subTest(form=form):
                self.write("frontend/src/features/item.ts", form + "\n")
                self.assert_guard(1, contains="cross-implementation import/embed")

    def test_backend_static_embed_forms(self):
        forms = (
            'const X: &str = include_str!("../../../../frontend/src/ui.ts");',
            'const X: &[u8] = include_bytes!("../../../../frontend/dist/index.html");',
            'include!("../../../../frontend/src/ui.rs");',
            '#[path = "../../../../frontend/src/ui.rs"] mod ui;',
            '#[folder = "frontend/dist"] struct Assets;',
            'include_dir!("$CARGO_MANIFEST_DIR/../../../frontend/dist");',
            'include_str!(r#"../../../../frontend/dist/index.html"#);',
            'use frontend::render;', 'use ::frontend::render;', 'extern crate frontend;',
        )
        for form in forms:
            with self.subTest(form=form):
                self.write("backend/crates/domain/src/item.rs", form + "\n")
                self.assert_guard(1, "backend", contains="cross-implementation import/embed")

    def test_comments_strings_and_public_contract_imports_allowed(self):
        self.write("frontend/src/features/item.ts", '''
// import x from "../../../backend/server";
/* export * from "@backend/private"; */
const prose = 'import x from "backend/server"';
import type { User } from '../../../contracts/generated/http';
import { route } from './backend-client';
import { x } from './backend/../local';
''')
        self.assert_guard(0)

    def test_backend_comments_and_documentation_strings_allowed(self):
        self.write("backend/crates/domain/src/item.rs", '''
// include_str!("../../../../../frontend/dist/index.html");
/* use frontend::render; */
const DOC: &str = r#"use frontend::render;"#;
use crate::domain;
''')
        self.assert_guard(0, "backend")

    def test_path_with_spaces_tabs_newlines_and_option_prefix(self):
        self.write("frontend/src/features/- odd\tname\n.ts", "export const x = 1;\n")
        self.assert_guard(0)
        self.write("backend/crates/domain/src/odd\tname\n.rs", "pub struct Odd;\n")
        output = self.assert_guard(1)
        self.assertIn("odd\\tname\\n.rs", output)

    def test_backslash_filename_fails_closed(self):
        self.write("frontend/src/features/odd\\name.ts", "export {};\n")
        self.assert_guard(1, contains="unsafe/non-canonical")

    @unittest.skipIf(os.name == "nt", "POSIX byte filenames")
    def test_non_utf8_filename_is_reported_safely(self):
        path = os.fsencode(self.root) + b"/backend/crates/domain/src/odd-\xff.rs"
        try:
            with open(path, "wb") as file:
                file.write(b"pub struct Odd;\n")
        except OSError as error:
            if error.errno == errno.EILSEQ:
                self.skipTest("filesystem rejects non-UTF-8 filenames")
            raise
        self.assert_guard(1, contains="opposite-lane write")

    def test_symlink_is_denied_without_following(self):
        (self.root / "frontend/src/features/link.ts").symlink_to("/nonexistent/mcr-fixture")
        self.assert_guard(1, contains="symlink")

    def test_symlinked_directory_is_denied(self):
        (self.root / "frontend/src/linked").symlink_to(self.root / "backend", target_is_directory=True)
        self.assert_guard(1, contains="symlink")

    def test_staged_symlink_is_denied_when_working_file_is_regular(self):
        path = "frontend/src/features/link.ts"
        (self.root / path).symlink_to("/nonexistent/mcr-fixture")
        self.git("add", "--", path)
        (self.root / path).unlink()
        self.write(path, "export {};\n")
        self.assert_guard(1, contains="symlink, submodule")

    def test_unresolved_merge_fails(self):
        initial_branch = self.git("branch", "--show-current").stdout.strip()
        self.git("checkout", "-qb", "other")
        self.write("frontend/src/features/item.ts", "export const item = 2;\n")
        self.commit()
        self.git("checkout", initial_branch)
        self.write("frontend/src/features/item.ts", "export const item = 3;\n")
        self.commit()
        merge = subprocess.run(["git", "merge", "other"], cwd=self.root, env=self.env,
                               capture_output=True, text=True)
        self.assertEqual(merge.returncode, 1, merge.stdout + merge.stderr)
        self.assert_guard(1, contains="unresolved")

    def test_invalid_utf8_source_fails_closed(self):
        (self.root / "frontend/src/features/item.ts").write_bytes(b"\xff\xfe")
        self.assert_guard(1, contains="cannot safely inspect")

    def test_invalid_base_and_option_injection(self):
        self.assert_guard(2, base="missing-ref", contains="BOUNDARIES ERROR")
        self.assert_guard(2, base="--help", contains="BOUNDARIES ERROR")

    def test_run_from_repository_subdirectory(self):
        self.write("frontend/src/features/item.ts", "export const item = 2;\n")
        self.assert_guard(0, cwd=self.root / "frontend/src")

    def test_run_outside_git_repository(self):
        with tempfile.TemporaryDirectory(prefix="mcr-not-git-") as directory:
            self.assert_guard(2, cwd=directory, contains="BOUNDARIES ERROR")


if __name__ == "__main__":
    unittest.main()
