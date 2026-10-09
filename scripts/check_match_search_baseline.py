#!/usr/bin/env python3
"""Audit fixed pre-Module-11 baseline bytes without running or regenerating a codec.

The committed archive is read in memory, never extracted. SHA-256 checks connect
its summary, package manifest, reproduction sources and current replay inputs.
Use --root with an isolated fixture tree for negative validation.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = "tests/fixtures/match-search-baseline"
SUMMARY = "docs/evidence/2026-10-09-match-search-baseline.json"
ARCHIVE = "docs/evidence/2026-10-09-match-search-baseline.tar.gz"
CODEC_REVISION = "ba4529c9e44eee2218209bb660038e42848d11e8"
REFERENCE_REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
MAX_FILE_BYTES = 2 << 20
MAX_ARCHIVE_MEMBERS = 32
SOURCES = {
    "capture.rs", "capture.Cargo.toml", "capture.Cargo.lock",
    "capture.py", "oracle.py", "rdg_support.rs",
}


class AuditError(ValueError):
    """A required provenance or byte comparison failed."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AuditError(message)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def check_digest(data: bytes, expected: str, label: str) -> None:
    require(isinstance(expected, str) and re.fullmatch(r"[0-9a-f]{64}", expected) is not None,
            f"{label}: invalid SHA-256 metadata")
    require(digest(data) == expected, f"{label}: SHA-256 differs")


def read_file(root: Path, name: str) -> bytes:
    path = root / name
    require(path.resolve().is_relative_to(root), f"{name}: path escapes the supplied root")
    require(not path.is_symlink() and path.is_file(), f"{name}: missing regular file")
    require(path.stat().st_size <= MAX_FILE_BYTES, f"{name}: exceeds audit size limit")
    data = path.read_bytes()
    require(len(data) <= MAX_FILE_BYTES, f"{name}: exceeds audit size limit")
    return data


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, f"JSON duplicate key: {key}")
        result[key] = value
    return result


def object_from(data: bytes, label: str) -> dict:
    result = json.loads(data, object_pairs_hook=unique_object)
    require(isinstance(result, dict), f"{label}: expected a JSON object")
    return result


def archive_members(data: bytes) -> dict[str, bytes]:
    members = {}
    total = 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive:
            name = member.name
            parsed = PurePosixPath(name)
            require(bool(name) and not parsed.is_absolute() and str(parsed) == name
                    and ".." not in parsed.parts and "\\" not in name,
                    f"archive: invalid member name {name!r}")
            require(member.isfile(), f"archive: nonregular member {name}")
            require(name not in members, f"archive: duplicate member {name}")
            total += member.size
            require(0 <= member.size <= MAX_FILE_BYTES and total <= MAX_FILE_BYTES,
                    "archive: uncompressed size limit exceeded")
            require(len(members) < MAX_ARCHIVE_MEMBERS, "archive: member limit exceeded")
            handle = archive.extractfile(member)
            require(handle is not None, f"archive: cannot read {name}")
            content = handle.read(member.size + 1)
            require(len(content) == member.size, f"archive: incomplete member {name}")
            members[name] = content
    return members


def check_entry(data: bytes, entry: dict, label: str) -> None:
    size = entry["bytes"]
    require(type(size) is int and size >= 0, f"{label}: invalid size metadata")
    require(len(data) == size, f"{label}: byte length differs")
    check_digest(data, entry["sha256"], label)


def tsv_rows(data: bytes, columns: int, count: int, label: str) -> list[dict[str, str]]:
    reader = csv.DictReader(io.StringIO(data.decode("utf-8")), delimiter="\t")
    fields = reader.fieldnames
    require(fields is not None and len(fields) == columns and len(set(fields)) == columns,
            f"{label}: invalid TSV header")
    rows = list(reader)
    require(len(rows) == count, f"{label}: row count differs")
    require(all(None not in row and None not in row.values() for row in rows),
            f"{label}: invalid TSV row width")
    return rows


def audit(root: Path) -> None:
    root = root.resolve()
    summary = object_from(read_file(root, SUMMARY), SUMMARY)
    provenance = object_from(read_file(root, f"{COLLECTION}/provenance.json"), "provenance")
    require(summary["archive"]["path"] == provenance["archived_package"] == ARCHIVE,
            "archive path differs from the fixed baseline path")
    packed = read_file(root, ARCHIVE)
    check_digest(packed, summary["archive"]["sha256"], "summary archive")
    check_digest(packed, provenance["archived_package_sha256"], "provenance archive")
    members = archive_members(packed)
    manifest_bytes = members["package-manifest.json"]
    check_digest(manifest_bytes, summary["package_manifest_sha256"], "package manifest")
    manifest = object_from(manifest_bytes, "package manifest")
    catalog = manifest["files"]
    require(isinstance(catalog, dict), "package files must be an object")
    require(set(members) == set(catalog) | {"package-manifest.json"},
            "archive members differ from the package manifest")
    for name, entry in catalog.items():
        check_entry(members[name], entry, f"archive/{name}")
    require(sum(len(members[name]) for name in catalog) == manifest["bytes_excluding_manifest"],
            "package total byte count differs")

    metadata = object_from(members["metadata.json"], "archived metadata")
    evidence = object_from(members["capture-evidence.json"], "capture evidence")
    snapshot = object_from(members["capture-snapshot-manifest.json"], "capture snapshot")
    for document in (summary, provenance, manifest, metadata, evidence):
        require(document["codec_revision"] == CODEC_REVISION, "codec revision differs")
    require(snapshot["revision"] == CODEC_REVISION, "snapshot revision differs")
    for document in (summary, provenance, evidence):
        require(document["reference_revision"] == REFERENCE_REVISION, "reference revision differs")
    require(evidence["summary"] == summary["summary"], "capture summary differs")
    for key, value in metadata.items():
        require(provenance[key] == value, f"provenance/{key}: differs from archived metadata")
    check_digest(members["capture-evidence.json"], summary["capture_evidence_sha256"],
                 "summary capture evidence")
    check_digest(members["capture-evidence.json"], provenance["source_evidence_sha256"],
                 "provenance capture evidence")
    require(provenance["independent_c_decodes"] == summary["summary"]["native_c_exact_decodes"] == 152,
            "independent C decode count differs")

    require(set(metadata["source_files"]) == SOURCES, "reproduction source set differs")
    for name, entry in metadata["source_files"].items():
        check_entry(members[f"sources/{name}"], entry, f"reproduction source/{name}")
        require(evidence["files"][entry["original_path"]] ==
                {"sha256": entry["sha256"], "bytes": entry["bytes"]},
                f"reproduction source/{name}: original evidence differs")

    current = {}
    for name in ("expected.tsv", "cases.tsv"):
        current[name] = read_file(root, f"{COLLECTION}/{name}")
        check_digest(current[name], provenance[name.replace(".tsv", "_tsv_sha256")], name)
        require(current[name] == members[name], f"{name}: differs from original archive bytes")
    check_digest(current["expected.tsv"], summary["expected_tsv_sha256"], "summary expected.tsv")
    records = tsv_rows(current["expected.tsv"], 33, 152, "expected.tsv")
    cases = tsv_rows(current["cases.tsv"], 14, 12, "cases.tsv")
    require(manifest["expected_records"] == metadata["row_count"] == len(records),
            "expected-record metadata count differs")
    require(metadata["input_cases"] == len(cases), "recipe metadata count differs")
    for backend in ("safe", "all"):
        selected = [row for row in records if row["backend"] == backend]
        require(len(selected) == manifest["backend_records"][backend] ==
                metadata["rows_per_backend"] == 76, f"{backend}: record count differs")
        require([int(row["record_index"]) for row in selected] == list(range(76)),
                f"{backend}: record order differs")

    support_path = "tests/upstream/support/mod.rs"
    support = read_file(root, support_path)
    require(support == members["sources/rdg_support.rs"], "current RDG source differs from capture")
    check_digest(support, snapshot["files"][support_path], "snapshot RDG source")
    dictionary_path = "tests/upstream/dictionaries/formatted.dict"
    dictionary = read_file(root, dictionary_path)
    check_digest(dictionary, snapshot["files"][dictionary_path], "snapshot formatted dictionary")
    formatted = [row for row in cases if row["case"] == "formatted-seed"]
    require(len(formatted) == 1, "formatted recipe must occur exactly once")
    check_digest(dictionary, formatted[0]["dictionary_sha256"], "recipe formatted dictionary")
    require(len(dictionary) == int(formatted[0]["dictionary_serialized_length"]),
            "recipe formatted dictionary size differs")
    for backend in ("safe", "all"):
        original = evidence["cases"][backend]["formatted-seed"]
        check_digest(dictionary, original["dictionary_sha256"], f"{backend} formatted dictionary")
        require(len(dictionary) == original["dictionary_serialized_length"],
                f"{backend} formatted dictionary size differs")
    print(f"Verified match-search baseline: {len(records)} records, {len(cases)} recipes, "
          f"{len(members)} archived files, {len(SOURCES)} reproduction sources, 2 current inputs.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT,
                        help="repository or isolated fixture tree (default: this repository)")
    args = parser.parse_args()
    try:
        audit(args.root)
    except (OSError, ValueError, KeyError, TypeError, AttributeError, tarfile.TarError) as error:
        print(f"Match-search baseline audit failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
