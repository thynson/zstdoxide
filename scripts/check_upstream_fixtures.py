#!/usr/bin/env python3
"""Verify committed upstream fixture bytes and provenance without a C toolchain.

This checks hashes and metadata consistency, not codec behavior. Run Cargo tests
for actual decoding/encoding assertions and the importers' --check modes to
regenerate fixtures against an external pinned upstream build.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
REQUIRED_COLLECTIONS = ("data", "decodecorpus", "dictionaries", "entropy-data", "entropy-lifetimes", "inline-regressions", "match-search", "parameters", "testcard", "window-sequences")
OPTIONAL_COLLECTIONS = ()
NON_FIXTURE_FILES = {"manifest.json", "README.md", "cases.rs", "NOTICE"}


def safe_relative(path: str) -> bool:
    parsed = PurePosixPath(path)
    return bool(path) and not parsed.is_absolute() and ".." not in parsed.parts and "\\" not in path


def read_metadata(path: Path, errors: list[str]) -> dict | None:
    try:
        value = json.loads(path.read_text())
        if not isinstance(value, dict):
            raise ValueError("manifest must be an object")
        return value
    except (OSError, ValueError) as error:
        errors.append(f"{path}: cannot read manifest: {error}")
        return None


def check_file(directory: Path, path: str, size: int, digest: str,
               errors: list[str], label: str) -> bool:
    if not isinstance(path, str) or not safe_relative(path):
        errors.append(f"{label}: invalid fixture path {path!r}")
        return False
    target = directory / path
    if not target.resolve().is_relative_to(directory.resolve()):
        errors.append(f"{label}: fixture resolves outside its collection: {path}")
        return False
    if not isinstance(size, int) or isinstance(size, bool) or size < 0:
        errors.append(f"{label}: invalid fixture size for {path}")
        return False
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
        errors.append(f"{label}: invalid SHA-256 for {path}")
        return False
    try:
        actual_size = target.stat().st_size
        actual = hashlib.sha256()
        with target.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                actual.update(chunk)
        if actual_size != size:
            errors.append(f"{label}/{path}: size differs: expected {size}, found {actual_size}")
        if actual.hexdigest() != digest:
            errors.append(f"{label}/{path}: SHA-256 differs")
        return actual_size == size and actual.hexdigest() == digest
    except OSError as error:
        errors.append(f"{label}/{path}: missing or unreadable fixture: {error}")
        return False


def check_origin(path: str, digest: str, catalog: dict, errors: list[str], label: str) -> None:
    if path not in catalog:
        errors.append(f"{label}: upstream source is not in the pinned catalog: {path}")
    elif catalog[path]["sha256"] != digest:
        errors.append(f"{label}: source SHA-256 differs from pinned inventory: {path}")


def entries_for(collection: str, metadata: dict) -> list[dict]:
    if collection == "entropy-lifetimes":
        return [metadata["fixture"]]
    if collection == "data":
        return metadata["files"]
    if collection == "decodecorpus":
        entries = []
        for frame in metadata["frames"]:
            entries.extend([
                {"path": frame["name"] + ".zst", "size": frame["encoded_size"],
                 "sha256": frame["encoded_sha256"]},
                {"path": frame["name"] + ".raw", "size": frame["original_size"],
                 "sha256": frame["original_sha256"]},
            ])
        entries.extend({"path": header["name"] + ".zst", "size": header["header_size"],
                        "sha256": header["sha256"]} for header in metadata["headers"])
        return entries
    if isinstance(metadata["fixtures"], dict):
        return [dict(value, path=path) for path, value in metadata["fixtures"].items()]
    return metadata["fixtures"]


def check_expected_output(record: dict, entries: dict, errors: list[str], label: str) -> None:
    expected = record.get("expected_output")
    if expected is None:
        return
    if isinstance(expected, str):
        if expected not in entries:
            errors.append(f"{label}: missing expected-output fixture reference")
        return
    size, digest = expected["size"], expected["sha256"]
    if "derived_from" in record:
        original = entries.get(record["derived_from"])
        if original is None or original["size"] != size or original["sha256"] != digest:
            errors.append(f"{label}: generated frame's expected output differs from its source fixture")
    elif "hex" in expected:
        output = bytes.fromhex(expected["hex"])
        if len(output) != size or hashlib.sha256(output).hexdigest() != digest:
            errors.append(f"{label}: literal expected-output metadata is inconsistent")
    elif "repeat_byte" in expected:
        if not isinstance(size, int) or not 0 <= size <= 1 << 30:
            errors.append(f"{label}: invalid repeated expected-output size")
            return
        block = bytes([expected["repeat_byte"]]) * (1 << 16)
        actual = hashlib.sha256()
        whole, tail = divmod(size, len(block))
        for _ in range(whole):
            actual.update(block)
        actual.update(block[:tail])
        if actual.hexdigest() != digest:
            errors.append(f"{label}: repeated expected-output hash is inconsistent")
    else:
        errors.append(f"{label}: expected output has no checked origin, literal, or repeated byte")


def audit_fixtures(root: Path = ROOT) -> tuple[list[str], int, int]:
    errors: list[str] = []
    inventory = read_metadata(root / "tests/upstream/manifest.json", errors)
    if inventory is None:
        return errors, 0, 0
    try:
        if inventory["upstream"]["revision"] != REVISION:
            errors.append("The source inventory does not identify pinned upstream v1.5.7")
        catalog = {record["path"]: record for record in
                   inventory["sources"] + inventory["excluded_sources"]}
    except (KeyError, TypeError) as error:
        errors.append(f"Invalid upstream source inventory: {error}")
        return errors, 0, 0
    count = 0
    collections = 0
    base = root / "tests/upstream"
    for collection in REQUIRED_COLLECTIONS + OPTIONAL_COLLECTIONS:
        directory = base / collection
        if collection in OPTIONAL_COLLECTIONS and not directory.exists():
            continue
        metadata_path = directory / ("provenance.json" if collection == "entropy-lifetimes" else "manifest.json")
        metadata = read_metadata(metadata_path, errors)
        if metadata is None:
            continue
        collections += 1
        if metadata.get("upstream_revision") != REVISION:
            errors.append(f"{collection}: fixture manifest does not identify the pinned revision")
        try:
            records = entries_for(collection, metadata)
            if not records:
                errors.append(f"{collection}: fixture collection must not be empty")
            expected_paths: set[str] = set()
            entries = {record["path"]: record for record in records}
            if len(entries) != len(records):
                errors.append(f"{collection}: duplicate fixture path in manifest")
            for record in records:
                path = record["path"]
                expected_paths.add(path)
                check_file(directory, path, record["size"], record["sha256"], errors, collection)
                count += 1
                if "origin" in record:
                    check_origin(record["origin"], record["sha256"], catalog, errors, collection)
                if "source_sha256" in record:
                    check_origin(record["source"], record["source_sha256"], catalog, errors, collection)
                if "source" in record and record["source"] not in catalog:
                    errors.append(f"{collection}/{path}: source is absent from pinned inventory")
                if "dictionary" in record and record["dictionary"] not in entries:
                    errors.append(f"{collection}/{path}: missing dictionary fixture reference")
                if "byte_range" in record:
                    source = record["derived_from"]
                    start, end = record["byte_range"]
                    if source not in entries or not 0 <= start <= end <= entries[source]["size"]:
                        errors.append(f"{collection}/{path}: invalid source byte range")
                    elif not safe_relative(source) or not (directory / source).resolve().is_relative_to(directory.resolve()):
                        errors.append(f"{collection}/{path}: derived source resolves outside collection")
                    else:
                        original = (directory / source).read_bytes()[start:end]
                        if len(original) != record["size"] or hashlib.sha256(original).hexdigest() != record["sha256"]:
                            errors.append(f"{collection}/{path}: derived byte range differs from its source fixture")
                check_expected_output(record, entries, errors, f"{collection}/{path}")
            # A collection may keep its hashed oracle next to the fixture.
            # That source is checked independently below; it is not codec data.
            metadata_paths = {metadata_path.name}
            generator = metadata.get("generator")
            if isinstance(generator, str) and safe_relative(generator):
                generator_path = (root / generator).resolve()
                if generator_path.is_relative_to(directory.resolve()):
                    metadata_paths.add(generator_path.relative_to(directory.resolve()).as_posix())
            actual_paths = {path.relative_to(directory).as_posix()
                            for path in directory.rglob("*") if path.is_file()
                            and path.name not in NON_FIXTURE_FILES
                            and path.relative_to(directory).as_posix() not in metadata_paths}
            if actual_paths != expected_paths:
                errors.append(f"{collection}: fixture catalog differs from files: "
                              f"missing={sorted(expected_paths - actual_paths)}, "
                              f"unlisted={sorted(actual_paths - expected_paths)}")
            for source, digest in metadata.get("source_sha256", {}).items():
                check_origin(source, digest, catalog, errors, collection)
            if "generator_source" in metadata:
                check_origin(metadata["generator_source"], metadata["generator_source_sha256"],
                             catalog, errors, collection)
            local_sources = [(role, metadata[role], metadata[role + "_sha256"])
                             for role in ("generator", "importer") if role in metadata]
            local_sources.extend(("repository input", path, digest) for path, digest
                                 in metadata.get("repository_input_sha256", {}).items())
            for role, path, digest in local_sources:
                if (not isinstance(path, str) or not safe_relative(path)
                        or not (root / path).is_file()
                        or not (root / path).resolve().is_relative_to(root.resolve())):
                    errors.append(f"{collection}: missing or invalid local {role}")
                elif hashlib.sha256((root / path).read_bytes()).hexdigest() != digest:
                    errors.append(f"{collection}: local {role} SHA-256 differs")
        except (KeyError, TypeError, ValueError, OSError) as error:
            errors.append(f"{collection}: malformed fixture manifest: {error}")
    return errors, collections, count


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT,
                        help="repository root (primarily useful for isolated audit checks)")
    args = parser.parse_args()
    errors, collections, count = audit_fixtures(args.root)
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    if errors:
        return 1
    print(f"Fixture integrity valid: {count} files in {collections} collections; "
          "source provenance and recorded expected-output metadata agree.")
    print("No upstream checkout, C compiler, or codec executable was required.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
