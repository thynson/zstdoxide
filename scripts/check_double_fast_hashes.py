#!/usr/bin/env python3
"""Check the frozen small-log, eight-byte C hash observations offline."""

from pathlib import Path
import argparse
import csv
import hashlib
import io
import json
import sys

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / "tests/upstream/double-fast-hashes"
REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
HELPER = "scripts/oracles/double_fast_hashes.c"
PATTERNS = "tests/upstream/match-search/patterns.bin"
PRIOR = "tests/upstream/match-search/vectors.tsv"
CHECKER = "scripts/check_double_fast_hashes.py"
PINS = {HELPER: "dcb59d147063e812b1e179939b39b056c7fa0a9d8529866ced6c13f052c171d8",
        PATTERNS: "1081bb9bf9f8165786c86c63c227ad9e7450c0a87a83dd4645a3bddac6771ac3",
        PRIOR: "628ff9df0d3b6c972233c43e5ffbc4aa11a1445908054c5d30da47b3afd078ce"}
VECTOR_SHA256 = "16ee71dfb607ef5e6dfbbdf7ed3fd65599b1e5e24bfe701c2cecd76672c43cad"


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def exact(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)


def decode(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate JSON key")
            result[key] = value
        return result

    def nonfinite(value):
        raise ValueError("nonfinite JSON scalar: " + value)

    return json.loads(data, object_pairs_hook=pairs, parse_constant=nonfinite)


def validate_vectors(data):
    reader = csv.DictReader(io.StringIO(data.decode("ascii")), delimiter="\t")
    require(reader.fieldnames == ["log", "shift", "pattern", "bytes", "bucket"], "vector columns")
    rows = list(reader)
    require(len(rows) == 512, "vector count")
    patterns = (ROOT / PATTERNS).read_bytes()
    prior = {int(row["pattern"]): int(row["bucket"]) for row in
             csv.DictReader(io.StringIO((ROOT / PRIOR).read_text()), delimiter="\t")
             if row["width"] == "8" and row["log"] == "10"}
    for i, row in enumerate(rows):
        log, shift, pattern = 6 + i // 128, (i // 64) % 2, i % 64
        key = patterns[8 * pattern:8 * pattern + 8]
        expected = ((int.from_bytes(key, "little") * 0xcf1bbcdcb7a56463) & 0xffffffffffffffff) >> (64 - log)
        require(prior[pattern] >> (10 - log) == expected, "prior high-bit agreement")
        require(row == {"log": str(log), "shift": str(shift), "pattern": str(pattern),
                        "bytes": key.hex(), "bucket": str(expected)}, "canonical direct observation")


def check(collection):
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, "fixed input identity: " + name)
    collection = collection.resolve()
    require(collection.is_dir(), "missing collection")
    paths = list(collection.rglob("*"))
    require(all(path.is_file() and not path.is_symlink() for path in paths), "flat regular collection")
    require({path.name for path in paths} == {"manifest.json", "README.md", "NOTICE", "vectors.tsv"}, "exact collection catalog")
    require((collection / "manifest.json").stat().st_size < 1 << 20, "manifest cap")
    manifest = decode((collection / "manifest.json").read_bytes())
    require(type(manifest) is dict and exact(manifest.get("schema_version"), 1)
            and manifest.get("upstream_revision") == REVISION, "schema/revision")
    require(manifest.get("generator") == HELPER and manifest.get("generator_sha256") == PINS[HELPER], "helper identity")
    require(manifest.get("importer") == CHECKER and manifest.get("importer_sha256") == sha(ROOT / CHECKER), "checker identity")
    require(exact(manifest.get("repository_input_sha256"), {name: PINS[name] for name in (PATTERNS, PRIOR)}), "fixed pattern provenance")
    vector = collection / "vectors.tsv"
    require(vector.stat().st_size < 1 << 20 and sha(vector) == VECTOR_SHA256, "pinned C vector identity")
    require(exact(manifest.get("fixtures"), [{"path": "vectors.tsv", "size": vector.stat().st_size, "sha256": VECTOR_SHA256}]), "fixture metadata")
    require(exact(manifest.get("coverage"), {"width": 8, "logs": [6, 7, 8, 9], "shifts": [0, 1], "patterns": 64, "rows": 512,
                                            "normal_sanitized_equal": True, "inline_hash_only": True, "leak_sanitizer": False}), "scope")
    inventory = decode((ROOT / "tests/upstream/manifest.json").read_bytes())
    sources = {row["path"]: row["sha256"] for row in inventory["sources"] + inventory["excluded_sources"]
               if row["path"].startswith("lib/") and Path(row["path"]).suffix in (".c", ".h")}
    require(exact(manifest.get("source_sha256"), sources), "pinned source catalog")
    validate_vectors(vector.read_bytes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.add_argument("--collection", type=Path, default=COLLECTION)
    args = parser.parse_args()
    check(args.collection)
    print("Validated 512 direct eight-byte C hash observations at logs6..9 offline.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("ERROR: " + str(error), file=sys.stderr)
        raise SystemExit(1)
