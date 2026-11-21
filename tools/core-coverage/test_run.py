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
    def test_collector_binds_integration_and_classifier_dependencies_in_their_own_workspaces(self):
        with tempfile.TemporaryDirectory() as directory:
            checkout = Path(directory) / "checkout"
            run_dir = Path(directory) / "coverage"
            groups = {
                "production": {"outpost_core-aa": ["crates/core/src/lib.rs"]},
                "execution": {"outpost_core-bb": ["crates/core/src/lib.rs"],
                              "operation-cc": ["crates/core/tests/operation.rs", "fixtures/expected"]},
                "classifier": {"outpost_core_coverage_classifier-dd": ["src/main.rs", "../../../formats/classification"],
                               "outpost_core_coverage_classifier-ee": ["src/lib.rs"]},
            }
            for mode, binaries in groups.items():
                deps = run_dir / f"{mode}-build/debug/deps"
                deps.mkdir(parents=True)
                for binary, inputs in binaries.items():
                    (deps / f"{binary}.d").write_text(
                        "object: " + " ".join(inputs) + "\n\n" + "".join(name + ":\n" for name in inputs),
                        encoding="utf-8",
                    )
            core = checkout / "crates/core"
            tools = checkout / "tools/core-coverage/classifier"
            workspace = runner.CargoPackages({core / "src/lib.rs": core, core / "tests/operation.rs": core}, set(), {})
            classifier = runner.CargoPackages({tools / "src/main.rs": tools, tools / "src/lib.rs": tools}, set(),
                                              {tools / "src/lib.rs": "lib", tools / "src/main.rs": "bin"})
            records = runner.compiler_input_evidence(run_dir, checkout, workspace, classifier)
            targets = ["lib:outpost_core", "test:operation"]
            self.assertEqual(runner.compiler_input_names(records, checkout, run_dir, targets, workspace, classifier), {
                "crates/core/src/lib.rs", "crates/core/tests/operation.rs", "fixtures/expected",
                "tools/core-coverage/classifier/src/main.rs", "tools/core-coverage/classifier/src/lib.rs", "formats/classification",
            })
            records["builds"]["classifier"].pop("debug/deps/outpost_core_coverage_classifier-dd.d")
            with self.assertRaisesRegex(EvidenceError, "library and executable"):
                runner.compiler_input_names(records, checkout, run_dir, targets, workspace, classifier)

    def test_external_compile_inputs_need_locked_provenance_and_project_env_must_be_declared(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "checkout"
            root.mkdir()
            external = Path(directory) / "package"
            crate = external / "lib.rs"
            source = "registry+https://example.com/index"
            lock = root / "Cargo.lock"
            lock.write_text(f'[[package]]\nname="dependency"\nversion="1.0.0"\nsource="{source}"\nchecksum="{"a" * 64}"\n', encoding="utf-8")
            package = {"id": "dep", "name": "dependency", "version": "1.0.0", "manifest_path": str(external / "Cargo.toml"),
                       "source": None, "targets": [{"src_path": str(crate), "kind": ["lib"]}]}
            metadata = {"packages": [package], "resolve": {"nodes": [{"id": "dep"}]}}
            with self.assertRaisesRegex(EvidenceError, "external Cargo package"):
                runner.cargo_packages(metadata, lock, root)
            package["source"] = source
            packages = runner.cargo_packages(metadata, lock, root)
            other = external.parent / "mutable"
            text = f"object: {crate} {other}\n{crate}:\n{other}:\n"
            with self.assertRaisesRegex(EvidenceError, "no locked package provenance"):
                runner.dep_info_inputs(text, root, root, root / "target", packages)
            project = root / "src.rs"
            packages.targets[project] = root
            text = "object: src.rs\nsrc.rs:\n# env-dep:PROFILE_SETTING=private-value\n"
            with self.assertRaisesRegex(EvidenceError, "undeclared.*PROFILE_SETTING") as error:
                runner.dep_info_inputs(text, root, root, root / "target", packages)
            self.assertNotIn("private-value", str(error.exception))

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

    def test_personal_git_ignore_rules_do_not_change_source_provenance(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory) / "checkout"
            ignore = Path(directory) / "personal-ignore"
            ignore.write_text("local-input\n", encoding="utf-8")
            git_env = {"PATH": os.defpath, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull}
            subprocess.run(["git", "init", "-q", repo], check=True, env=git_env)
            (repo / ".gitignore").write_text("build-output\n", encoding="utf-8")
            for name in ("local-input", "build-output"):
                (repo / name).write_text(name, encoding="utf-8")
            before = current_source_hashes(repo)
            subprocess.run(["git", "-C", repo, "config", "core.excludesFile", ignore], check=True, env=git_env)
            self.assertEqual(set(before), {".gitignore", "local-input"})
            self.assertEqual(current_source_hashes(repo), before)

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
