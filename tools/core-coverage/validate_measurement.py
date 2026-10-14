"""Isolated Rust/LLVM experiments proving measurement, not Git behavior."""

from pathlib import Path
import json

from evidence import EvidenceError, analyze, parse_raw_lcov


SOURCE = '''#![allow(dead_code)]
pub fn unit_only(value: i32) -> i32 {
    #[cfg(any(test, feature = "test-helpers"))]
    observe();
    value + 1
}
pub fn integration_only() -> i32 {
    42
}
pub fn uncovered() -> i32 {
    99
}
pub fn unused_generic<T>(value: T) -> T {
    value
}
pub const fn unused_const() -> i32 {
    123
}
pub trait DefaultProbe {
    fn unused_default(&self) -> i32 {
        456
    }
}
impl DefaultProbe for () {}
pub struct Invoker {
    value: i32,
    #[cfg(any(test, feature = "test-helpers"))]
    observations: std::sync::Mutex<Vec<i32>>,
}
impl Invoker {
    pub fn new(value: i32) -> Self {
        Self {
            value,
            #[cfg(any(test, feature = "test-helpers"))]
            observations: std::sync::Mutex::new(Vec::new()),
        }
    }
    pub fn output(&self) -> i32 {
        #[cfg(any(test, feature = "test-helpers"))]
        self.observations
            .lock()
            .expect("observation mutex poisoned")
            .push(self.value);
        Some(self.value).map(|value| value + 1).unwrap()
    }
}
#[cfg(any(test, feature = "test-helpers"))]
fn observe() {
    assert!(true);
}
#[cfg(test)]
mod tests {
    #[test]
    fn unit_calls_production() {
        assert_eq!(super::unit_only(4), 5);
        assert_eq!(super::Invoker::new(4).output(), 5);
    }
}
'''


def validate(commands, rustc: Path, llvm: Path, classifier: Path) -> None:
    root = commands.run_dir / "measurement-fixture"
    source = root / "crates/core/src/lib.rs"
    source.parent.mkdir(parents=True)
    source.write_text(SOURCE, encoding="utf-8")
    root_env = commands.env
    base = [rustc, "--edition=2024", "--crate-name=measurement_fixture", "-C", "instrument-coverage",
            "-C", "opt-level=0", source]
    production = root / "libproduction.rlib"
    helpers = root / "libhelpers.rlib"
    unit = root / "unit-tests"
    integration = root / "integration"
    commands.run("fixture-production", [*base, "--crate-type=rlib", "-o", production], "fixture-production.log")
    commands.run("fixture-helpers", [*base, "--crate-type=rlib", "--cfg", 'feature="test-helpers"', "-o", helpers], "fixture-helpers.log")
    commands.run("fixture-build-unit", [*base, "--test", "-o", unit], "fixture-build-unit.log")
    driver = root / "integration.rs"
    driver.write_text("fn main() { assert_eq!(measurement_fixture::integration_only(), 42); }\n", encoding="utf-8")
    commands.run("fixture-build-integration", [rustc, "--edition=2024", "-C", "instrument-coverage", driver,
                 "--extern", f"measurement_fixture={helpers}", "-o", integration], "fixture-build-integration.log")
    env = dict(root_env, LLVM_PROFILE_FILE=str(root / "profiles-%p-%m.profraw"))
    commands.run("fixture-unit", [unit, "--test-threads=1"], "fixture-unit.log", env=env)
    commands.run("fixture-integration", [integration], "fixture-integration.log", env=env)
    profiles = sorted(root.glob("profiles-*.profraw"))
    if len(profiles) != 2:
        raise EvidenceError("tooling fixture did not produce both fresh execution profiles")
    commands.run("fixture-merge", [llvm / "llvm-profdata", "merge", "--failure-mode=any", "-sparse", *profiles,
                 "-o", root / "merged.profdata"], "fixture-merge.log")
    objects = root / "objects"
    objects.mkdir()
    commands.run("fixture-extract", [llvm / "llvm-ar", "x", production], "fixture-extract.log", cwd=objects)
    object_paths = sorted(objects.glob("*.o"))
    if not object_paths:
        raise EvidenceError("tooling fixture ordinary production map is missing")
    inputs = [object_paths[0]]
    for obj in object_paths[1:]:
        inputs.extend(["-object", obj])
    for fmt, extension in (("text", "json"), ("lcov", "lcov")):
        result = commands.run("fixture-production-export", [llvm / "llvm-cov", "export", "--empty-profile",
                              f"-format={fmt}", *inputs], f"fixture-production.{extension}")
        (root / f"production.{extension}").write_text(result, encoding="utf-8")
        result = commands.run("fixture-execution-export", [llvm / "llvm-cov", "export", f"-format={fmt}",
                              f"-instr-profile={root / 'merged.profdata'}", unit, "-object", integration],
                              f"fixture-execution.{extension}")
        (root / f"execution.{extension}").write_text(result, encoding="utf-8")
    result = commands.run("fixture-classify", [classifier, root, "crates/core/src"], "fixture-classification.json")
    (root / "classification.json").write_text(result, encoding="utf-8")
    report = analyze(root / "production.lcov", root / "execution.lcov", root / "production.json",
                     root / "execution.json", root / "classification.json", root)
    file = report["files"][0]
    mapped = set(file["production_lines"])
    covered = set(file["covered_lines"])
    source_lines = SOURCE.splitlines()

    def line(text: str) -> int:
        return source_lines.index(text) + 1

    expected_covered = {line("    value + 1"), line("    42"),
                        line("        Some(self.value).map(|value| value + 1).unwrap()")}
    if not expected_covered <= covered:
        raise EvidenceError("unit/integration production execution or closures did not contribute")
    expected_uncovered = {line("    99"), line("    value"), line("    123"), line("        456")}
    if not expected_uncovered <= mapped or expected_uncovered & covered:
        raise EvidenceError("unused production functions/generics/const/default methods disappeared or gained hits")
    initializer = line("            observations: std::sync::Mutex::new(Vec::new()),")
    raw = parse_raw_lcov(root / "production.lcov", root).lines["crates/core/src/lib.rs"]
    if initializer not in raw or initializer in mapped:
        raise EvidenceError("compiler initializer spillover was not independently classified")
    observer = line("fn observe() {")
    unit_test = line("    fn unit_calls_production() {")
    if {observer, unit_test} & mapped or report["uncovered_total"] <= 0:
        raise EvidenceError("test-only code entered denominator or uncovered fixture failed to remain visible")
    validate_mixed_line(commands, rustc, llvm, classifier)
    write = commands.run_dir / "measurement-validation.json"
    write.write_text(json.dumps({"schema": 1, "status": "passed", "fixture_report": report,
                     "guarantees": ["unit contribution", "integration contribution", "unused production mapping",
                                    "test-only exclusion", "initializer spill correction", "visible uncovered lines",
                                    "mixed-line execution rejected"]},
                     indent=2) + "\n", encoding="utf-8")


def validate_mixed_line(commands, rustc: Path, llvm: Path, classifier: Path) -> None:
    root = commands.run_dir / "mixed-line-fixture"
    source = root / "crates/core/src/lib.rs"
    source.parent.mkdir(parents=True)
    source.write_text('pub fn uncovered() -> i32 { 17 } #[cfg(test)] #[test] fn checks_other() { assert_eq!(2, 2); }\n', encoding="utf-8")
    production = root / "libmixed.rlib"
    unit = root / "unit-tests"
    base = [rustc, "--edition=2024", "--crate-name=mixed_fixture", "-C", "instrument-coverage", source]
    commands.run("mixed-production", [*base, "--crate-type=rlib", "-o", production], "mixed-production.log")
    commands.run("mixed-build-unit", [*base, "--test", "-o", unit], "mixed-build-unit.log")
    env = dict(commands.env, LLVM_PROFILE_FILE=str(root / "profile-%p-%m.profraw"))
    commands.run("mixed-unit", [unit, "--test-threads=1"], "mixed-unit.log", env=env)
    commands.run("mixed-merge", [llvm / "llvm-profdata", "merge", "--failure-mode=any", "-sparse",
                 *sorted(root.glob("profile-*.profraw")), "-o", root / "merged.profdata"], "mixed-merge.log")
    objects = root / "objects"
    objects.mkdir()
    commands.run("mixed-extract", [llvm / "llvm-ar", "x", production], "mixed-extract.log", cwd=objects)
    paths = sorted(objects.glob("*.o"))
    inputs = [paths[0]]
    for obj in paths[1:]:
        inputs.extend(["-object", obj])
    for fmt, extension in (("text", "json"), ("lcov", "lcov")):
        for name, profile, selected in (("production", ["--empty-profile"], inputs),
                                        ("execution", [f"-instr-profile={root / 'merged.profdata'}"], [unit])):
            result = commands.run(f"mixed-{name}-export", [llvm / "llvm-cov", "export", f"-format={fmt}", *profile, *selected],
                                  f"mixed-{name}.{extension}")
            (root / f"{name}.{extension}").write_text(result, encoding="utf-8")
    result = commands.run("mixed-classify", [classifier, root, "crates/core/src"], "mixed-classification.json")
    (root / "classification.json").write_text(result, encoding="utf-8")
    raw = parse_raw_lcov(root / "execution.lcov", root).lines["crates/core/src/lib.rs"]
    if raw != {1: 1}:
        raise EvidenceError("mixed-line fixture did not demonstrate borrowed test-only execution")
    try:
        analyze(root / "production.lcov", root / "execution.lcov", root / "production.json",
                root / "execution.json", root / "classification.json", root)
    except EvidenceError as error:
        if "mixed production/test" not in str(error):
            raise
    else:
        raise EvidenceError("an executed test-only function falsely covered same-line production")
