#!/usr/bin/env python3
"""Run and verify the pinned core measurement inside a supervised Linux cgroup."""

from __future__ import annotations

import argparse
import fcntl
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import time
import uuid

sys.dont_write_bytecode = True

from evidence import (
    EvidenceError, analyze, archive_object_names, canonical_hash, current_commit, current_source_hashes,
    eligible_lcov, read_json, sha256_file, test_results, verify_evidence, verify_pending_evidence,
)

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/core-coverage"


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def environment(run_dir: Path, config: dict) -> dict[str, str]:
    # Git fixtures provide their own author/configuration through _with interfaces.
    # Compiler and coverage flags are owned by the declared contract.
    prefixes = ("GIT_", "RUST", "LLVM_", "__CARGO_LLVM", "CARGO_PROFILE_",
                "CARGO_BUILD_", "CARGO_TARGET_", "CARGO_ENCODED_", "CARGO_LLVM_")
    env = {key: value for key, value in os.environ.items() if not key.startswith(prefixes)}
    if "RUSTUP_TOOLCHAIN" in os.environ:
        env["RUSTUP_TOOLCHAIN"] = os.environ["RUSTUP_TOOLCHAIN"]
    cargo_bin = Path.home() / ".cargo/bin"
    env["PATH"] = f"{cargo_bin}:/usr/bin:/bin"
    env.update({
        "CARGO_TERM_COLOR": "never", "CARGO_BUILD_JOBS": str(config["limits"]["cargo_jobs"]),
        "CARGO_INCREMENTAL": "0", "RUST_TEST_THREADS": str(config["limits"]["test_threads"]),
        "RUSTFLAGS": "", "CARGO_ENCODED_RUSTFLAGS": "",
        "RUSTC_WRAPPER": "", "RUSTC_WORKSPACE_WRAPPER": "",
        "CARGO_LLVM_COV_DENY_WARNINGS": "1", "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_TERMINAL_PROMPT": "0", "PYTHONDONTWRITEBYTECODE": "1",
    })
    empty_config = run_dir / "empty.gitconfig"
    empty_config.write_text("", encoding="utf-8")
    env["GIT_CONFIG_GLOBAL"] = str(empty_config)
    for key, value in config["profile_properties"].items():
        key = "DEBUG" if key == "debuginfo" else key.upper()
        env[f"CARGO_PROFILE_DEV_{key}"] = str(value).lower() if isinstance(value, bool) else str(value)
    return env


class Commands:
    def __init__(self, run_dir: Path, env: dict[str, str]):
        self.run_dir, self.env = run_dir, env
        self.records: list[dict] = []

    def run(self, stage: str, argv: list[str | Path], log: str, cwd: Path = ROOT,
            env: dict[str, str] | None = None) -> str:
        argv = [str(argument) for argument in argv]
        display = ' '.join(argv) if len(' '.join(argv)) <= 500 else f"{Path(argv[0]).name} {argv[1]} (full command in commands.json)"
        print(f"{stage}: {display}", flush=True)
        started = time.monotonic()
        separate = log.endswith((".json", ".lcov"))
        with (self.run_dir / log).open("w", encoding="utf-8") as output:
            result = subprocess.run(argv, cwd=cwd, env=env or self.env, stdout=output,
                                    stderr=subprocess.PIPE if separate else subprocess.STDOUT)
        if separate and result.stderr:
            diagnostic = result.stderr.decode("utf-8", errors="replace")
            (self.run_dir / f"{log}.stderr.log").write_text(diagnostic, encoding="utf-8")
            if stage.startswith(("production-", "execution-")) and ("warning:" in diagnostic or "error:" in diagnostic):
                raise EvidenceError(f"{stage} emitted LLVM diagnostics: {diagnostic}")
        self.records.append({"stage": stage, "argv": argv, "exit_code": result.returncode,
                             "duration_seconds": time.monotonic() - started, "log": log})
        write_json(self.run_dir / "commands.json", {"schema": 1, "commands": self.records})
        text = (self.run_dir / log).read_text(encoding="utf-8")
        if not text and not separate:
            (self.run_dir / log).write_text("Command produced no output.\n", encoding="utf-8")
        if result.returncode:
            print(text[-12000:], file=sys.stderr, flush=True)
            raise EvidenceError(f"{stage} failed with exit {result.returncode}; see {log}")
        return text


def cgroup_limits(config: dict, ci: bool, seconds: int, unit: str) -> dict:
    member = Path("/proc/self/cgroup").read_text().strip().split("::", 1)[1]
    cgroup = Path("/sys/fs/cgroup") / member.lstrip("/")
    expected = {
        "memory.max": config["limits"]["ci_memory_bytes" if ci else "local_memory_bytes"],
        "memory.swap.max": config["limits"]["memory_swap_bytes"],
        "memory.oom.group": 1, "pids.max": config["limits"]["pids_max"],
    }
    for name, limit in expected.items():
        actual = (cgroup / name).read_text().strip()
        if actual != str(limit):
            raise EvidenceError(f"{name} is {actual}, expected enforced {limit}")
    return {
        "wall_time_seconds": seconds, "cargo_jobs": config["limits"]["cargo_jobs"],
        "test_threads": config["limits"]["test_threads"], "memory_bytes": expected["memory.max"],
        "memory_swap_bytes": expected["memory.swap.max"], "pids_max": expected["pids.max"],
        "enforcement": "cgroup-v2", "memory_oom_group": 1, "kill_mode": "control-group",
        "timeout_stop_seconds": config["limits"]["stop_timeout_seconds"],
        "cgroup": str(cgroup), "unit": unit,
    }


def toolchain(commands: Commands, config: dict) -> tuple[Path, Path, Path, dict]:
    rustc = Path(commands.run("locate-rustc", ["rustup", "which", "rustc"], "rustc-path.log").strip())
    cargo = Path(commands.run("locate-cargo", ["rustup", "which", "cargo"], "cargo-path.log").strip())
    rust_version = commands.run("rust-version", [rustc, "-Vv"], "rust-version.log")
    values = dict(line.split(": ", 1) for line in rust_version.splitlines() if ": " in line)
    if values.get("release") != config["rust_release"] or values.get("commit-hash") != config["rust_commit"]:
        raise EvidenceError(f"coverage requires Rust {config['rust_release']} ({config['rust_commit']})")
    llvm = rustc.parent.parent / "lib/rustlib" / config["target"] / "bin"
    commands.env["RUSTC"] = str(rustc)
    commands.env["CARGO"] = str(cargo)
    commands.env["PATH"] = f"{rustc.parent}:{commands.env['PATH']}"
    versions = {
        "rustc": rust_version,
        "cargo": commands.run("cargo-version", [cargo, "-V"], "cargo-version.log"),
        "cargo_llvm_cov": commands.run("coverage-version", [cargo, "llvm-cov", "--version"], "coverage-version.log"),
        "llvm_cov": commands.run("llvm-cov-version", [llvm / "llvm-cov", "--version"], "llvm-cov-version.log"),
        "llvm_profdata": commands.run("llvm-profdata-version", [llvm / "llvm-profdata", "--version"], "llvm-profdata-version.log"),
    }
    if versions["cargo_llvm_cov"].strip() != f"cargo-llvm-cov {config['cargo_llvm_cov_version']}":
        raise EvidenceError("cargo-llvm-cov version differs from the approved contract")
    if any(f"LLVM version {config['llvm_version']}" not in versions[key] for key in ("llvm_cov", "llvm_profdata")):
        raise EvidenceError("LLVM tools do not match the approved compiler")
    return rustc, cargo, llvm, versions


def expected_targets(metadata: dict, package: str) -> list[str]:
    core = [entry for entry in metadata["packages"] if entry["name"] == package]
    if len(core) != 1:
        raise EvidenceError("Cargo metadata does not identify the core package")
    if sorted(core[0]["features"]) != ["test-helpers"]:
        raise EvidenceError("core feature inventory changed; measurement configuration needs review")
    return sorted(f"{kind}:{target['name']}" for target in core[0]["targets"]
                  for kind in ("lib", "test") if kind in target["kind"] and target["test"])


def compiler_sources(directory: Path, root: Path) -> list[str]:
    # rustc emits one empty make target per source dependency. Read those targets
    # rather than splitting the escaped first make rule on whitespace.
    sources: set[str] = set()
    for dep in directory.rglob("outpost_core-*.d"):
        for line in dep.read_text(encoding="utf-8").splitlines():
            if not line.endswith(":") or ": " in line:
                continue
            name = line[:-1].replace("\\ ", " ").replace("\\#", "#").replace("$$", "$")
            path = Path(name)
            path = path if path.is_absolute() else root / path
            try:
                relative = path.resolve().relative_to(root).as_posix()
            except ValueError:
                continue
            if relative.startswith("crates/core/src/") and relative.endswith(".rs"):
                sources.add(relative)
    inventory = sorted(path.relative_to(root).as_posix() for path in (root / "crates/core/src").rglob("*.rs"))
    if sorted(sources) != inventory:
        raise EvidenceError(f"compiler source inventory mismatch: {sorted(set(inventory) - sources)}")
    return sorted(sources)


def export_production(commands: Commands, llvm: Path, library: Path, suffix: str = "") -> None:
    objects_dir = commands.run_dir / f"production-objects{suffix}"
    objects_dir.mkdir()
    table = commands.run("list-production-archive", [llvm / "llvm-ar", "t", library],
                         f"production-archive{suffix}.log")
    expected_objects = archive_object_names(table)
    commands.run("extract-production-map", [llvm / "llvm-ar", "x", library], f"extract{suffix}.log", cwd=objects_dir)
    objects = sorted(objects_dir.glob("*.o"))
    if [path.name for path in objects] != expected_objects:
        raise EvidenceError("extracted production object inventory differs from its archive")
    arguments: list[str | Path] = [objects[0]]
    for obj in objects[1:]:
        arguments.extend(["-object", obj])
    for fmt, extension in (("text", "json"), ("lcov", "lcov")):
        output = commands.run(f"production-{extension}", [llvm / "llvm-cov", "export", "--empty-profile",
                              f"-format={fmt}", *arguments], f"production{suffix}.{extension}")
        if "warning:" in output or "error:" in output:
            raise EvidenceError("LLVM production export emitted diagnostics")


def build_classifier(commands: Commands, cargo: Path) -> Path:
    target = commands.run_dir / "classifier-build"
    env = dict(commands.env, CARGO_TARGET_DIR=str(target))
    commands.run("build-classifier", [cargo, "build", "--manifest-path", TOOL / "classifier/Cargo.toml",
                 "--locked", "--offline", "--jobs", "2"], "classifier-build.log", env=env)
    return target / "debug/outpost-core-coverage-classifier"


def canonical_worker(commands: Commands, config: dict, run_id: str, started_ns: int, limits: dict,
                     require_zero: bool) -> None:
    rustc, cargo, llvm, versions = toolchain(commands, config)
    del rustc
    source_hashes = current_source_hashes(ROOT)
    commit = current_commit(ROOT)
    tree = {
        "schema": 1, "commit": commit, "file_hashes": source_hashes,
        "diff": commands.run("tested-diff", ["git", "diff", "--binary", "HEAD"], "tree-diff.log"),
        "untrackedfiles": commands.run("untracked-inventory", ["git", "ls-files", "--others", "--exclude-standard"], "untracked.log").splitlines(),
    }
    write_json(commands.run_dir / "tree.json", tree)
    metadata = json.loads(commands.run("cargo-metadata", [cargo, "metadata", "--no-deps", "--locked",
                          "--offline", "--format-version", "1"], "cargo-metadata.json"))
    targets = expected_targets(metadata, config["package"])
    write_json(commands.run_dir / "test-targets.json", {"schema": 1, "targets": targets})
    classifier = build_classifier(commands, cargo)
    commands.run("classify-source", [classifier, ROOT, config["source_root"]], "classification.json")
    production_dir = commands.run_dir / "production-build"
    env = dict(commands.env, CARGO_TARGET_DIR=str(production_dir))
    build = commands.run("build-production", [cargo, "rustc", "--package", config["package"], "--lib",
                         "--target", config["target"], "--profile", config["profile"], "--locked", "--offline",
                         "--jobs", "2", "--message-format", "json", "--", "-C", "instrument-coverage"],
                         "production-build.log", env=env)
    artifacts = [json.loads(line) for line in build.splitlines() if line.startswith("{")]
    artifacts = [entry for entry in artifacts if entry.get("reason") == "compiler-artifact"
                 and entry["target"]["name"] == "outpost_core" and "lib" in entry["target"]["kind"]]
    if len(artifacts) != 1 or artifacts[0]["features"] or artifacts[0]["profile"]["test"]:
        raise EvidenceError("ordinary production artifact has unexpected configuration")
    write_json(commands.run_dir / "production-artifact.json", artifacts[0])
    profile = artifacts[0]["profile"]
    for key, expected in (("opt_level", "0"), ("debuginfo", 2), ("debug_assertions", True), ("overflow_checks", True)):
        if profile.get(key) != expected:
            raise EvidenceError(f"effective production profile differs: {key}")
    libraries = [Path(name) for name in artifacts[0]["filenames"] if name.endswith(".rlib")]
    if len(libraries) != 1:
        raise EvidenceError("ordinary production library artifact is missing or ambiguous")
    export_production(commands, llvm, libraries[0])
    execution_dir = commands.run_dir / "execution-build"
    env = dict(commands.env, CARGO_TARGET_DIR=str(execution_dir), CARGO_LLVM_COV_TARGET_DIR=str(execution_dir))
    arguments = ["--package", config["package"], "--tests", "--target", config["target"],
                 "--profile", config["profile"], "--features", "test-helpers", "--locked", "--offline",
                 "--jobs", "2", "--no-cfg-coverage", "--no-cfg-coverage-nightly"]
    log = commands.run("test-core", [cargo, "llvm-cov", *arguments, "--no-report", "--", "--test-threads=1"],
                       "test.log", env=env)
    tests = test_results(log, targets)
    for flag, extension in (("--json", "json"), ("--lcov", "lcov")):
        commands.run(f"execution-{extension}", [cargo, "llvm-cov", "report", "--package", config["package"],
                     "--target", config["target"], "--profile", config["profile"],
                     "--no-default-ignore-filename-regex", "--failure-mode", "any", flag,
                     "--output-path", commands.run_dir / f"execution.{extension}"], f"execution-{extension}.log", env=env)
    write_json(commands.run_dir / "compiler-sources.json", {"schema": 1,
               "production": compiler_sources(production_dir, ROOT), "execution": compiler_sources(execution_dir, ROOT)})
    report = analyze(commands.run_dir / "production.lcov", commands.run_dir / "execution.lcov",
                     commands.run_dir / "production.json", commands.run_dir / "execution.json",
                     commands.run_dir / "classification.json", ROOT)
    write_json(commands.run_dir / "report.json", report)
    (commands.run_dir / "eligible.lcov").write_text(eligible_lcov(report, ROOT), encoding="utf-8")
    commands.run("doctests", [cargo, "test", "--package", config["package"], "--doc", "--target", config["target"],
                 "--profile", config["profile"], "--locked", "--offline", "--jobs", "2", "--", "--test-threads=1"],
                 "doctest.log", env=dict(commands.env, CARGO_TARGET_DIR=str(execution_dir)))
    if current_source_hashes(ROOT) != source_hashes or current_commit(ROOT) != commit:
        raise EvidenceError("tested source changed during the canonical run")
    write_json(commands.run_dir / "supervisor.json", {
        "schema": 1, "unit": limits["unit"], "result": "pending", "returncode": None,
    })
    binary_evidence = list((commands.run_dir / "production-objects").glob("*.o")) + libraries
    artifact_hashes = {
        path.relative_to(commands.run_dir).as_posix(): sha256_file(path)
        for path in sorted(commands.run_dir.glob("*.json")) + sorted(commands.run_dir.glob("*.log"))
                    + sorted(commands.run_dir.glob("*.lcov")) + sorted(binary_evidence)
        if path.name not in ("pending-manifest.json", "manifest.json") and path.stat().st_size > 0
    }
    manifest = {
        "schema": 1, "status": "pending-supervision", "run_id": run_id, "started_ns": started_ns,
        "finished_ns": time.time_ns(), "artifact_hashes": artifact_hashes,
        "commit": commit, "config": config, "config_sha256": canonical_hash(config),
        "source_hashes": source_hashes, "versions": versions, "limits_actual": limits,
        "host": {"system": platform.system(), "machine": platform.machine(),
                 "kernel": platform.release(), "os_release": platform.freedesktop_os_release()},
        "expected_test_targets": targets, "tests": tests,
    }
    write_json(commands.run_dir / "pending-manifest.json", manifest)
    checked = verify_pending_evidence(commands.run_dir, ROOT, require_zero=require_zero)
    manifest["data_verification"] = {"status": "passed", "counts": {
        key: value for key, value in checked.items() if key.endswith("_total")
    }}
    write_json(commands.run_dir / "pending-manifest.json", manifest)


def worker(args: argparse.Namespace, config: dict) -> None:
    # The service holds this lease, so caller disconnection cannot allow another
    # compiler/LLVM workload to overlap its still-running descendants.
    with (ROOT / "target/core-coverage/workload.lock").open("a") as lease:
        try:
            fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise EvidenceError("another bounded build/test/coverage workload is running") from error
        run_worker(args, config)


def run_worker(args: argparse.Namespace, config: dict) -> None:
    run_dir = Path(args.run_dir).resolve()
    # Inode timestamps can use a coarser clock than Python CLOCK_REALTIME. The
    # fresh supervisor marker supplies a filesystem-clock boundary before launch.
    started_ns = (run_dir / "run-start.json").stat().st_mtime_ns
    limits = cgroup_limits(config, args.ci, args.seconds, args.unit)
    commands = Commands(run_dir, environment(run_dir, config))
    if args.mode in ("baseline", "gate"):
        canonical_worker(commands, config, args.run_id, started_ns, limits, require_zero=args.mode == "gate")
    elif args.mode == "self-test":
        from validate_measurement import validate
        rustc, cargo, llvm, _ = toolchain(commands, config)
        if not (TOOL / "classifier/Cargo.lock").exists():
            commands.run("lock-classifier", [cargo, "generate-lockfile", "--manifest-path", TOOL / "classifier/Cargo.toml",
                         "--offline"], "classifier-lock.log")
        env = dict(commands.env, CARGO_TARGET_DIR=str(run_dir / "classifier-build"))
        commands.run("test-classifier", [cargo, "test", "--manifest-path", TOOL / "classifier/Cargo.toml",
                     "--locked", "--offline", "--jobs", "2", "--", "--test-threads=1"], "classifier-test.log", env=env)
        commands.run("test-evidence", [sys.executable, "-m", "unittest", "discover", "-s", TOOL,
                     "-p", "test_*.py", "-v"], "evidence-test.log")
        classifier = run_dir / "classifier-build/debug/outpost-core-coverage-classifier"
        validate(commands, rustc, llvm, classifier)
        print("Measurement tooling validation passed", flush=True)
    elif args.mode == "check":
        _, cargo, _, _ = toolchain(commands, config)
        arguments = args.command[1:] if args.command[:1] == ["--"] else args.command
        if arguments[:1] != ["cargo"]:
            raise EvidenceError("bounded checks must explicitly invoke cargo")
        commands.run("focused-check", [cargo, *arguments[1:]], "check.log")
    elif args.mode == "verify":
        report = verify_evidence(Path(args.evidence_dir), ROOT, args.require_zero)
        write_json(run_dir / "verified-counts.json", {
            key: value for key, value in report.items() if key.endswith("_total")
        })
        print((run_dir / "verified-counts.json").read_text(), flush=True)


def record_ci_run_directory(run_dir: Path, ci: bool, env: dict[str, str]) -> None:
    if ci and env.get("GITHUB_OUTPUT"):
        with Path(env["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"run_directory={run_dir}\n")


def supervise(args: argparse.Namespace, config: dict) -> Path:
    run_id = str(uuid.uuid4())
    run_dir = ROOT / "target/core-coverage" / run_id
    run_dir.mkdir(parents=True)
    record_ci_run_directory(run_dir, args.ci, os.environ)
    unit = f"outpost-core-coverage-{run_id}.service"
    write_json(run_dir / "run-start.json", {"schema": 1, "run_id": run_id, "unit": unit})
    seconds = config["limits"]["wall_time_seconds"] if args.mode in ("baseline", "gate") else args.seconds
    memory = config["limits"]["ci_memory_bytes" if args.ci else "local_memory_bytes"]
    manager = ["sudo", "-n", "systemd-run", f"--uid={os.getuid()}", f"--gid={os.getgid()}"] if args.ci else ["systemd-run", "--user"]
    stop = ["sudo", "-n", "systemctl"] if args.ci else ["systemctl", "--user"]
    argv = [*manager, f"--unit={unit}", "--wait", "--pipe", "--collect",
            f"--property=MemoryMax={memory}", "--property=MemorySwapMax=0", "--property=OOMPolicy=kill",
            f"--property=TasksMax={config['limits']['pids_max']}", f"--property=RuntimeMaxSec={seconds}s",
            f"--property=TimeoutStopSec={config['limits']['stop_timeout_seconds']}s", "--property=KillMode=control-group",
            f"--property=WorkingDirectory={ROOT}", sys.executable, TOOL / "run.py", "_worker",
            "--mode", args.mode, "--run-dir", run_dir, "--run-id", run_id, "--unit", unit,
            "--seconds", str(seconds)]
    if "RUSTUP_TOOLCHAIN" in os.environ:
        argv.insert(len(manager), f"--setenv=RUSTUP_TOOLCHAIN={os.environ['RUSTUP_TOOLCHAIN']}")
    if args.ci:
        argv.append("--ci")
    if args.mode == "check":
        command = args.command[1:] if args.command[:1] == ["--"] else args.command
        argv.extend(["--", *command])
    elif args.mode == "verify":
        argv.extend(["--evidence-dir", str(Path(args.run_dir).resolve())])
        if args.require_zero:
            argv.append("--require-zero")
    print(f"Run: {run_dir}\nUnit: {unit}; cap={memory} bytes, deadline={seconds}s", flush=True)
    proc = subprocess.Popen([str(argument) for argument in argv])

    def cancel(signum: int, _frame: object) -> None:
        subprocess.run([*stop, "stop", unit], check=False, timeout=config["limits"]["stop_timeout_seconds"] + 15)
        proc.wait(timeout=15)
        raise SystemExit(128 + signum)

    previous = {sig: signal.signal(sig, cancel) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        returncode = proc.wait()
    except BaseException:
        subprocess.run([*stop, "stop", unit], check=False, timeout=config["limits"]["stop_timeout_seconds"] + 15)
        proc.wait(timeout=15)
        raise
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    write_json(run_dir / "supervisor.json", {"schema": 1, "unit": unit,
               "result": "success" if returncode == 0 else "failed", "returncode": returncode})
    if returncode:
        raise EvidenceError(f"bounded workload failed ({returncode}); diagnostics: {run_dir}")
    if args.mode in ("baseline", "gate"):
        # Heavy hashing and validation have completed under the service's lease
        # and resource bounds. Outside it, publish only bounded bookkeeping.
        if (run_dir / "pending-manifest.json").stat().st_size > 2 * 1024 * 1024:
            raise EvidenceError("pending completion metadata exceeds its bookkeeping limit")
        manifest = read_json(run_dir / "pending-manifest.json")
        if manifest.get("status") != "pending-supervision" or manifest.get("data_verification", {}).get("status") != "passed":
            raise EvidenceError("bounded evidence validation did not finish")
        manifest["artifact_hashes"]["supervisor.json"] = sha256_file(run_dir / "supervisor.json")
        manifest.update({"status": "complete", "finished_ns": time.time_ns()})
        temporary = run_dir / "manifest.json.tmp"
        write_json(temporary, manifest)
        temporary.replace(run_dir / "manifest.json")
        report = manifest["data_verification"]["counts"]
        print(f"Production {report['production_total']}; exempted {report['exempted_total']}; "
              f"covered {report['covered_total']}; uncovered {report['uncovered_total']}", flush=True)
    print(f"Completed: {run_dir}", flush=True)
    return run_dir


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    for mode in ("baseline", "gate", "self-test", "check"):
        child = sub.add_parser(mode)
        child.add_argument("--ci", action="store_true")
        if mode in ("self-test", "check"):
            child.add_argument("--seconds", type=int, default=300)
        if mode == "check":
            child.add_argument("command", nargs=argparse.REMAINDER)
    verify = sub.add_parser("verify")
    verify.add_argument("run_dir")
    verify.add_argument("--require-zero", action="store_true")
    verify.add_argument("--ci", action="store_true")
    verify.add_argument("--seconds", type=int, default=300)
    internal = sub.add_parser("_worker", help=argparse.SUPPRESS)
    internal.add_argument("--mode", dest="worker_mode", required=True)
    internal.add_argument("--run-dir", required=True)
    internal.add_argument("--run-id", required=True)
    internal.add_argument("--unit", required=True)
    internal.add_argument("--seconds", type=int, required=True)
    internal.add_argument("--ci", action="store_true")
    internal.add_argument("--evidence-dir")
    internal.add_argument("--require-zero", action="store_true")
    internal.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    config = read_json(TOOL / "contract.json")
    if args.mode == "_worker":
        args.mode = args.worker_mode
        worker(args, config)
    else:
        if args.mode in ("self-test", "check", "verify") and not 1 <= args.seconds <= config["limits"]["wall_time_seconds"]:
            raise EvidenceError("check deadline must be within the approved wall-time limit")
        supervise(args, config)


if __name__ == "__main__":
    try:
        main()
    except (EvidenceError, OSError, ValueError, KeyError) as error:
        print(f"core coverage: {error}", file=sys.stderr)
        raise SystemExit(1)
