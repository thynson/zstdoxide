#!/usr/bin/env python3
"""Validate the pinned official-test inventory without running the C codec.

ZSTD_UPSTREAM, or --upstream, additionally verifies the complete v1.5.7 source
checkout and re-extracts case anchors. --require-complete makes unported test
intent an error; an ordinary inventory audit does not imply migration parity.
"""

from __future__ import annotations

import argparse
from collections import Counter
from fnmatch import fnmatchcase
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
TRACKED_FILES = 639
PATH_CATALOG_SHA256 = "3de4268a350f7a885b119df304e8b8f140f9ee316bf2aa5fac4e1e80486dfa25"
CASE_GROUPS = 615
CASE_CATALOG_SHA256 = "2ba77bf77f04cf15b2e75f5c83ac2d0e9e96d81bc6c3783ca0e90df28079ad66"
STATUSES = {"ported", "partial", "planned", "blocked", "non_applicable"}
ROLES = {"test_suite", "test_case", "fuzz_target", "fixture", "support", "entrypoint", "documentation"}
ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests/upstream/manifest.json"
SOURCE_PREFIXES = (
    "tests/", "build/", ".github/workflows/", "examples/", "zlibWrapper/",
    "doc/educational_decoder/", "contrib/pzstd/test/", "contrib/pzstd/utils/test/",
    "contrib/linux-kernel/test/", "contrib/seekable_format/tests/",
    "contrib/seekable_format/examples/", "contrib/VS2005/", "lib/dll/example/",
)
SOURCE_PATHS = {
    "Makefile", "TESTING.md", ".cirrus.yml", "lib/Makefile", "programs/Makefile",
    "programs/datagen.c", "programs/datagen.h", "programs/lorem.c", "programs/lorem.h",
    "contrib/freestanding_lib/freestanding.py", "contrib/diagnose_corruption/check_flipped_bits.c",
    "contrib/externalSequenceProducer/sequence_producer.c",
    "contrib/externalSequenceProducer/sequence_producer.h",
    "contrib/pzstd/BUCK", "contrib/pzstd/README.md", "contrib/linux-kernel/README.md",
}


def is_test_source(path: str) -> bool:
    """Include every tracked test tree, build/CI entrypoint, and test example."""
    return (
        path.startswith(SOURCE_PREFIXES)
        or path in SOURCE_PATHS
        or (path.startswith("contrib/") and PurePosixPath(path).name == "Makefile")
    )


def source_bytes(path: Path) -> bytes:
    # Git hashes the link destination, not the contents of the linked file.
    if path.is_symlink():
        return os.readlink(path).encode()
    data = path.read_bytes()
    # The pinned .gitattributes checks these text files out as CRLF even on
    # Unix. Hash canonical Git bytes so the inventory is platform-independent.
    crlf_patterns = ("*.sln", "*.vcxproj*", "*.vcproj*", "*.rc", "*.bat", "*.cmd")
    return data.replace(b"\r\n", b"\n") if any(
        fnmatchcase(path.name, pattern) for pattern in crlf_patterns
    ) else data


def tree_entries(upstream: Path) -> dict[str, tuple[str, str]]:
    output = subprocess.check_output(
        ["git", "-C", str(upstream), "ls-tree", "-rz", "--full-tree", REVISION]
    )
    result = {}
    for record in output.split(b"\0"):
        if not record:
            continue
        header, path = record.split(b"\t", 1)
        mode, kind, blob = header.decode().split()
        if kind != "blob":
            raise ValueError(f"Unsupported source object: {path!r} ({kind})")
        result[path.decode()] = (mode, blob)
    return result


def extract_cases(path: str, data: bytes) -> list[dict]:
    """Extract named groups, not a claim to enumerate loop/assertion executions.

    Each whole-file record also tracks unlabelled cases. Source spans extend to
    the next named group and are navigation ranges, not C syntax boundaries.
    """
    if PurePosixPath(path).suffix not in {".c", ".cpp", ".sh"}:
        return []
    text = data.decode("utf-8", errors="replace")
    markers: dict[int, tuple[str, str]] = {}
    lines = text.splitlines()
    for index, line in enumerate(lines, 1):
        # C/C++ named diagnostics include fuzzer, streaming, and seekable tests.
        if re.search(r"(?:DISPLAY(?:LEVEL)?|printf)\s*\(", line):
            strings = re.findall(r'"((?:[^"\\]|\\.)*)"', line)
            label = " ".join(strings)
            if re.search(r"(?:^|\s)(?:test%|longtest%|Test %|test\s*:|thread pool test\s*:)", label):
                markers[index] = ("diagnostic_group", label.strip())
        google = re.match(r"\s*(TEST(?:_F|_P)?)\s*\(\s*(\w+)\s*,\s*(\w+)\s*\)", line)
        if google:
            markers[index] = ("google_test", f"{google[2]}.{google[3]}")
        if re.match(r"\s*int\s+LLVMFuzzerTestOneInput\s*\(", line):
            markers[index] = ("fuzz_entrypoint", "LLVMFuzzerTestOneInput")
        # Functions capture randomized campaigns and standalone assertion suites
        # which deliberately have no diagnostic label for every assertion.
        function = re.match(
            r"^(?:static\s+)?(?:int|void|size_t|unsigned)\s+(\w*[Tt]est\w*)\s*\([^;]*$",
            line,
        )
        if function and not line.rstrip().endswith(";"):
            markers.setdefault(index, ("test_function", function[1]))
        if path.endswith(".sh"):
            shell = re.match(r'\s*(?:println|echo)\s+"(.*)"', line)
            if shell and re.search(r"(?:^test\s*:|^Test\s*:|===>)", shell[1]):
                markers[index] = ("shell_group", shell[1].strip())
    result = []
    starts = sorted(markers)
    for position, start in enumerate(starts):
        kind, label = markers[start]
        end = starts[position + 1] - 1 if position + 1 < len(starts) else len(lines)
        result.append({"id": f"{path}:{start}", "kind": kind, "line_start": start,
                       "line_end": end, "label": label})
    return result


def valid_relative(path: str) -> bool:
    parsed = PurePosixPath(path)
    return bool(path) and not parsed.is_absolute() and ".." not in parsed.parts and "\\" not in path


def check_reference(reference: str, errors: list[str], context: str) -> None:
    path, _, symbol = reference.partition("#")
    if not valid_relative(path) or not (ROOT / path).is_file():
        errors.append(f"{context}: missing or invalid local target {reference}")
    elif symbol:
        pattern = (r"\bfn\s+" if path.endswith(".rs") else r"\b") + re.escape(symbol) + r"\b"
        contents = (ROOT / path).read_text()
        generated_test = path.endswith(".rs") and re.search(
            r"!\s*\(\s*" + re.escape(symbol) + r"\s*[,)]", contents
        )
        if not re.search(pattern, contents) and not generated_test:
            errors.append(f"{context}: local target function does not exist: {reference}")


def check_status(unit: dict, errors: list[str], context: str) -> None:
    if unit.get("status") not in STATUSES:
        errors.append(f"{context}: invalid status {unit.get('status')!r}")
    if not isinstance(unit.get("reason"), str) or len(unit["reason"].strip()) < 12:
        errors.append(f"{context}: an explicit status explanation is required")
    targets = unit.get("rust_tests")
    if not isinstance(targets, list) or any(not isinstance(target, str) for target in targets):
        errors.append(f"{context}: rust_tests must be a list of local file#symbol references")
        return
    if unit.get("status") in {"ported", "partial"} and not targets:
        errors.append(f"{context}: implemented status requires at least one local target")
    for reference in targets:
        check_reference(reference, errors, context)


def audit(manifest: dict, upstream: Path | None, require_complete: bool) -> list[str]:
    errors: list[str] = []
    if manifest.get("schema_version") != 1:
        errors.append("Unknown manifest schema")
    if manifest.get("upstream", {}).get("revision") != REVISION:
        errors.append("Manifest does not identify the pinned upstream revision")
    if manifest.get("upstream", {}).get("tracked_file_count") != TRACKED_FILES:
        errors.append("Manifest source count differs from pinned upstream v1.5.7")
    if manifest.get("upstream", {}).get("path_catalog_sha256") != PATH_CATALOG_SHA256:
        errors.append("Manifest path catalog differs from pinned upstream v1.5.7")
    sources = manifest.get("sources", [])
    excluded = manifest.get("excluded_sources", [])
    all_records = sources + excluded
    paths = [entry.get("path", "") for entry in all_records]
    if len(paths) != len(set(paths)):
        errors.append("Duplicate source path in inventory")
    if len(paths) != manifest.get("upstream", {}).get("tracked_file_count"):
        errors.append("Tracked source count does not match the complete catalog")
    digest = hashlib.sha256("\n".join(sorted(paths)).encode()).hexdigest()
    if digest != PATH_CATALOG_SHA256:
        errors.append("Complete upstream path catalog changed or has omissions")
    for record in all_records:
        path = record.get("path", "")
        if not valid_relative(path):
            errors.append(f"Invalid upstream path {path!r}")
        if not re.fullmatch(r"[0-9a-f]{64}", record.get("sha256", "")):
            errors.append(f"{path}: invalid SHA-256")
        if not re.fullmatch(r"[0-9a-f]{40}", record.get("git_blob", "")):
            errors.append(f"{path}: invalid Git blob identifier")
    case_ids = set()
    for source in sources:
        path = source["path"]
        if not is_test_source(path):
            errors.append(f"{path}: source is outside the documented selection policy")
        if source.get("role") not in ROLES or not source.get("suite"):
            errors.append(f"{path}: missing or invalid role/suite")
        check_status(source, errors, path)
        for case in source.get("cases", []):
            case_id = case.get("id", "")
            if case_id in case_ids:
                errors.append(f"Duplicate case {case_id}")
            case_ids.add(case_id)
            start, end = case.get("line_start", 0), case.get("line_end", 0)
            if not 1 <= start <= end <= source.get("line_count", 0):
                errors.append(f"{case_id}: invalid case source span")
            if case_id != f"{path}:{start}":
                errors.append(f"{case_id}: unstable or mismatched source anchor")
            check_status(case, errors, case_id)
        if source["status"] == "ported" and any(
            case["status"] not in {"ported", "non_applicable"} for case in source.get("cases", [])
        ):
            errors.append(f"{path}: fully ported source contains unfinished case groups")
    for record in excluded:
        if is_test_source(record["path"]):
            errors.append(f"{record['path']}: test-related source incorrectly excluded")
        if len(record.get("reason", "")) < 12:
            errors.append(f"{record['path']}: missing exclusion reason")
    case_digest = hashlib.sha256("\n".join(sorted(case_ids)).encode()).hexdigest()
    if len(case_ids) != CASE_GROUPS or case_digest != CASE_CATALOG_SHA256:
        errors.append("Named case catalog changed or has omissions relative to pinned v1.5.7")
    for corpus in manifest.get("external_corpora", []):
        if corpus.get("source") not in paths or corpus.get("status") not in STATUSES:
            errors.append("External corpus has an invalid source or status")
        if len(corpus.get("reason", "")) < 12:
            errors.append("External corpus requires a reproducibility explanation")
        if "manifest" in corpus:
            check_reference(corpus["manifest"], errors, corpus["source"])
        for reference in corpus.get("rust_tests", []):
            check_reference(reference, errors, corpus["source"])
    if upstream is not None:
        entries = tree_entries(upstream)
        if set(paths) != set(entries):
            missing = sorted(set(entries) - set(paths))
            extra = sorted(set(paths) - set(entries))
            errors.append(f"Pinned Git tree differs: missing={missing}, extra={extra}")
        expected_included = {path for path in entries if is_test_source(path)}
        if {source["path"] for source in sources} != expected_included:
            errors.append("Inventory does not include every test-related file in the pinned tree")
        for record in all_records:
            path = record["path"]
            if not valid_relative(path):
                continue
            local = upstream / path
            if not local.exists() and not local.is_symlink():
                errors.append(f"{path}: missing from upstream checkout")
                continue
            data = source_bytes(local)
            if hashlib.sha256(data).hexdigest() != record["sha256"]:
                errors.append(f"{path}: upstream checkout SHA-256 differs")
            if entries.get(path, (None, None))[1] != record["git_blob"]:
                errors.append(f"{path}: pinned Git blob differs")
            git_object = f"blob {len(data)}\0".encode() + data
            if hashlib.sha1(git_object).hexdigest() != record["git_blob"]:
                errors.append(f"{path}: checkout contents differ from pinned Git blob")
            if "cases" in record:
                actual = extract_cases(path, data)
                selected_keys = {"id", "kind", "line_start", "line_end", "label"}
                recorded = [{key: value for key, value in case.items() if key in selected_keys}
                            for case in record["cases"]]
                if actual != recorded:
                    errors.append(f"{path}: named test groups differ from pinned source")
    if require_complete:
        unfinished = [source["path"] for source in sources
                      if source["status"] not in {"ported", "non_applicable"}]
        if unfinished:
            errors.append(f"Migration incomplete: {len(unfinished)} source units remain unported")
        if any(corpus["status"] not in {"ported", "non_applicable"}
               for corpus in manifest.get("external_corpora", [])):
            errors.append("Migration incomplete: external upstream corpus target coverage remains partial or blocked")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, default=os.environ.get("ZSTD_UPSTREAM"))
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--require-complete", action="store_true")
    args = parser.parse_args()
    try:
        manifest = json.loads(args.manifest.read_text())
        errors = audit(manifest, args.upstream, args.require_complete)
    except (KeyError, TypeError, ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Inventory error: {error}", file=sys.stderr)
        return 1
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    if errors:
        return 1
    sources = manifest["sources"]
    cases = [case for source in sources for case in source.get("cases", [])]
    print(f"Inventory valid: {len(sources)} test-related sources, {len(manifest['excluded_sources'])} "
          f"explicit non-test exclusions, {len(cases)} named case groups.")
    print("Source migration status: " + json.dumps(dict(sorted(Counter(s['status'] for s in sources).items()))))
    print("Case migration status: " + json.dumps(dict(sorted(Counter(c['status'] for c in cases).items()))))
    print("Upstream checkout hashes and case anchors verified." if args.upstream else
          "Structural audit only; set ZSTD_UPSTREAM to verify the pinned checkout hashes.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
