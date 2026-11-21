"""Runner contracts: preserve failure and require complete harness execution."""

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from evidence import EvidenceError, current_source_hashes, test_results

SPEC = importlib.util.spec_from_file_location("coverage_runner", Path(__file__).with_name("run.py"))
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class RunnerTests(unittest.TestCase):
    def test_environment_owns_compiler_flags_and_wrappers(self):
        config = {"rust_release": "1.94.0", "limits": {"cargo_jobs": 2, "test_threads": 1}, "profile_properties": {}}
        with tempfile.TemporaryDirectory() as directory, mock.patch.dict(os.environ, {
            "RUSTUP_TOOLCHAIN": "nightly", "RUSTFLAGS": "--cfg unreviewed", "CARGO_ENCODED_RUSTFLAGS": "unreviewed",
            "RUSTC_WRAPPER": "/unreviewed/wrapper", "RUSTC_WORKSPACE_WRAPPER": "/unreviewed/other",
            "GIT_DIR": "/unreviewed/repository",
        }):
            env = runner.environment(Path(directory), config)
        for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
            self.assertEqual(env[name], "", name)
        self.assertNotIn("GIT_DIR", env)
        self.assertEqual(env["RUSTUP_TOOLCHAIN"], "1.94.0")

    def test_provenance_reads_the_measured_checkout_when_called_from_another_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            measured, caller = (Path(directory) / name for name in ("measured", "caller"))
            empty_config = Path(directory) / "empty.gitconfig"
            empty_config.touch()
            git_env = {"PATH": os.defpath, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": str(empty_config)}
            for repo in (measured, caller):
                subprocess.run(["git", "init", "-q", repo], check=True, env=git_env)
                (repo / repo.name).write_text("source", encoding="utf-8")
                subprocess.run(["git", "-C", repo, "add", repo.name], check=True, env=git_env)
            with mock.patch.dict(os.environ, {"GIT_DIR": str(caller / ".git"), "GIT_WORK_TREE": str(caller)}):
                self.assertEqual(set(current_source_hashes(measured)), {"measured"})

    def test_failed_child_command_cannot_become_successful_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            commands = runner.Commands(Path(directory), dict(os.environ))
            with self.assertRaisesRegex(EvidenceError, "failed with exit 7"):
                commands.run("failing-test", [sys.executable, "-c", "raise SystemExit(7)"], "failed.log")
            self.assertEqual(commands.records[0]["exit_code"], 7)
            self.assertFalse((Path(directory) / "manifest.json").exists())

    def test_unfinished_target_rejects_an_otherwise_successful_harness_summary(self):
        log = "Running unittests src/lib.rs (/tmp/outpost_core-abc)\n" \
              "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n"
        with self.assertRaises(EvidenceError):
            test_results(log, ["lib:outpost_core", "test:integration"])

    def test_filtered_and_ignored_execution_cannot_establish_a_complete_run(self):
        for ignored, filtered in ((1, 0), (0, 1)):
            log = "Running unittests src/lib.rs (/tmp/outpost_core-abc)\n" \
                  f"test result: ok. 1 passed; 0 failed; {ignored} ignored; 0 measured; {filtered} filtered out; finished in 0s\n"
            with self.subTest(ignored=ignored, filtered=filtered), self.assertRaises(EvidenceError):
                test_results(log, ["lib:outpost_core"])


if __name__ == "__main__":
    unittest.main()
