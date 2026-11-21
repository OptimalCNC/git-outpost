"""Fail-closed checks for the approved production executable-line measurement.

The compiler's ordinary production map owns the denominator. Execution from
every unit and integration target contributes hits, but cannot shrink that map.
Syntax classification only removes explicitly attributed non-production source.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import subprocess
import uuid
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


# Use the checkout's ignore files consistently on developer machines and in CI.
REPOSITORY_IGNORES = "--exclude-per-directory=.gitignore"


class EvidenceError(ValueError):
    """Coverage evidence does not establish the measurement contract."""


@dataclass(frozen=True)
class LineSummary:
    """LLVM's sum of function line summaries, distinct from physical DA lines."""

    found: int
    hit: int


@dataclass(frozen=True)
class LcovReport:
    lines: dict[str, dict[int, int]]
    summaries: dict[str, LineSummary]


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise EvidenceError(message)


def _integer(value: Any, context: str, minimum: int = 0) -> int:
    _require(type(value) is int and value >= minimum, f"invalid {context}: {value!r}")
    return value


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        _require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path | str) -> dict[str, Any]:
    try:
        value = json.loads(
            Path(path).read_text(encoding="utf-8"),
            object_pairs_hook=_unique_object,
            parse_constant=lambda value: (_ for _ in ()).throw(
                EvidenceError(f"non-finite JSON value: {value}")
            ),
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read JSON evidence {path}: {error}") from error
    _require(isinstance(value, dict), f"JSON evidence is not an object: {path}")
    return value


def _source_key(source: str, root: Path) -> str:
    _require(bool(source) and "\x00" not in source, "empty or invalid source filename")
    path = Path(source)
    if not path.is_absolute():
        path = root / path
    path = path.resolve()
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        # Dependency and sysroot source still participates in raw report checks.
        # Only exact classifier paths enter the production denominator.
        return str(path)


def _relative_path(value: Any, context: str) -> str:
    _require(isinstance(value, str) and bool(value), f"invalid {context}")
    path = PurePosixPath(value)
    _require(
        not path.is_absolute()
        and ".." not in path.parts
        and "." not in path.parts
        and str(path) == value
        and "\\" not in value
        and "\x00" not in value,
        f"{context} must be a normalized relative path: {value!r}",
    )
    return value


def parse_raw_lcov(path: Path | str, root: Path | str) -> LcovReport:
    """Read physical DA lines and retain raw LF/LH function-summary counts.

    Paths in the workspace are relative to root; dependency paths outside it
    remain absolute. Duplicate records/lines and incomplete records are errors.
    """
    root = Path(root).resolve()
    try:
        content = Path(path).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise EvidenceError(f"cannot read LCOV evidence {path}: {error}") from error
    _require(bool(content.strip()), f"empty LCOV evidence: {path}")
    records: dict[str, dict[int, int]] = {}
    summaries: dict[str, LineSummary] = {}
    source: str | None = None
    lines: dict[int, int] = {}
    found: int | None = None
    hit: int | None = None
    saw_record_data = False
    ancillary = {"FN", "FNDA", "FNF", "FNH", "BRDA", "BRF", "BRH"}
    for number, record in enumerate(content.splitlines(), 1):
        _require(bool(record), f"blank LCOV record at {path}:{number}")
        if record == "end_of_record":
            _require(source is not None, f"LCOV record has no source at {path}:{number}")
            _require(found is not None and hit is not None, f"LCOV LF/LH missing for {source}")
            _require(hit <= found, f"LCOV LH exceeds LF for {source}")
            _require(source not in records, f"duplicate LCOV source: {source}")
            records[source] = lines
            summaries[source] = LineSummary(found, hit)
            source, lines, found, hit, saw_record_data = None, {}, None, None, False
            continue
        tag, separator, value = record.partition(":")
        _require(bool(separator), f"malformed LCOV record at {path}:{number}")
        if tag == "TN":
            _require(source is None and not saw_record_data, "LCOV TN appears inside a source record")
            continue
        if tag == "SF":
            _require(source is None, f"LCOV source record is unfinished at {path}:{number}")
            source = _source_key(value, root)
            continue
        _require(source is not None, f"LCOV {tag} appears before SF at {path}:{number}")
        saw_record_data = True
        if tag == "DA":
            fields = value.split(",")
            _require(len(fields) in (2, 3), f"malformed LCOV DA for {source}")
            _require(all(field.isascii() and field.isdigit() for field in fields[:2]), f"invalid LCOV DA for {source}")
            line, count = map(int, fields[:2])
            _require(line > 0 and line not in lines, f"duplicate or invalid LCOV line for {source}: {line}")
            if len(fields) == 3:
                _require(bool(fields[2]), f"empty LCOV checksum for {source}:{line}")
            lines[line] = count
        elif tag in {"LF", "LH"}:
            _require(value.isascii() and value.isdigit(), f"invalid LCOV {tag} for {source}")
            if tag == "LF":
                _require(found is None, f"duplicate LCOV LF for {source}")
                found = int(value)
            else:
                _require(hit is None, f"duplicate LCOV LH for {source}")
                hit = int(value)
        else:
            _require(tag in ancillary, f"unsupported LCOV field: {tag}")
            _require(bool(value), f"empty LCOV {tag} for {source}")
            if tag in {"FNF", "FNH", "BRF", "BRH"}:
                _require(value.isascii() and value.isdigit(), f"invalid LCOV {tag} for {source}")
            elif tag in {"FN", "FNDA"}:
                numeric, comma, name = value.partition(",")
                _require(bool(comma) and bool(name) and numeric.isascii() and numeric.isdigit(), f"invalid LCOV {tag} for {source}")
                _require(tag != "FN" or int(numeric) > 0, f"invalid LCOV FN line for {source}")
            else:
                fields = value.split(",")
                _require(len(fields) == 4 and all(field.isascii() and field.isdigit() for field in fields[:3]) and int(fields[0]) > 0, f"invalid LCOV BRDA for {source}")
                _require(fields[3] == "-" or fields[3].isascii() and fields[3].isdigit(), f"invalid LCOV branch count for {source}")
    _require(source is None and not saw_record_data, f"unfinished LCOV evidence: {path}")
    _require(bool(records), f"LCOV evidence contains no source records: {path}")
    return LcovReport(records, summaries)


def parse_lcov(path: Path | str, root: Path | str) -> dict[str, dict[int, int]]:
    """Parse published eligible LCOV, whose LF/LH use physical source lines."""
    report = parse_raw_lcov(path, root)
    for source, lines in report.lines.items():
        summary = report.summaries[source]
        _require(summary.found == len(lines), f"LCOV LF mismatch for {source}")
        _require(summary.hit == sum(count > 0 for count in lines.values()), f"LCOV LH mismatch for {source}")
    return report.lines


def _segment_lines(value: Any, source: str, root: Path, expected_lines: int) -> dict[int, int]:
    """Match LLVM 21.1.8 LineCoverageStats over exported per-file segments.

    The reference preserves gap, skipped-region and wrapped-counter semantics:
    https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/llvm/lib/ProfileData/Coverage/CoverageMapping.cpp#L1441-L1494
    JSON's six-field representation comes from CoverageExporterJson.cpp#L77-L80
    at that same tag. This validates LLVM DA output; it does not invent a scope.
    """
    _require(isinstance(value, list), f"LLVM source segments missing for {source}")
    path = Path(source) if Path(source).is_absolute() else root / source
    try:
        source_lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise EvidenceError(f"cannot establish LLVM source bounds for {source}: {error}") from error
    by_line: dict[int, list[list[Any]]] = {}
    previous = (0, 0)
    for segment in value:
        _require(isinstance(segment, list) and len(segment) == 6, f"invalid LLVM segment shape for {source}")
        line = _integer(segment[0], f"LLVM segment line for {source}", 1)
        column = _integer(segment[1], f"LLVM segment column for {source}", 1)
        _integer(segment[2], f"LLVM segment count for {source}")
        _require(all(type(flag) is bool for flag in segment[3:]), f"invalid LLVM segment flags for {source}")
        _require(previous < (line, column), f"LLVM segments are not strictly ordered for {source}")
        _require(line <= len(source_lines) and column <= len(source_lines[line - 1].encode("utf-8")) + 1, f"LLVM segment extends beyond source in {source}")
        previous = (line, column)
        by_line.setdefault(line, []).append(segment)
    result: dict[int, int] = {}
    wrapped: list[Any] | None = None
    for line in range(1, previous[0] + 1):
        segments = by_line.get(line, [])
        entries = [segment for segment in segments if not segment[5] and segment[3] and segment[4]]
        skipped = bool(segments and not segments[0][3] and segments[0][4])
        mapped = (not skipped and (bool(wrapped and wrapped[3]) or bool(entries))) or any(segment[3] and segment[4] for segment in segments)
        if mapped:
            # A corrupt sparse map cannot make this validator allocate an
            # enormous missing-line inventory before it reports the mismatch.
            _require(len(result) < expected_lines, f"LLVM/LCOV mapped-line inventory differs for {source}")
            count = wrapped[2] if wrapped else 0
            if entries:
                count = max(count, *(segment[2] for segment in entries))
            result[line] = count
        if segments:
            wrapped = segments[-1]
    return result


def verify_llvm_json(
    path: Path | str, lcov: LcovReport, root: Path | str
) -> dict[str, Any]:
    """Check physical DA against segments and LF/LH against JSON summary counts.

    LLVM 21.1.8 CoverageReport.cpp#L418-L436 and
    CoverageSummaryInfo.h#L259-L265 sum function-line summaries. Nested functions
    can share physical lines, so those summaries must not become DA counts.
    """
    root = Path(root).resolve()
    report = read_json(path)
    _require(report.get("type") == "llvm.coverage.json.export", "unexpected LLVM JSON type")
    _require(report.get("version") == "3.0.1", "unsupported LLVM JSON schema version")
    data = report.get("data")
    _require(isinstance(data, list) and len(data) == 1, "LLVM JSON must contain one merged data set")
    dataset = data[0]
    _require(isinstance(dataset, dict), "invalid LLVM JSON data set")
    files = dataset.get("files")
    _require(isinstance(files, list) and bool(files), "LLVM JSON contains no source files")
    seen: set[str] = set()
    total, covered = 0, 0
    for file in files:
        _require(isinstance(file, dict) and isinstance(file.get("filename"), str), "invalid LLVM source file")
        source = _source_key(file["filename"], root)
        _require(source not in seen, f"duplicate LLVM source file: {source}")
        seen.add(source)
        _require(source in lcov.lines, f"LLVM source absent from LCOV: {source}")
        file_summary = file.get("summary")
        _require(isinstance(file_summary, dict), f"LLVM summary missing for {source}")
        summary = file_summary.get("lines")
        _require(isinstance(summary, dict), f"LLVM line summary missing for {source}")
        count = _integer(summary.get("count"), f"LLVM line count for {source}")
        hits = _integer(summary.get("covered"), f"LLVM covered lines for {source}")
        _require(hits <= count, f"LLVM summary covered exceeds total for {source}")
        _require(LineSummary(count, hits) == lcov.summaries[source], f"LLVM/LCOV function-line summary differs for {source}")
        decoded = _segment_lines(file.get("segments"), source, root, len(lcov.lines[source]))
        _require(decoded == lcov.lines[source], f"LLVM/LCOV physical mapped lines or execution counts differ for {source}")
        if "notcovered" in summary:
            _require(_integer(summary["notcovered"], "LLVM uncovered lines") == count - hits, f"LLVM uncovered total differs for {source}")
        total += count
        covered += hits
    _require(seen == set(lcov.lines), f"LCOV sources absent from LLVM JSON: {sorted(set(lcov.lines) - seen)}")
    dataset_totals = dataset.get("totals")
    _require(isinstance(dataset_totals, dict), "LLVM aggregate summary missing")
    totals = dataset_totals.get("lines")
    _require(isinstance(totals, dict), "LLVM aggregate line summary missing")
    _require(_integer(totals.get("count"), "LLVM total lines") == total, "LLVM aggregate line total differs")
    _require(_integer(totals.get("covered"), "LLVM total covered lines") == covered, "LLVM aggregate covered total differs")
    return {
        "unit": "LLVM summed function-line summaries; informational",
        "files": {source: {"found": lcov.summaries[source].found, "hit": lcov.summaries[source].hit} for source in sorted(seen)},
        "totals": {"found": total, "hit": covered},
    }


def _line_set(value: Any, context: str, length: int) -> set[int]:
    _require(isinstance(value, list), f"invalid {context}")
    lines = [_integer(line, context, 1) for line in value]
    _require(lines == sorted(set(lines)), f"{context} must contain sorted unique lines")
    _require(all(line <= length for line in lines), f"{context} extends beyond the source")
    return set(lines)


def _classification(path: Path | str, root: Path) -> dict[str, dict[str, Any]]:
    document = read_json(path)
    _require(document.get("schema") == 1, "unsupported source-classification schema")
    files = document.get("files")
    _require(isinstance(files, list) and bool(files), "source classification contains no files")
    result: dict[str, dict[str, Any]] = {}
    for file in files:
        _require(isinstance(file, dict), "invalid classified file")
        source = _relative_path(file.get("path"), "classified source")
        _require(source.startswith("crates/core/src/") and source.endswith(".rs"), f"classified source outside core production scope: {source}")
        _require(source not in result, f"duplicate classified source: {source}")
        _require(_source_key(source, root) == source, f"classified source escapes through a symlink: {source}")
        try:
            source_lines = (root / source).read_text(encoding="utf-8").splitlines()
            length = len(source_lines)
        except (OSError, UnicodeError) as error:
            raise EvidenceError(f"cannot read classified source {source}: {error}") from error
        production = _line_set(file.get("production_token_lines"), f"production token lines for {source}", length)
        nonproduction = _line_set(file.get("nonproduction_token_lines"), f"non-production token lines for {source}", length)
        wholly = _line_set(file.get("nonproduction_lines"), f"non-production lines for {source}", length)
        _require(not wholly & production, f"production tokens appear on excluded lines in {source}")
        spans = file.get("nonproduction")
        _require(isinstance(spans, list), f"non-production spans missing for {source}")
        span_lines: set[int] = set()
        for span in spans:
            _require(isinstance(span, dict), f"invalid non-production span in {source}")
            start = _integer(span.get("start_line"), "span start line", 1)
            end = _integer(span.get("end_line"), "span end line", 1)
            # proc_macro2 reports one-based lines and zero-based columns;
            # these ownership spans are not LLVM's column-coordinate unit.
            start_column = _integer(span.get("start_column"), "span start column")
            end_column = _integer(span.get("end_column"), "span end column")
            _require((start, start_column) < (end, end_column) and start <= end <= length, f"invalid non-production span bounds in {source}")
            _require(start_column <= len(source_lines[start - 1]) and end_column <= len(source_lines[end - 1]), f"non-production span column extends beyond source in {source}")
            _require(isinstance(span.get("reason"), str) and bool(span["reason"].strip()), f"non-production span has no reason in {source}")
            span_lines.update(range(start, end + 1))
        _require(wholly <= span_lines and nonproduction <= span_lines, f"non-production classification lacks span evidence in {source}")
        result[source] = {
            "production_tokens": production,
            "nonproduction_tokens": nonproduction,
            "wholly_nonproduction": wholly,
            "spans": spans,
            "source_lines": length,
        }
    actual = {path.relative_to(root).as_posix() for path in (root / "crates/core/src").rglob("*.rs")}
    _require(set(result) == actual, f"source classification inventory differs: missing={sorted(actual - set(result))}, unexpected={sorted(set(result) - actual)}")
    return result


def analyze(
    production_lcov: Path | str,
    execution_lcov: Path | str,
    production_json: Path | str,
    execution_json: Path | str,
    classification_json: Path | str,
    root: Path | str,
) -> dict[str, Any]:
    """Produce auditable exact counts, retaining missing execution as uncovered."""
    root = Path(root).resolve()
    production_raw = parse_raw_lcov(production_lcov, root)
    execution_raw = parse_raw_lcov(execution_lcov, root)
    production_summaries = verify_llvm_json(production_json, production_raw, root)
    execution_summaries = verify_llvm_json(execution_json, execution_raw, root)
    production, execution = production_raw.lines, execution_raw.lines
    classification = _classification(classification_json, root)
    for source in set(production) | set(execution):
        if source.startswith("crates/core/src/"):
            _require(source in classification, f"mapped core source is unclassified: {source}")
    files: list[dict[str, Any]] = []
    for source, classified in sorted(classification.items()):
        oracle = production.get(source, {})
        hits = execution.get(source, {})
        mapped = set(oracle) | set(hits)
        _require(all(line <= classified["source_lines"] for line in mapped), f"mapped line beyond source in {source}")
        mixed = mapped & classified["production_tokens"] & classified["nonproduction_tokens"]
        _require(not mixed, f"mixed production/test executable physical lines in {source}: {sorted(mixed)}")
        excluded = classified["wholly_nonproduction"]
        additional = set(hits) - set(oracle) - excluded
        _require(not additional, f"execution-map production lines absent from ordinary production map in {source}: {sorted(additional)}; mapping review required")
        eligible = set(oracle) - excluded
        covered = {line for line in eligible if hits.get(line, 0) > 0}
        uncovered = eligible - covered
        files.append({
            "path": source,
            "raw_production_lines": sorted(oracle),
            "classified_nonproduction_lines": sorted(set(oracle) & excluded),
            "execution_nonproduction_lines": sorted(set(hits) & excluded),
            "nonproduction_spans": classified["spans"],
            "production_lines": sorted(eligible),
            "exemptions": [],
            "eligible_lines": sorted(eligible),
            "covered_lines": sorted(covered),
            "uncovered_lines": sorted(uncovered),
            "missing_execution_lines": sorted(eligible - set(hits)),
            "line_counts": {str(line): hits.get(line, 0) for line in sorted(eligible)},
        })
    total = sum(len(file["production_lines"]) for file in files)
    _require(total > 0, "production executable-line denominator is empty")
    covered_total = sum(len(file["covered_lines"]) for file in files)
    return {
        "schema": 1,
        "measurement": "production executable lines",
        "denominator": "ordinary production LLVM DA records minus syntax-classified non-production lines",
        "raw_production_total": sum(len(file["raw_production_lines"]) for file in files),
        "classified_nonproduction_total": sum(len(file["classified_nonproduction_lines"]) for file in files),
        "production_total": total,
        "exemptions": [],
        "exempted_total": 0,
        "eligible_total": total,
        "covered_total": covered_total,
        "uncovered_total": total - covered_total,
        "llvm_function_line_summaries": {"production": production_summaries, "execution": execution_summaries},
        "files": files,
    }


def eligible_lcov(report: dict[str, Any], root: Path | str) -> str:
    """Publish only the report's exact eligible production executable lines."""
    root = Path(root).resolve()
    _require(report.get("schema") == 1 and report.get("eligible_total", 0) > 0, "invalid eligible-line report")
    records: list[str] = []
    for file in report["files"]:
        if not file["eligible_lines"]:
            continue
        records.extend(["TN:production executable lines", f"SF:{root / file['path']}"])
        records.extend(f"DA:{line},{file['line_counts'][str(line)]}" for line in file["eligible_lines"])
        records.extend([f"LF:{len(file['eligible_lines'])}", f"LH:{len(file['covered_lines'])}", "end_of_record"])
    return "\n".join(records) + "\n"


def sha256_file(path: Path | str) -> str:
    try:
        digest = hashlib.sha256()
        with Path(path).open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(block)
        return digest.hexdigest()
    except OSError as error:
        raise EvidenceError(f"cannot hash evidence {path}: {error}") from error


def canonical_hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")).hexdigest()


def _git(root: Path, *arguments: str) -> bytes:
    try:
        env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        env["PATH"] = "/usr/bin:/bin"
        result = subprocess.run(
            ["git", "-C", str(root), *arguments],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, env=env,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise EvidenceError(f"cannot establish current Git tree: {error}") from error
    return result.stdout


def current_source_hashes(root: Path | str) -> dict[str, str]:
    """Bind Git files and every core source/test input, including ignored files."""
    root = Path(root).resolve()
    names = _git(root, "ls-files", "-z", "--cached", "--others", REPOSITORY_IGNORES)
    hashes: dict[str, str] = {}
    source_names: set[str] = set()
    for raw in names.split(b"\x00"):
        if not raw:
            continue
        try:
            name = _relative_path(raw.decode("utf-8"), "Git source path")
        except UnicodeError as error:
            raise EvidenceError("non-UTF-8 Git source path cannot be recorded") from error
        source_names.add(name)
    # Git ignore rules do not determine the coverage scope. A compiled ignored
    # file must not evade freshness checks when its line count stays unchanged.
    for scope in ("crates/core/src", "crates/core/tests"):
        source_names.update(path.relative_to(root).as_posix() for path in (root / scope).rglob("*") if path.is_file())
    for name in sorted(source_names):
        path = root / name
        if not path.exists():
            continue
        _require(path.is_file(), f"unsupported non-file Git source: {name}")
        _require(_source_key(name, root) == name, f"Git source escapes through a symlink: {name}")
        hashes[name] = sha256_file(path)
    _require(bool(hashes), "current Git source inventory is empty")
    return dict(sorted(hashes.items()))


def current_commit(root: Path | str) -> str:
    value = _git(Path(root).resolve(), "rev-parse", "HEAD").decode("ascii").strip()
    _require(len(value) == 40 and all(char in "0123456789abcdef" for char in value), "invalid current commit identifier")
    return value


def test_results(log: str, targets: list[str]) -> list[dict[str, Any]]:
    """Require an identified complete Rust harness result for every Cargo target."""
    by_binary = {target.split(":", 1)[1]: target for target in targets}
    _require(len(by_binary) == len(targets), "ambiguous Cargo test executable names")
    current: str | None = None
    results: list[dict[str, Any]] = []
    for line in log.splitlines():
        running = re.match(r"\s*Running .+ \((.+)\)$", line)
        if running:
            _require(current is None, f"test target did not finish: {current}")
            binary = Path(running[1]).name.rsplit("-", 1)[0]
            _require(binary in by_binary, f"unexpected test executable: {binary}")
            current = by_binary[binary]
        outcome = re.match(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", line)
        if outcome:
            _require(current is not None, "test result lacks an identified executable")
            passed, failed, ignored, measured, filtered = map(int, outcome.groups()[1:])
            _require(outcome[1] == "ok" and failed == ignored == measured == filtered == 0, f"test execution is unsuccessful or incomplete: {current}")
            results.append({
                "target": current, "status": "passed", "exit_code": 0,
                "total": passed, "passed": passed, "failed": failed,
                "ignored": ignored, "filtered_out": filtered,
            })
            current = None
    _require(current is None and sorted(result["target"] for result in results) == targets, "some expected unit or integration targets did not finish")
    return sorted(results, key=lambda result: result["target"])


REQUIRED_ARTIFACTS = frozenset({
    "production.lcov", "production.json", "execution.lcov", "execution.json",
    "classification.json", "report.json", "eligible.lcov", "tree.json",
    "test.log", "production-build.log", "commands.json", "compiler-sources.json",
    "test-targets.json", "cargo-metadata.json", "production-artifact.json", "run-start.json",
    "production-archive.log",
})


def archive_object_names(table: str) -> list[str]:
    """Parse the pinned native rlib member table without silently skipping code."""
    members = table.splitlines()
    _require(bool(members), "ordinary production archive member table is empty")
    seen: set[str] = set()
    objects: list[str] = []
    for name in members:
        name = _relative_path(name, "production archive member")
        _require(PurePosixPath(name).name == name, "production archive member is not a flat filename")
        _require(name not in seen, f"duplicate production archive member: {name}")
        seen.add(name)
        if name.endswith(".o"):
            objects.append(name)
        else:
            _require(name == "lib.rmeta", f"unexpected production archive member requires mapping review: {name}")
    _require(bool(objects), "ordinary production archive contains no native objects")
    return sorted(objects)


def _verify_commands(
    commands: list[dict[str, Any]], contract: dict[str, Any],
    artifacts: dict[str, str], run_dir: Path, root: Path,
) -> None:
    required = (
        "cargo-metadata", "build-classifier", "classify-source", "build-production",
        "list-production-archive", "extract-production-map", "production-json", "production-lcov", "test-core",
        "execution-json", "execution-lcov", "doctests",
    )
    selected: dict[str, dict[str, Any]] = {}
    positions: dict[str, int] = {}
    for position, command in enumerate(commands):
        stage = command["stage"]
        if stage in required:
            _require(stage not in selected, f"duplicate required command stage: {stage}")
            selected[stage], positions[stage] = command, position
    _require(set(selected) == set(required), f"required measurement command stages are missing: {sorted(set(required) - set(selected))}")
    for before, after in (
        ("cargo-metadata", "test-core"), ("build-classifier", "classify-source"),
        ("classify-source", "build-production"), ("build-production", "list-production-archive"),
        ("list-production-archive", "extract-production-map"),
        ("extract-production-map", "production-json"), ("extract-production-map", "production-lcov"),
        ("production-json", "test-core"), ("production-lcov", "test-core"),
        ("test-core", "execution-json"), ("test-core", "execution-lcov"),
        ("execution-json", "doctests"), ("execution-lcov", "doctests"),
    ):
        _require(positions[before] < positions[after], f"measurement command order is invalid: {before}, {after}")
    cargo = selected["build-production"]["argv"][0]
    package, target, profile = contract["package"], contract["target"], contract["profile"]
    jobs = str(contract["limits"]["cargo_jobs"])
    expected = {
        "cargo-metadata": [cargo, "metadata", "--no-deps", "--locked", "--offline", "--format-version", "1"],
        "build-classifier": [cargo, "build", "--manifest-path", str(root / "tools/core-coverage/classifier/Cargo.toml"), "--locked", "--offline", "--jobs", jobs],
        "classify-source": [str(run_dir / "classifier-build/debug/outpost-core-coverage-classifier"), str(root), contract["source_root"]],
        "build-production": [cargo, "rustc", "--package", package, "--lib", "--target", target, "--profile", profile,
                             "--locked", "--offline", "--jobs", jobs, "--message-format", "json", "--", "-C", "instrument-coverage"],
        "test-core": [cargo, "llvm-cov", "--package", package, "--tests", "--target", target, "--profile", profile,
                      "--features", "test-helpers", "--locked", "--offline", "--jobs", jobs, "--no-cfg-coverage",
                      "--no-cfg-coverage-nightly", "--no-report", "--", "--test-threads=1"],
        "doctests": [cargo, "test", "--package", package, "--doc", "--target", target, "--profile", profile,
                     "--locked", "--offline", "--jobs", jobs, "--", "--test-threads=1"],
    }
    for extension, flag in (("json", "--json"), ("lcov", "--lcov")):
        expected[f"execution-{extension}"] = [
            cargo, "llvm-cov", "report", "--package", package, "--target", target, "--profile", profile,
            "--no-default-ignore-filename-regex", "--failure-mode", "any", flag,
            "--output-path", str(run_dir / f"execution.{extension}"),
        ]
    for stage, argv in expected.items():
        _require(selected[stage]["argv"] == argv, f"command differs from the approved measurement: {stage}")
    compiler_artifact = read_json(run_dir / "production-artifact.json")
    _require(compiler_artifact.get("reason") == "compiler-artifact" and compiler_artifact.get("fresh") is False, "ordinary production artifact was not freshly compiled")
    artifact_target = compiler_artifact.get("target")
    _require(isinstance(artifact_target, dict) and isinstance(artifact_target.get("kind"), list) and artifact_target.get("name") == "outpost_core" and "lib" in artifact_target["kind"], "ordinary production artifact has the wrong target")
    _require(compiler_artifact.get("features") == contract["production_features"] == [], "ordinary production artifact has unexpected features")
    actual_profile = compiler_artifact.get("profile")
    _require(isinstance(actual_profile, dict) and actual_profile.get("test") is False, "ordinary production artifact is test instrumentation")
    for key in ("opt_level", "debuginfo", "debug_assertions", "overflow_checks"):
        expected_property = contract["profile_properties"][key]
        _require(type(actual_profile.get(key)) is type(expected_property) and actual_profile[key] == expected_property, f"ordinary production profile differs: {key}")
    filenames = compiler_artifact.get("filenames")
    _require(isinstance(filenames, list) and all(isinstance(name, str) for name in filenames), "ordinary production filenames are missing")
    libraries = [Path(name).resolve() for name in filenames if name.endswith(".rlib")]
    _require(len(libraries) == 1 and libraries[0].is_relative_to(run_dir / "production-build"), "ordinary production library is missing or outside its build directory")
    library = libraries[0]
    library_key = library.relative_to(run_dir).as_posix()
    _require(library_key in artifacts and library.stat().st_size > 0, "ordinary production library is not retained as hashed evidence")
    objects = sorted((run_dir / "production-objects").glob("*.o"))
    _require(bool(objects), "ordinary production object inventory is empty")
    archive_stage = selected["list-production-archive"]
    _require(archive_stage["log"] == "production-archive.log", "production archive member table was not retained")
    try:
        archive_table = (run_dir / "production-archive.log").read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise EvidenceError(f"cannot read production archive member table: {error}") from error
    expected_objects = archive_object_names(archive_table)
    _require([path.name for path in objects] == expected_objects, "extracted production object inventory differs from its archive")
    expected_hashes = {f"production-objects/{name}" for name in expected_objects}
    hashed_objects = {name for name in artifacts if name.startswith("production-objects/") and name.endswith(".o")}
    _require(hashed_objects == expected_hashes, "hashed production object inventory differs from its archive")
    for path in objects:
        _require(path.is_file() and not path.is_symlink() and path.stat().st_size > 0, f"production object is not an ordinary retained file: {path.name}")
    llvm_cov = selected["production-json"]["argv"][0]
    _require(Path(llvm_cov).name == "llvm-cov", "production exporter is not the declared LLVM tool")
    llvm_ar = str(Path(llvm_cov).with_name("llvm-ar"))
    _require(archive_stage["argv"] == [llvm_ar, "t", str(library)], "production archive member listing differs")
    _require(selected["extract-production-map"]["argv"] == [llvm_ar, "x", str(library)], "production archive extraction differs")
    object_arguments = [str(objects[0])]
    for path in objects[1:]:
        object_arguments.extend(["-object", str(path)])
    for extension, fmt in (("json", "text"), ("lcov", "lcov")):
        argv = [llvm_cov, "export", "--empty-profile", f"-format={fmt}", *object_arguments]
        _require(selected[f"production-{extension}"]["argv"] == argv, f"production export omitted or added objects: {extension}")


def verify_evidence(
    run_dir: Path | str, root: Path | str, require_zero: bool
) -> dict[str, Any]:
    """Verify a published measurement against its artifacts and the current tree."""
    manifest = read_json(Path(run_dir) / "manifest.json")
    return validate_evidence(run_dir, root, manifest, require_zero)


def validate_evidence(
    run_dir: Path | str, root: Path | str, manifest: dict[str, Any], require_zero: bool
) -> dict[str, Any]:
    """Check the actual commands, tests, source selection and counts before publication or reuse."""
    run_dir, root = Path(run_dir).resolve(), Path(root).resolve()
    _require(type(manifest.get("schema")) is int and manifest["schema"] == 2, "unsupported coverage-manifest schema")
    _require(manifest.get("status") == "complete", "coverage run is incomplete")
    try:
        run_id = str(uuid.UUID(manifest.get("run_id", "")))
    except (ValueError, TypeError, AttributeError) as error:
        raise EvidenceError("invalid coverage run identifier") from error
    _require(manifest.get("run_id") == run_id, "coverage run identifier must be a canonical UUID")
    contract = read_json(root / "tools/core-coverage/contract.json")
    _require(manifest.get("config") == contract, "run configuration differs from the approved contract")
    _require(manifest.get("config_sha256") == canonical_hash(contract), "run configuration hash differs")
    _require(manifest.get("commit") == current_commit(root), "coverage evidence belongs to a different commit")
    host = manifest.get("host")
    _require(isinstance(host, dict) and host.get("system") == "Linux" and host.get("machine") == "x86_64", "measured host differs from the approved Linux target")
    _require(isinstance(host.get("kernel"), str) and bool(host["kernel"]), "measured kernel is missing")
    os_release = host.get("os_release")
    _require(isinstance(os_release, dict) and all(isinstance(os_release.get(key), str) and bool(os_release[key]) for key in ("ID", "NAME")), "measured operating system is missing")
    versions = manifest.get("versions")
    _require(isinstance(versions, dict) and all(isinstance(versions.get(key), str) and bool(versions[key]) for key in ("rustc", "cargo", "cargo_llvm_cov", "llvm_cov", "llvm_profdata")), "tool versions are missing")
    rust_version = dict(line.split(": ", 1) for line in versions["rustc"].splitlines() if ": " in line)
    _require(rust_version.get("release") == contract.get("rust_release") and rust_version.get("commit-hash") == contract.get("rust_commit"), "Rust compiler differs from the approved version")
    _require(versions["cargo_llvm_cov"].strip() == f"cargo-llvm-cov {contract.get('cargo_llvm_cov_version')}", "cargo-llvm-cov differs from the approved version")
    for name in ("llvm_cov", "llvm_profdata"):
        llvm_version = re.search(r"LLVM version (\S+)", versions[name])
        expected_llvm = contract.get("llvm_version")
        approved_versions = {expected_llvm, f"{expected_llvm}-rust-{contract.get('rust_release')}-stable"}
        _require(llvm_version is not None and llvm_version.group(1) in approved_versions, f"{name} differs from the approved LLVM version")
    sources = manifest.get("source_hashes")
    _require(isinstance(sources, dict) and bool(sources), "source hashes are missing")
    _require(sources == current_source_hashes(root), "coverage evidence is stale or the source inventory changed")
    artifacts = manifest.get("artifact_hashes")
    _require(isinstance(artifacts, dict), "artifact hashes are missing")
    _require(REQUIRED_ARTIFACTS <= set(artifacts), f"required coverage artifacts are missing: {sorted(REQUIRED_ARTIFACTS - set(artifacts))}")
    started = _integer(manifest.get("started_ns"), "run start timestamp", 1)
    finished = _integer(manifest.get("finished_ns"), "run completion timestamp", 1)
    _require(started <= finished, "run completion precedes its start")
    start_path = run_dir / "run-start.json"
    marker = read_json(start_path)
    _require(type(marker.get("schema")) is int and marker["schema"] == 1 and marker.get("run_id") == run_id, "run start marker does not identify this run")
    _require(started == start_path.stat().st_mtime_ns, "run start timestamp differs from its filesystem marker")
    for name, digest in artifacts.items():
        name = _relative_path(name, "artifact path")
        artifact = run_dir / name
        _require(artifact.resolve().is_relative_to(run_dir), f"artifact escapes its run directory: {name}")
        _require(isinstance(digest, str) and len(digest) == 64, f"invalid artifact hash for {name}")
        _require(artifact.is_file(), f"coverage artifact is missing: {name}")
        _require(name not in REQUIRED_ARTIFACTS or artifact.stat().st_size > 0, f"coverage artifact is empty: {name}")
        _require(started <= artifact.stat().st_mtime_ns <= finished, f"coverage artifact is stale or outside this run: {name}")
        _require(sha256_file(artifact) == digest, f"coverage artifact changed: {name}")
    tree = read_json(run_dir / "tree.json")
    _require(tree.get("schema") == 1 and tree.get("commit") == manifest["commit"] and tree.get("file_hashes") == sources, "tested-tree provenance differs from the manifest")
    _require(isinstance(tree.get("diff"), str) and isinstance(tree.get("untrackedfiles"), list), "tested uncommitted changes were not recorded")
    command_document = read_json(run_dir / "commands.json")
    _require(command_document.get("schema") == 1, "unsupported command-evidence schema")
    commands = command_document.get("commands")
    _require(isinstance(commands, list) and bool(commands), "run commands were not recorded")
    for command in commands:
        _require(isinstance(command, dict), "invalid command evidence")
        _require(isinstance(command.get("stage"), str) and bool(command["stage"]), "command stage is missing")
        argv = command.get("argv")
        _require(isinstance(argv, list) and bool(argv) and all(isinstance(argument, str) and bool(argument) for argument in argv), "command argv is missing or invalid")
        _require(type(command.get("exit_code")) is int and command["exit_code"] == 0, "failed command cannot establish coverage evidence")
        duration = command.get("duration_seconds")
        _require(type(duration) in (int, float) and math.isfinite(duration) and duration >= 0, "invalid command duration")
        log = _relative_path(command.get("log"), "command log")
        _require(log in artifacts, f"command log is not retained as hashed evidence: {log}")
    _verify_commands(commands, contract, artifacts, run_dir, root)
    inventory = sorted(path.relative_to(root).as_posix() for path in (root / "crates/core/src").rglob("*.rs"))
    compiler = read_json(run_dir / "compiler-sources.json")
    _require(compiler.get("schema") == 1, "unsupported compiler-source inventory schema")
    for mode in ("production", "execution"):
        values = compiler.get(mode)
        _require(isinstance(values, list) and all(isinstance(name, str) for name in values) and values == sorted(set(values)), f"invalid {mode} compiler-source inventory")
        for name in values:
            _relative_path(name, f"{mode} compiler source")
        scoped = [name for name in values if name.startswith("crates/core/src/")]
        _require(scoped == inventory, f"{mode} compiler inventory omits or adds unclassified core source")
    target_document = read_json(run_dir / "test-targets.json")
    _require(target_document.get("schema") == 1, "unsupported test-target inventory schema")
    targets = manifest.get("expected_test_targets")
    _require(isinstance(targets, list) and bool(targets) and all(isinstance(target, str) for target in targets) and targets == sorted(set(targets)), "expected test targets are missing or invalid")
    _require(targets == target_document.get("targets"), "test-target inventory differs from expected targets")
    _require(all(isinstance(target, str) and target.startswith(("lib:", "test:")) for target in targets), "invalid expected test target")
    _require("lib:outpost_core" in targets, "unit-test execution was omitted")
    metadata = read_json(run_dir / "cargo-metadata.json")
    packages = metadata.get("packages")
    _require(isinstance(packages, list), "Cargo metadata packages are missing")
    core_packages = [package for package in packages if isinstance(package, dict) and package.get("name") == "outpost-core"]
    _require(len(core_packages) == 1, "Cargo metadata does not identify exactly one outpost-core package")
    metadata_targets: set[str] = set()
    core_targets = core_packages[0].get("targets")
    _require(isinstance(core_targets, list), "Cargo package targets are missing")
    for target in core_targets:
        _require(isinstance(target, dict) and isinstance(target.get("kind"), list) and isinstance(target.get("name"), str) and type(target.get("test")) is bool, "invalid Cargo target metadata")
        if target.get("test") is not True:
            continue
        kind = target["kind"]
        if "lib" in kind:
            metadata_targets.add(f"lib:{target.get('name')}")
        elif "test" in kind:
            metadata_targets.add(f"test:{target.get('name')}")
    _require(sorted(metadata_targets) == targets, "expected test targets omit or add Cargo test targets")
    tests = manifest.get("tests")
    _require(isinstance(tests, list), "test results are missing")
    seen: set[str] = set()
    for test in tests:
        _require(isinstance(test, dict), "invalid test result")
        target = test.get("target")
        _require(isinstance(target, str) and target in targets and target not in seen, "unexpected or duplicate test result")
        seen.add(target)
        _require(test.get("status") == "passed" and type(test.get("exit_code")) is int and test["exit_code"] == 0, f"test target failed or incomplete: {target}")
        total = _integer(test.get("total"), f"total tests for {target}")
        passed = _integer(test.get("passed"), f"passed tests for {target}")
        failed = _integer(test.get("failed"), f"failed tests for {target}")
        ignored = _integer(test.get("ignored"), f"ignored tests for {target}")
        filtered = _integer(test.get("filtered_out"), f"filtered tests for {target}")
        _require(total == passed + failed + ignored + filtered, f"test counters disagree for {target}")
        _require(failed == ignored == filtered == 0, f"failed, ignored or filtered tests cannot establish complete execution: {target}")
    _require(seen == set(targets), f"test targets did not finish: {sorted(set(targets) - seen)}")
    actual_tests = test_results((run_dir / "test.log").read_text(encoding="utf-8"), targets)
    _require(sorted(tests, key=lambda result: result["target"]) == actual_tests, "manifest test counters differ from complete harness results")
    report = analyze(
        run_dir / "production.lcov", run_dir / "execution.lcov",
        run_dir / "production.json", run_dir / "execution.json",
        run_dir / "classification.json", root,
    )
    _require(read_json(run_dir / "report.json") == report, "published report differs from raw measurement")
    try:
        published_lcov = (run_dir / "eligible.lcov").read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise EvidenceError(f"cannot read eligible LCOV: {error}") from error
    _require(published_lcov == eligible_lcov(report, root), "published LCOV differs from eligible production lines")
    _require(not require_zero or report["uncovered_total"] == 0, f"production executable lines remain uncovered: {report['uncovered_total']}")
    return report
