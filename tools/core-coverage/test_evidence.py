"""Controlled measurement fixtures; these never manufacture core Git behavior."""

import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest import mock

import evidence


SOURCE = "crates/core/src/lib.rs"
COMMIT = "a" * 40
RUN_ID = "dc08dcb2-7c2a-40b2-aa3a-2db7a661dbed"


class EvidenceFixture(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / "repo"
        self.run = self.base / "run"
        (self.root / "crates/core/src").mkdir(parents=True)
        (self.root / "tools/core-coverage").mkdir(parents=True)
        self.run.mkdir()
        self.write_json("run-start.json", {"schema": 1, "run_id": RUN_ID})
        (self.root / SOURCE).write_text(
            "fn production() {\n    operation();\n}\n"
            "#[cfg(test)]\nfn test_only() {\n    production();\n}\n",
            encoding="utf-8",
        )
        self.classification = {
            "schema": 1,
            "files": [{
                "path": SOURCE,
                "production_token_lines": [1, 2, 3],
                "nonproduction_token_lines": [4, 5, 6, 7],
                "nonproduction_lines": [4, 5, 6, 7],
                "nonproduction": [{
                    "start_line": 4, "start_column": 0,
                    "end_line": 7, "end_column": 1, "reason": "cfg(test)",
                }],
            }],
        }
        self.write_json("classification.json", self.classification)
        self.maps({1: 0, 2: 0, 3: 0, 5: 0}, {1: 1, 2: 1, 3: 1, 5: 1, 6: 1, 7: 1})

    def write_json(self, name, value):
        (self.run / name).write_text(json.dumps(value, sort_keys=True), encoding="utf-8")

    def maps(self, production, execution, extra_production=None, extra_execution=None):
        self.write_map("production", {SOURCE: production, **(extra_production or {})})
        self.write_map("execution", {SOURCE: execution, **(extra_execution or {})})

    def write_map(self, name, files):
        text = []
        json_files = []
        total, covered = 0, 0
        for source, lines in sorted(files.items()):
            source_path = str(self.root / source)
            path = Path(source_path)
            if not path.exists():
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("// controlled measurement source\n" * max(lines, default=1), encoding="utf-8")
            hits = sum(count > 0 for count in lines.values())
            text.append(f"SF:{source_path}")
            text.extend(f"DA:{line},{count}" for line, count in sorted(lines.items()))
            text.extend([f"LF:{len(lines)}", f"LH:{hits}", "end_of_record"])
            segments = []
            for line, count in sorted(lines.items()):
                segments.extend([[line, 1, count, True, True, False], [line, 2, 0, False, False, False]])
            json_files.append({"filename": source_path, "segments": segments, "summary": {"lines": {"count": len(lines), "covered": hits}}})
            total += len(lines)
            covered += hits
        (self.run / f"{name}.lcov").write_text("\n".join(text) + "\n", encoding="utf-8")
        self.write_json(f"{name}.json", {
            "type": "llvm.coverage.json.export", "version": "3.0.1",
            "data": [{"files": json_files, "totals": {"lines": {"count": total, "covered": covered}}}],
        })

    def analyze(self):
        return evidence.analyze(
            self.run / "production.lcov", self.run / "execution.lcov",
            self.run / "production.json", self.run / "execution.json",
            self.run / "classification.json", self.root,
        )

    def manifest(self):
        started_ns = (self.run / "run-start.json").stat().st_mtime_ns
        contract = {
            "schema": 1, "package": "outpost-core", "source_root": "crates/core/src",
            "target": "x86_64-unknown-linux-gnu", "profile": "dev",
            "rust_release": "1.94.0", "rust_commit": "4a4ef493e3a1488c6e321570238084b38948f6db",
            "llvm_version": "21.1.8", "cargo_llvm_cov_version": "0.9.0",
            "production_features": [], "execution_features": ["test-helpers"],
            "metric": "production_executable_lines", "exemptions": [],
            "profile_properties": {"opt_level": "0", "debuginfo": 2, "debug_assertions": True, "overflow_checks": True},
            "limits": {"cargo_jobs": 2, "test_threads": 1},
        }
        (self.root / "tools/core-coverage/contract.json").write_text(json.dumps(contract), encoding="utf-8")
        source_hashes = {path.relative_to(self.root).as_posix(): evidence.sha256_file(path) for path in self.root.rglob("*") if path.is_file()}
        report = self.analyze()
        self.write_json("report.json", report)
        (self.run / "eligible.lcov").write_text(evidence.eligible_lcov(report, self.root), encoding="utf-8")
        self.write_json("tree.json", {"schema": 1, "commit": COMMIT, "diff": "", "untrackedfiles": [], "file_hashes": source_hashes})
        self.write_json("compiler-sources.json", {"schema": 1, "production": [SOURCE], "execution": [SOURCE]})
        targets = ["lib:outpost_core", "test:operation"]
        self.write_json("test-targets.json", {"schema": 1, "targets": targets})
        self.write_json("cargo-metadata.json", {"packages": [{"name": "outpost-core", "targets": [
            {"name": "outpost_core", "kind": ["lib"], "test": True},
            {"name": "operation", "kind": ["test"], "test": True},
        ]}]})
        library = self.run / "production-build/x86_64-unknown-linux-gnu/debug/liboutpost_core.rlib"
        library.parent.mkdir(parents=True)
        library.write_bytes(b"controlled archive fixture")
        objects = self.run / "production-objects"
        objects.mkdir()
        object_path = objects / "production.o"
        object_path.write_bytes(b"controlled object fixture")
        (self.run / "production-archive.log").write_text("lib.rmeta\nproduction.o\n", encoding="utf-8")
        self.write_json("production-artifact.json", {
            "reason": "compiler-artifact", "fresh": False, "features": [],
            "target": {"name": "outpost_core", "kind": ["lib"]},
            "profile": {"test": False, "opt_level": "0", "debuginfo": 2, "debug_assertions": True, "overflow_checks": True},
            "filenames": [str(library)],
        })
        commands = [
            ("cargo-metadata", ["cargo", "metadata", "--no-deps", "--locked", "--offline", "--format-version", "1"]),
            ("build-classifier", ["cargo", "build", "--manifest-path", str(self.root / "tools/core-coverage/classifier/Cargo.toml"), "--locked", "--offline", "--jobs", "2"]),
            ("classify-source", [str(self.run / "classifier-build/debug/outpost-core-coverage-classifier"), str(self.root), "crates/core/src"]),
            ("build-production", ["cargo", "rustc", "--package", "outpost-core", "--lib", "--target", "x86_64-unknown-linux-gnu", "--profile", "dev", "--locked", "--offline", "--jobs", "2", "--message-format", "json", "--", "-C", "instrument-coverage"]),
            ("list-production-archive", ["/controlled/llvm-ar", "t", str(library)]),
            ("extract-production-map", ["/controlled/llvm-ar", "x", str(library)]),
            ("production-json", ["/controlled/llvm-cov", "export", "--empty-profile", "-format=text", str(object_path)]),
            ("production-lcov", ["/controlled/llvm-cov", "export", "--empty-profile", "-format=lcov", str(object_path)]),
            ("test-core", ["cargo", "llvm-cov", "--package", "outpost-core", "--tests", "--target", "x86_64-unknown-linux-gnu", "--profile", "dev", "--features", "test-helpers", "--locked", "--offline", "--jobs", "2", "--no-cfg-coverage", "--no-cfg-coverage-nightly", "--no-report", "--", "--test-threads=1"]),
            ("execution-json", ["cargo", "llvm-cov", "report", "--package", "outpost-core", "--target", "x86_64-unknown-linux-gnu", "--profile", "dev", "--no-default-ignore-filename-regex", "--failure-mode", "any", "--json", "--output-path", str(self.run / "execution.json")]),
            ("execution-lcov", ["cargo", "llvm-cov", "report", "--package", "outpost-core", "--target", "x86_64-unknown-linux-gnu", "--profile", "dev", "--no-default-ignore-filename-regex", "--failure-mode", "any", "--lcov", "--output-path", str(self.run / "execution.lcov")]),
            ("doctests", ["cargo", "test", "--package", "outpost-core", "--doc", "--target", "x86_64-unknown-linux-gnu", "--profile", "dev", "--locked", "--offline", "--jobs", "2", "--", "--test-threads=1"]),
        ]
        self.write_json("commands.json", {"schema": 1, "commands": [
            {"stage": stage, "argv": argv, "exit_code": 0, "duration_seconds": 0.25,
             "log": "production-archive.log" if stage == "list-production-archive" else "test.log"}
            for stage, argv in commands
        ]})
        (self.run / "test.log").write_text(
            "     Running unittests src/lib.rs (target/debug/deps/outpost_core-abc)\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
            "     Running tests/operation.rs (target/debug/deps/operation-def)\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n",
            encoding="utf-8",
        )
        (self.run / "production-build.log").write_text("controlled ordinary production compile\n", encoding="utf-8")
        self.document = {
            "schema": 2, "status": "complete", "run_id": RUN_ID, "commit": COMMIT,
            "started_ns": started_ns, "finished_ns": time.time_ns(),
            "config": contract, "config_sha256": evidence.canonical_hash(contract),
            "source_hashes": source_hashes,
            "host": {"system": "Linux", "machine": "x86_64", "kernel": "fixture-kernel",
                     "os_release": {"ID": "fixture", "NAME": "controlled tooling fixture"}},
            "versions": {
                "rustc": "rustc 1.94.0\nrelease: 1.94.0\ncommit-hash: 4a4ef493e3a1488c6e321570238084b38948f6db\n",
                "cargo": "cargo 1.94.0", "cargo_llvm_cov": "cargo-llvm-cov 0.9.0",
                "llvm_cov": "LLVM version 21.1.8", "llvm_profdata": "LLVM version 21.1.8",
            },
            "artifact_hashes": {name: evidence.sha256_file(self.run / name) for name in evidence.REQUIRED_ARTIFACTS},
            "expected_test_targets": targets,
            "tests": [{"target": target, "status": "passed", "exit_code": 0,
                       "total": 1, "passed": 1, "failed": 0, "ignored": 0, "filtered_out": 0} for target in targets],
        }
        for path in (library, object_path):
            self.document["artifact_hashes"][path.relative_to(self.run).as_posix()] = evidence.sha256_file(path)
        self.write_json("manifest.json", self.document)
        return source_hashes

    def verify(self, require_zero=True, current_hashes=None):
        if current_hashes is None:
            current_hashes = self.document["source_hashes"]
        with mock.patch.object(evidence, "current_commit", return_value=COMMIT), mock.patch.object(evidence, "current_source_hashes", return_value=current_hashes):
            return evidence.verify_evidence(self.run, self.root, require_zero)

    def update_manifest(self):
        self.document["finished_ns"] = time.time_ns()
        self.write_json("manifest.json", self.document)

    def rehash(self, name):
        self.document["artifact_hashes"][name] = evidence.sha256_file(self.run / name)
        self.update_manifest()


class MeasurementTests(EvidenceFixture):
    def test_unit_and_integration_hits_count_without_their_source(self):
        # The ordinary compile has zero hits. Test execution supplies production
        # hits, while the test bodies and integration source never enter scope.
        self.maps({1: 0, 2: 0, 3: 0, 5: 0}, {1: 2, 2: 1, 3: 1, 5: 7, 6: 7, 7: 7},
                  extra_execution={"crates/core/tests/operation.rs": {1: 50}})
        report = self.analyze()
        self.assertEqual((report["raw_production_total"], report["production_total"], report["covered_total"]), (4, 3, 3))
        self.assertEqual(report["exemptions"], [])
        published = evidence.eligible_lcov(report, self.root)
        parsed = self.run / "eligible.lcov"
        parsed.write_text(published, encoding="utf-8")
        self.assertEqual(evidence.parse_lcov(parsed, self.root), {SOURCE: {1: 2, 2: 1, 3: 1}})

    def test_missing_execution_record_remains_uncovered(self):
        self.maps({1: 0, 2: 0, 3: 0}, {1: 1, 3: 1, 5: 500})
        report = self.analyze()
        self.assertEqual(report["production_total"], 3)
        self.assertEqual(report["files"][0]["uncovered_lines"], [2])
        self.assertEqual(report["files"][0]["missing_execution_lines"], [2])

    def test_test_only_hits_cannot_improve_the_eligible_result(self):
        self.maps({1: 0, 2: 0, 3: 0, 5: 0}, {1: 1, 2: 0, 3: 1, 5: 999, 6: 999, 7: 999})
        first = self.analyze()
        self.maps({1: 0, 2: 0, 3: 0, 5: 0}, {1: 1, 2: 0, 3: 1, 5: 0, 6: 0, 7: 0})
        second = self.analyze()
        self.assertEqual((first["covered_total"], first["uncovered_total"]), (2, 1))
        self.assertEqual(first["covered_total"], second["covered_total"])

    def test_production_mapping_disagreement_requires_review(self):
        self.maps({1: 0, 3: 0}, {1: 1, 2: 1, 3: 1})
        with self.assertRaisesRegex(evidence.EvidenceError, "mapping review required"):
            self.analyze()

    def test_mixed_physical_line_cannot_borrow_test_hits(self):
        file = self.classification["files"][0]
        file["nonproduction_token_lines"] = [2, 4, 5, 6, 7]
        file["nonproduction"].append({"start_line": 2, "start_column": 5, "end_line": 2, "end_column": 9, "reason": "cfg(test)"})
        self.write_json("classification.json", self.classification)
        with self.assertRaisesRegex(evidence.EvidenceError, "mixed production/test"):
            self.analyze()

    def test_production_token_cannot_be_excluded(self):
        self.classification["files"][0]["nonproduction_lines"] = [2, 4, 5, 6, 7]
        self.write_json("classification.json", self.classification)
        with self.assertRaisesRegex(evidence.EvidenceError, "production tokens appear on excluded"):
            self.analyze()

    def test_empty_denominator_cannot_pass(self):
        self.maps({5: 0}, {5: 1})
        with self.assertRaisesRegex(evidence.EvidenceError, "denominator is empty"):
            self.analyze()

    def test_new_or_orphan_source_cannot_disappear(self):
        (self.root / "crates/core/src/orphan.rs").write_text("pub fn orphan() {}\n", encoding="utf-8")
        with self.assertRaisesRegex(evidence.EvidenceError, "classification inventory differs"):
            self.analyze()

    def test_lcov_and_json_must_agree_for_every_source(self):
        self.maps({1: 0, 2: 0, 3: 0}, {1: 1, 2: 1, 3: 1},
                  extra_execution={"crates/core/tests/operation.rs": {1: 1}})
        report = evidence.read_json(self.run / "execution.json")
        report["data"][0]["files"].pop()
        self.write_json("execution.json", report)
        with self.assertRaisesRegex(evidence.EvidenceError, "LCOV sources absent"):
            self.analyze()

    def test_line_unit_mismatch_is_rejected(self):
        report = evidence.read_json(self.run / "production.json")
        report["data"][0]["files"][0]["summary"]["lines"]["count"] += 1
        self.write_json("production.json", report)
        with self.assertRaisesRegex(evidence.EvidenceError, "LLVM/LCOV function-line summary differs"):
            self.analyze()

    def test_shared_closure_line_is_one_physical_line_despite_two_summary_hits(self):
        # LLVM's containing function and closure each contribute a line to its
        # summary, but export the shared physical source line as one DA record.
        # These segment columns are copied from the compiler validation fixture's
        # Invoker::output (original lines 38/44/45, relocated to 3/4/5).
        (self.root / SOURCE).write_text(
            "pub struct Invoker { value: i32 }\nimpl Invoker {\n"
            "    pub fn output(&self) -> i32 {\n"
            "        Some(self.value).map(|value| value + 1).unwrap()\n"
            "    }\n}\n", encoding="utf-8",
        )
        self.classification["files"][0] = {
            "path": SOURCE, "production_token_lines": [1, 2, 3, 4, 5, 6],
            "nonproduction_token_lines": [], "nonproduction_lines": [], "nonproduction": [],
        }
        self.write_json("classification.json", self.classification)
        self.maps({3: 0, 4: 0, 5: 0}, {3: 1, 4: 1, 5: 1})
        for stem, hit in (("production", 0), ("execution", 1)):
            path = self.run / f"{stem}.lcov"
            text = path.read_text(encoding="utf-8").replace("LF:3", "LF:4")
            if hit:
                text = text.replace("LH:3", "LH:4")
            path.write_text(text, encoding="utf-8")
            document = evidence.read_json(self.run / f"{stem}.json")
            document["data"][0]["files"][0]["segments"] = [
                [3, 5, hit, True, True, False], [3, 32, 0, False, False, False],
                [4, 9, hit, True, True, False], [4, 25, 0, False, False, False],
                [4, 26, hit, True, True, False], [4, 29, 0, False, False, False],
                [4, 38, hit, True, True, False], [4, 43, 0, False, False, False],
                [4, 49, hit, True, True, False], [4, 55, 0, False, False, False],
                [5, 5, hit, True, True, False], [5, 6, 0, False, False, False],
            ]
            for summary in (document["data"][0]["files"][0]["summary"]["lines"], document["data"][0]["totals"]["lines"]):
                summary["count"] = 4
                summary["covered"] = 4 if hit else 0
            self.write_json(f"{stem}.json", document)
        report = self.analyze()
        self.assertEqual((report["production_total"], report["covered_total"]), (3, 3))
        self.assertEqual(report["llvm_function_line_summaries"]["execution"]["totals"], {"found": 4, "hit": 4})
        self.assertEqual(evidence.parse_raw_lcov(self.run / "execution.lcov", self.root).summaries[SOURCE], evidence.LineSummary(4, 4))
        with self.assertRaisesRegex(evidence.EvidenceError, "LCOV LF mismatch"):
            evidence.parse_lcov(self.run / "execution.lcov", self.root)

    def test_json_segments_must_establish_exact_physical_lines_and_hits(self):
        original = evidence.read_json(self.run / "execution.json")
        for mutation in ("missing_line", "wrong_hit", "missing_segments", "invalid_flag", "unsorted", "out_of_source"):
            with self.subTest(mutation=mutation):
                document = json.loads(json.dumps(original))
                file = document["data"][0]["files"][0]
                segments = file["segments"]
                if mutation == "missing_line":
                    file["segments"] = segments[2:]
                elif mutation == "wrong_hit":
                    segments[0][2] = 0
                elif mutation == "missing_segments":
                    file.pop("segments")
                elif mutation == "invalid_flag":
                    segments[0][3] = 1
                elif mutation == "unsorted":
                    segments[0], segments[1] = segments[1], segments[0]
                else:
                    segments[-1][0] = 1_000_000_000
                self.write_json("execution.json", document)
                with self.assertRaises(evidence.EvidenceError):
                    self.analyze()

    def test_wrapped_gap_and_skipped_region_semantics_match_physical_da(self):
        # Line 2 inherits the count from line 1; a skipped region suppresses line
        # 3. A gap on line 4 still uses the wrapped counter until a new entry.
        self.maps({1: 0, 2: 0, 4: 0}, {1: 7, 2: 7, 4: 0})
        for stem, count in (("production", 0), ("execution", 7)):
            document = evidence.read_json(self.run / f"{stem}.json")
            document["data"][0]["files"][0]["segments"] = [
                [1, 1, count, True, True, False],
                [3, 1, 0, False, True, False],
                [4, 1, 99, True, True, True],
                [4, 2, 0, False, False, False],
            ]
            self.write_json(f"{stem}.json", document)
        raw = evidence.parse_raw_lcov(self.run / "execution.lcov", self.root)
        evidence.verify_llvm_json(self.run / "execution.json", raw, self.root)
        self.assertEqual(raw.lines[SOURCE], {1: 7, 2: 7, 4: 0})

    def test_duplicate_incomplete_and_corrupt_lcov_are_rejected(self):
        valid = (self.run / "production.lcov").read_text(encoding="utf-8")
        for content in (
            "", valid + valid, valid.replace("DA:1,0", "DA:1,0\nDA:1,1"),
            valid.replace("end_of_record\n", ""), valid.replace("LF:4", "LF:3"),
            valid.replace("DA:1,0", "DA:1,-1"), "SF:x\nDA:0,0\nLF:1\nLH:0\nend_of_record\n",
            valid.replace("DA:1,0", "FNH:-1\nDA:1,0"),
            valid.replace("DA:1,0", "BRDA:1,0,0,garbage\nDA:1,0"),
        ):
            with self.subTest(content=content):
                (self.run / "production.lcov").write_text(content, encoding="utf-8")
                with self.assertRaises(evidence.EvidenceError):
                    evidence.parse_lcov(self.run / "production.lcov", self.root)

    def test_duplicate_or_malformed_json_is_rejected(self):
        for content in ('{"schema":1,"schema":2}', '{', '{"x":NaN}', '[]'):
            with self.subTest(content=content):
                (self.run / "bad.json").write_text(content, encoding="utf-8")
                with self.assertRaises(evidence.EvidenceError):
                    evidence.read_json(self.run / "bad.json")

    def test_zero_length_and_out_of_source_ownership_spans_are_rejected(self):
        for changes in (
            {"start_line": 4, "end_line": 4, "start_column": 0, "end_column": 0},
            {"start_column": 1000}, {"end_column": 1000},
        ):
            with self.subTest(changes=changes):
                classification = json.loads(json.dumps(self.classification))
                classification["files"][0]["nonproduction"][0].update(changes)
                self.write_json("classification.json", classification)
                with self.assertRaises(evidence.EvidenceError):
                    self.analyze()


class ManifestTests(EvidenceFixture):
    def test_ignored_core_source_is_still_bound_to_the_tested_tree(self):
        self.manifest()
        # Model Git omitting an ignored source. Rust/classifier inventory still
        # includes it, so a same-line-count edit must invalidate prior evidence.
        with mock.patch.object(evidence, "_git", return_value=b"tools/core-coverage/contract.json\x00"):
            before = evidence.current_source_hashes(self.root)
            path = self.root / SOURCE
            path.write_text(path.read_text(encoding="utf-8").replace("operation", "alteredxx"), encoding="utf-8")
            after = evidence.current_source_hashes(self.root)
        self.assertIn(SOURCE, before)
        self.assertNotEqual(before[SOURCE], after[SOURCE])

    def test_complete_current_evidence_passes(self):
        self.manifest()
        self.assertEqual(self.verify()["uncovered_total"], 0)

    def test_unpublished_measurement_cannot_pass_even_when_artifacts_are_complete(self):
        self.manifest()
        (self.run / "manifest.json").rename(self.run / "manifest.json.tmp")
        with mock.patch.object(evidence, "current_commit", return_value=COMMIT), \
             mock.patch.object(evidence, "current_source_hashes", return_value=self.document["source_hashes"]):
            self.assertEqual(evidence.validate_evidence(self.run, self.root, self.document, True)["uncovered_total"], 0)
            with self.assertRaisesRegex(evidence.EvidenceError, "cannot read JSON evidence.*manifest.json"):
                evidence.verify_evidence(self.run, self.root, True)

    def test_missing_or_incompatible_host_metadata_cannot_pass(self):
        self.manifest()
        original = self.document["host"]
        for host in (None, {}, {**original, "system": "Darwin"},
                     {**original, "machine": "aarch64"}, {**original, "os_release": {}}):
            with self.subTest(host=host):
                self.document["host"] = host
                self.update_manifest()
                with self.assertRaises(evidence.EvidenceError):
                    self.verify()

    def test_one_uncovered_eligible_line_fails_only_the_hard_gate(self):
        self.maps({1: 0, 2: 0, 3: 0}, {1: 1, 2: 0, 3: 1})
        self.manifest()
        self.assertEqual(self.verify(require_zero=False)["uncovered_total"], 1)
        with self.assertRaisesRegex(evidence.EvidenceError, "remain uncovered: 1"):
            self.verify()

    def test_missing_empty_or_altered_artifact_fails(self):
        self.manifest()
        path = self.run / "execution.lcov"
        original = path.read_bytes()
        for change in ("missing", "empty", "altered"):
            with self.subTest(change=change):
                if change == "missing":
                    path.unlink()
                else:
                    path.write_bytes(b"" if change == "empty" else original + b"unexpected\n")
                with self.assertRaises(evidence.EvidenceError):
                    self.verify()
                path.write_bytes(original)

    def test_stale_source_or_commit_fails(self):
        self.manifest()
        stale = dict(self.document["source_hashes"])
        stale[SOURCE] = "b" * 64
        with self.assertRaisesRegex(evidence.EvidenceError, "stale"):
            self.verify(current_hashes=stale)
        self.document["commit"] = "c" * 40
        self.update_manifest()
        with self.assertRaisesRegex(evidence.EvidenceError, "different commit"):
            self.verify()

    def test_preexisting_coverage_artifact_cannot_be_reused(self):
        self.manifest()
        path = self.run / "execution.lcov"
        earlier = self.document["started_ns"] - 1
        os.utime(path, ns=(earlier, earlier))
        with self.assertRaisesRegex(evidence.EvidenceError, "stale or outside this run"):
            self.verify()

    def test_start_marker_binds_the_run_identity_and_timestamp_floor(self):
        self.manifest()
        self.document["started_ns"] += 1
        self.update_manifest()
        with self.assertRaisesRegex(evidence.EvidenceError, "differs from its filesystem marker"):
            self.verify()
        self.document["started_ns"] = (self.run / "run-start.json").stat().st_mtime_ns
        self.write_json("run-start.json", {"schema": 1, "run_id": "f024ad42-59ef-45dc-b8f3-829e989d2d57"})
        self.rehash("run-start.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "does not identify this run"):
            self.verify()

    def test_partial_run_is_rejected(self):
        self.manifest()
        self.document["status"] = "running"
        self.update_manifest()
        with self.assertRaisesRegex(evidence.EvidenceError, "incomplete"):
            self.verify()

    def test_failed_ignored_filtered_and_unfinished_targets_fail(self):
        self.manifest()
        original = json.loads(json.dumps(self.document["tests"]))
        for field in ("failed", "ignored", "filtered_out", "unfinished"):
            with self.subTest(field=field):
                self.document["tests"] = json.loads(json.dumps(original))
                if field == "unfinished":
                    self.document["tests"].pop()
                else:
                    self.document["tests"][0][field] = 1
                    self.document["tests"][0]["passed"] = 0
                self.update_manifest()
                with self.assertRaises(evidence.EvidenceError):
                    self.verify()

    def test_dropped_target_cannot_be_hidden_by_matching_inventories(self):
        self.manifest()
        self.document["expected_test_targets"] = ["lib:outpost_core"]
        self.document["tests"] = self.document["tests"][:1]
        self.write_json("test-targets.json", {"schema": 1, "targets": ["lib:outpost_core"]})
        self.rehash("test-targets.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "omit or add Cargo"):
            self.verify()

    def test_omitted_compiler_source_fails_even_when_counts_look_complete(self):
        self.manifest()
        self.write_json("compiler-sources.json", {"schema": 1, "production": [], "execution": [SOURCE]})
        self.rehash("compiler-sources.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "compiler inventory omits"):
            self.verify()

    def test_changed_report_or_published_selection_fails_after_rehash(self):
        self.manifest()
        report = evidence.read_json(self.run / "report.json")
        report["production_total"] -= 1
        self.write_json("report.json", report)
        self.rehash("report.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "published report differs"):
            self.verify()
        self.write_json("report.json", self.analyze())
        self.rehash("report.json")
        (self.run / "eligible.lcov").write_text("wrong selection\n", encoding="utf-8")
        self.rehash("eligible.lcov")
        with self.assertRaisesRegex(evidence.EvidenceError, "published LCOV differs"):
            self.verify()

    def test_failed_command_is_not_masked_by_successful_test_counters(self):
        self.manifest()
        commands = evidence.read_json(self.run / "commands.json")
        commands["commands"][0]["exit_code"] = 1
        self.write_json("commands.json", commands)
        self.rehash("commands.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "failed command"):
            self.verify()

    def test_missing_or_filtered_measurement_command_fails(self):
        self.manifest()
        commands = evidence.read_json(self.run / "commands.json")
        original = json.loads(json.dumps(commands))
        commands["commands"] = [command for command in commands["commands"] if command["stage"] != "build-production"]
        self.write_json("commands.json", commands)
        self.rehash("commands.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "required measurement command stages"):
            self.verify()
        test = next(command for command in original["commands"] if command["stage"] == "test-core")
        test["argv"].insert(-1, "selected_test_name")
        self.write_json("commands.json", original)
        self.rehash("commands.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "command differs.*test-core"):
            self.verify()

    def test_unexpected_extracted_object_requires_archive_review(self):
        self.manifest()
        path = self.run / "production-objects/second.o"
        path.write_bytes(b"additional controlled object fixture")
        self.document["artifact_hashes"][path.relative_to(self.run).as_posix()] = evidence.sha256_file(path)
        self.update_manifest()
        with self.assertRaisesRegex(evidence.EvidenceError, "extracted production object inventory differs"):
            self.verify()

    def test_omitted_archive_object_cannot_pass_with_regenerated_complete_looking_maps(self):
        # The omitted member represents uncovered production. Re-exporting only
        # the remaining object would otherwise produce a consistent zero-gap map.
        self.maps({1: 0, 2: 0, 3: 0, 5: 0}, {1: 1, 2: 0, 3: 1, 5: 1, 6: 1, 7: 1})
        self.manifest()
        omitted = self.run / "production-objects/uncovered.o"
        omitted.write_bytes(b"controlled object with uncovered production")
        self.document["artifact_hashes"][omitted.relative_to(self.run).as_posix()] = evidence.sha256_file(omitted)
        (self.run / "production-archive.log").write_text("lib.rmeta\nproduction.o\nuncovered.o\n", encoding="utf-8")
        self.rehash("production-archive.log")
        commands = evidence.read_json(self.run / "commands.json")
        for command in commands["commands"]:
            if command["stage"] in ("production-json", "production-lcov"):
                command["argv"].extend(["-object", str(omitted)])
        self.write_json("commands.json", commands)
        self.rehash("commands.json")
        self.assertEqual(self.verify(require_zero=False)["uncovered_total"], 1)

        omitted.unlink()
        self.document["artifact_hashes"].pop(omitted.relative_to(self.run).as_posix())
        for command in commands["commands"]:
            if command["stage"] in ("production-json", "production-lcov"):
                del command["argv"][-2:]
        self.write_json("commands.json", commands)
        self.maps({1: 0, 3: 0, 5: 0}, {1: 1, 3: 1, 5: 1, 6: 1, 7: 1})
        report = self.analyze()
        self.assertEqual(report["uncovered_total"], 0)
        self.write_json("report.json", report)
        (self.run / "eligible.lcov").write_text(evidence.eligible_lcov(report, self.root), encoding="utf-8")
        for name in ("commands.json", "production.lcov", "production.json", "execution.lcov",
                     "execution.json", "report.json", "eligible.lcov"):
            self.rehash(name)
        with self.assertRaisesRegex(evidence.EvidenceError, "extracted production object inventory differs"):
            self.verify()

    def test_extracted_objects_must_all_be_hashed(self):
        self.manifest()
        self.document["artifact_hashes"].pop("production-objects/production.o")
        self.update_manifest()
        with self.assertRaisesRegex(evidence.EvidenceError, "hashed production object inventory differs"):
            self.verify()

    def test_test_instrumentation_cannot_replace_the_ordinary_artifact(self):
        self.manifest()
        artifact = evidence.read_json(self.run / "production-artifact.json")
        artifact["features"] = ["test-helpers"]
        self.write_json("production-artifact.json", artifact)
        self.rehash("production-artifact.json")
        with self.assertRaisesRegex(evidence.EvidenceError, "unexpected features"):
            self.verify()

    def test_manifest_counts_cannot_hide_an_unfinished_harness(self):
        self.manifest()
        text = (self.run / "test.log").read_text(encoding="utf-8")
        (self.run / "test.log").write_text(text.rsplit("test result:", 1)[0], encoding="utf-8")
        self.rehash("test.log")
        with self.assertRaisesRegex(evidence.EvidenceError, "targets did not finish"):
            self.verify()

    def test_missing_required_artifact_inventory_fails(self):
        self.manifest()
        self.document["artifact_hashes"].pop("test.log")
        self.update_manifest()
        with self.assertRaisesRegex(evidence.EvidenceError, "required coverage artifacts"):
            self.verify()


class ArchiveInventoryTests(unittest.TestCase):
    def test_native_objects_are_selected_without_treating_metadata_as_code(self):
        self.assertEqual(evidence.archive_object_names("lib.rmeta\nb.o\na.o\n"), ["a.o", "b.o"])

    def test_duplicate_unexpected_and_incomplete_archive_members_are_rejected(self):
        for table in ("", "lib.rmeta\n", "a.o\na.o\n", "a.o\nunknown.bc\n",
                      "subdir/a.o\n", "../a.o\n", "a.o\n\n"):
            with self.subTest(table=table), self.assertRaises(evidence.EvidenceError):
                evidence.archive_object_names(table)


if __name__ == "__main__":
    unittest.main()
